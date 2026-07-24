<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { Feature, FeatureCollection, LineString, Point } from "geojson";
import maplibregl, { type GeoJSONSource, type Map } from "maplibre-gl";

import IconGlyph from "@/components/IconGlyph.vue";
import { useWorkspaceStore } from "@/stores/workspace";

const store = useWorkspaceStore();
const container = ref<HTMLElement>();
const mapFailed = ref(false);
const showTrack = ref(true);
const showPhotos = ref(true);
let map: Map | undefined;

const trackPoints = computed(() => store.project?.tracks.flatMap((track) => track.points) ?? []);

const trackGeoJson = computed<FeatureCollection<LineString>>(() => ({
  type: "FeatureCollection",
  features: store.project?.tracks.map((track) => ({
    type: "Feature" as const,
    properties: { trackId: track.id, name: track.name },
    geometry: {
      type: "LineString" as const,
      coordinates: track.points.map((point) => [
        point.normalized.lon,
        point.normalized.lat,
      ]),
    },
  })) ?? [],
}));

const photoGeoJson = computed<FeatureCollection<Point>>(() => ({
  type: "FeatureCollection",
  features: (store.project?.matches ?? [])
    .filter(
      (match) =>
        typeof match.lon === "number" && typeof match.lat === "number",
    )
    .map((match) => ({
      type: "Feature" as const,
      properties: {
        photoId: match.photoId,
        status: match.status,
        selected: match.photoId === store.activePhotoId,
      },
      geometry: {
        type: "Point" as const,
        coordinates: [match.lon ?? 0, match.lat ?? 0],
      },
    })),
}));

const selectedFeature = computed<FeatureCollection<Point>>(() => {
  const match = store.activeMatch;
  const features: Array<Feature<Point>> =
    match && typeof match.lon === "number" && typeof match.lat === "number"
      ? [
          {
            type: "Feature",
            properties: { photoId: match.photoId },
            geometry: {
              type: "Point",
              coordinates: [match.lon, match.lat],
            },
          },
        ]
      : [];
  return { type: "FeatureCollection", features };
});

