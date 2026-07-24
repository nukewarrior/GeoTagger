<script setup lang="ts">
import { computed } from "vue";

import IconGlyph from "@/components/IconGlyph.vue";
import { useWorkspaceStore } from "@/stores/workspace";
import type { PhotoRecord } from "@/types/domain";

const store = useWorkspaceStore();

const bounds = computed(() => {
  const track = store.project?.tracks[0];
  return {
    start: track ? new Date(track.startUtc).getTime() : 0,
    end: track ? new Date(track.endUtc).getTime() : 1,
  };
});

const tickLabels = computed(() => {
  const { start, end } = bounds.value;
  return Array.from({ length: 6 }, (_, index) => {
    const time = start + ((end - start) * index) / 5;
    return new Intl.DateTimeFormat("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
      timeZone: "UTC",
    }).format(new Date(time));
  });
});

function position(photo: PhotoRecord): number {
  if (!photo.captureUtc) return 98;
  const corrected =
    new Date(photo.captureUtc).getTime() +
    (store.project?.settings.fixedOffsetMs ?? 0);
  const value =
    ((corrected - bounds.value.start) /
      Math.max(1, bounds.value.end - bounds.value.start)) *
    100;
  return Math.min(99, Math.max(1, value));
}

function statusClass(photoId: string): string {
  return (store.matchesByPhotoId.get(photoId)?.status ?? "NO_CAPTURE_TIME")
    .toLocaleLowerCase()
    .replaceAll("_", "-");
}
</script>

<template>
  <section class="timeline-panel">
    <div class="timeline-header">
      <div>
        <IconGlyph name="clock" :size="15" />
        <strong>时间轴</strong>
        <span>UTC 标准化预览</span>
      </div>
      <span v-if="store.previewPending" class="recalculating">
        正在重新匹配…
      </span>
      <span v-else>偏差 {{ (store.project?.settings.fixedOffsetMs ?? 0) / 1000 }} 秒</span>
    </div>

    <div class="timeline-ruler">
      <div class="ruler-labels">
        <span v-for="label in tickLabels" :key="label">{{ label }}</span>
      </div>
      <div class="ruler-track">
        <i class="route-range" />
        <button
          v-for="photo in store.project?.photos"
          :key="photo.id"
          class="timeline-event"
          :class="[
            statusClass(photo.id),
            { active: store.activePhotoId === photo.id },
          ]"
          :style="{ left: `${position(photo)}%` }"
          type="button"
          :title="photo.fileName"
          @click="store.selectPhoto(photo.id)"
        />
        <i
          v-if="store.activePhoto"
          class="time-cursor"
          :style="{ left: `${position(store.activePhoto)}%` }"
        >
          <span>{{
            store.activePhoto.captureLocal?.slice(11, 19) ?? "无时间"
          }}</span>
        </i>
      </div>
    </div>

    <div class="timeline-thumbnails">
      <button
        v-for="photo in store.project?.photos"
        :key="photo.id"
        type="button"
        :class="{ active: store.activePhotoId === photo.id }"
        @click="store.selectPhoto(photo.id)"
      >
        <span
          class="mini-photo"
          :style="{ '--thumb-tone': photo.thumbnailTone ?? '#718278' }"
        >
          <i />
        </span>
        <small>{{ photo.fileName.replace("DSC_", "").replace(".JPG", "") }}</small>
      </button>
    </div>
  </section>
</template>