const fallbackPath = computed(() => {
  if (trackPoints.value.length === 0) return "";
  const lons = trackPoints.value.map((point) => point.normalized.lon);
  const lats = trackPoints.value.map((point) => point.normalized.lat);
  const minLon = Math.min(...lons);
  const maxLon = Math.max(...lons);
  const minLat = Math.min(...lats);
  const maxLat = Math.max(...lats);

  return trackPoints.value
    .filter((_, index) => index % 3 === 0)
    .map((point, index) => {
      const x = 12 + ((point.normalized.lon - minLon) / (maxLon - minLon || 1)) * 76;
      const y = 90 - ((point.normalized.lat - minLat) / (maxLat - minLat || 1)) * 78;
      return `${index === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" ");
});

function source(name: string): GeoJSONSource | undefined {
  return map?.getSource(name) as GeoJSONSource | undefined;
}

function updateSources(): void {
  source("track")?.setData(trackGeoJson.value);
  source("photos")?.setData(photoGeoJson.value);
  source("selected-photo")?.setData(selectedFeature.value);
}

function fitTrack(): void {
  if (!map || trackPoints.value.length === 0) return;
  const bounds = new maplibregl.LngLatBounds();
  for (const point of trackPoints.value) {
    bounds.extend([point.normalized.lon, point.normalized.lat]);
  }
  map.fitBounds(bounds, { padding: 64, duration: 650, maxZoom: 12 });
}

function applyVisibility(): void {
  if (!map?.isStyleLoaded()) return;
  map.setLayoutProperty(
    "track-shadow",
    "visibility",
    showTrack.value ? "visible" : "none",
  );
  map.setLayoutProperty(
    "track-line",
    "visibility",
    showTrack.value ? "visible" : "none",
  );
  map.setLayoutProperty(
    "photo-halo",
    "visibility",
    showPhotos.value ? "visible" : "none",
  );
  map.setLayoutProperty(
    "photo-points",
    "visibility",
    showPhotos.value ? "visible" : "none",
  );
}

async function initializeMap(): Promise<void> {
  await nextTick();
  if (!container.value) return;

  try {
    map = new maplibregl.Map({
      container: container.value,
      center: [86.87, 48.69],
      zoom: 8.3,
      attributionControl: false,
      style: {
        version: 8,
        sources: {},
        layers: [
          {
            id: "offline-background",
            type: "background",
            paint: { "background-color": "#dfe5d7" },
          },
        ],
      },
    });

    map.addControl(
      new maplibregl.NavigationControl({ showCompass: true, visualizePitch: true }),
      "bottom-right",
    );

    map.on("load", () => {
      if (!map) return;
      map.addSource("track", {
        type: "geojson",
        data: trackGeoJson.value,
        lineMetrics: true,
      });
      map.addLayer({
        id: "track-shadow",
        type: "line",
        source: "track",
        paint: {
          "line-color": "#193d33",
          "line-width": 7,
          "line-opacity": 0.25,
          "line-blur": 2,
        },
      });
      map.addLayer({
        id: "track-line",
        type: "line",
        source: "track",
        paint: {
          "line-width": 3.6,
          "line-gradient": [
            "interpolate",
            ["linear"],
            ["line-progress"],
            0,
            "#21a978",
            0.48,
            "#d6aa2d",
            1,
            "#e6623b",
          ],
        },
      });
      map.addSource("photos", { type: "geojson", data: photoGeoJson.value });
      map.addLayer({
        id: "photo-halo",
        type: "circle",
        source: "photos",
        paint: {
          "circle-radius": 9,
          "circle-color": "#ffffff",
          "circle-opacity": 0.9,
        },
      });
      map.addLayer({
        id: "photo-points",
        type: "circle",
        source: "photos",
        paint: {
          "circle-radius": 5.5,
          "circle-color": [
            "match",
            ["get", "status"],
            "MATCHED_HIGH",
            "#168d61",
            "MATCHED_MEDIUM",
            "#d3a729",
            "MATCHED_LOW",
            "#df792d",
            "#d54d3f",
          ],
          "circle-stroke-width": 1,
          "circle-stroke-color": "#143a31",
        },
      });
      map.addSource("selected-photo", {
        type: "geojson",
        data: selectedFeature.value,
      });
      map.addLayer({
        id: "selected-photo-ring",
        type: "circle",
        source: "selected-photo",
        paint: {
          "circle-radius": 12,
          "circle-color": "rgba(255,255,255,0.25)",
          "circle-stroke-width": 3,
          "circle-stroke-color": "#1573e6",
        },
      });

      map.on("click", "photo-points", (event) => {
        const photoId = event.features?.[0]?.properties?.photoId;
        if (typeof photoId === "string") store.selectPhoto(photoId);
      });
      map.on("mouseenter", "photo-points", () => {
        if (map) map.getCanvas().style.cursor = "pointer";
      });
      map.on("mouseleave", "photo-points", () => {
        if (map) map.getCanvas().style.cursor = "";
      });

      store.mapReady = true;
      store.mapBaseAvailable = false;
      fitTrack();
    });

    map.on("error", () => {
      store.mapBaseAvailable = false;
    });
  } catch {
    mapFailed.value = true;
    store.mapReady = false;
  }
}

watch(
  () => [store.activePhotoId, store.project?.matches],
  () => {
    updateSources();
    const match = store.activeMatch;
    if (
      map &&
      typeof match?.lon === "number" &&
      typeof match.lat === "number"
    ) {
      map.easeTo({ center: [match.lon, match.lat], duration: 450 });
    }
  },
  { deep: true },
);

watch([showTrack, showPhotos], applyVisibility);

onMounted(initializeMap);
onBeforeUnmount(() => {
  map?.remove();
  map = undefined;
  store.mapReady = false;
});
</script>

<template>
  <section class="map-panel">
    <svg
      v-if="mapFailed"
      class="map-fallback"
      viewBox="0 0 100 100"
      preserveAspectRatio="none"
      aria-label="离线轨迹预览"
    >
      <defs>
        <pattern id="grid" width="8" height="8" patternUnits="userSpaceOnUse">
          <path d="M 8 0 L 0 0 0 8" fill="none" stroke="#96a591" stroke-width=".18" />
        </pattern>
        <linearGradient id="route-gradient" x1="0" y1="1" x2="1" y2="0">
          <stop offset="0" stop-color="#1aa273" />
          <stop offset=".55" stop-color="#d4ad2c" />
          <stop offset="1" stop-color="#e2603b" />
        </linearGradient>
      </defs>
      <rect width="100" height="100" fill="#dfe5d7" />
      <rect width="100" height="100" fill="url(#grid)" />
      <path
        :d="fallbackPath"
        fill="none"
        stroke="#173d32"
        stroke-width="1.8"
        opacity=".2"
      />
      <path
        :d="fallbackPath"
        fill="none"
        stroke="url(#route-gradient)"
        stroke-width=".75"
      />
    </svg>
    <div ref="container" class="maplibre-host" :class="{ hidden: mapFailed }" />

    <div class="map-mode-switch" aria-label="地图模式">
      <button class="active" type="button">地图</button>
      <button type="button" disabled>卫星</button>
      <button type="button" disabled>地形</button>
      <span>离线图层</span>
    </div>

    <div class="map-quick-controls">
      <button type="button" title="适应轨迹" @click="fitTrack">
        <IconGlyph name="home" />
      </button>
      <button type="button" title="图层">
        <IconGlyph name="layers" />
      </button>
    </div>

    <div class="layer-card">
      <strong>图层</strong>
      <label>
        <input v-model="showTrack" type="checkbox" />
        <span class="legend-line" />
        轨迹线
      </label>
      <label>
        <input v-model="showPhotos" type="checkbox" />
        <span class="legend-dot" />
        照片位置
      </label>
      <div class="time-legend">
        <span>颜色 · 按时间</span>
        <i />
        <small><b>早</b><b>晚</b></small>
      </div>
    </div>

    <div class="offline-badge">
      <span />
      底图未配置 · 自有轨迹图层正常
    </div>
  </section>
</template>
