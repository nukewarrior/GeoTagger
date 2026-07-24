import type {
  MatchStatus,
  PhotoMatch,
  PhotoRecord,
  ProjectSnapshot,
  TrackPoint,
} from "@/types/domain";

const routeSeed: Array<[number, number, number]> = [
  [86.716, 48.512, 1_026],
  [86.731, 48.535, 1_094],
  [86.744, 48.558, 1_168],
  [86.751, 48.587, 1_238],
  [86.765, 48.612, 1_314],
  [86.781, 48.632, 1_387],
  [86.798, 48.646, 1_446],
  [86.821, 48.659, 1_521],
  [86.844, 48.666, 1_588],
  [86.866, 48.684, 1_642],
  [86.878, 48.707, 1_716],
  [86.873, 48.731, 1_824],
  [86.885, 48.752, 1_907],
  [86.902, 48.772, 1_986],
  [86.897, 48.796, 2_056],
  [86.913, 48.817, 2_116],
  [86.936, 48.832, 2_043],
  [86.955, 48.851, 1_972],
  [86.982, 48.858, 1_894],
  [87.004, 48.872, 1_806],
];

const photoNames = [
  "DSC_01234.JPG",
  "DSC_01235.JPG",
  "DSC_01236.JPG",
  "DSC_01237.JPG",
  "DSC_01238.JPG",
  "DSC_01239.JPG",
  "DSC_01240.JPG",
  "DSC_01241.JPG",
  "DSC_01242.JPG",
  "DSC_01243.JPG",
  "DSC_01244.JPG",
  "DSC_01245.JPG",
];

const statuses: MatchStatus[] = [
  "MATCHED_HIGH",
  "MATCHED_HIGH",
  "MATCHED_MEDIUM",
  "MATCHED_HIGH",
  "OUT_OF_RANGE",
  "MATCHED_LOW",
  "MATCHED_HIGH",
  "MATCHED_MEDIUM",
  "MATCHED_HIGH",
  "SEGMENT_GAP",
  "MATCHED_HIGH",
  "NO_CAPTURE_TIME",
];

function isoAtMinutes(base: Date, minutes: number): string {
  return new Date(base.getTime() + minutes * 60_000).toISOString();
}

function buildTrackPoints(): TrackPoint[] {
  const base = new Date("2024-10-03T00:21:13.000Z");

  return routeSeed.flatMap(([lon, lat, elevation], index) => {
    const next = routeSeed[index + 1];
    const steps = next ? 5 : 1;

    return Array.from({ length: steps }, (_, step) => {
      const ratio = step / steps;
      const pointLon = next ? lon + (next[0] - lon) * ratio : lon;
      const pointLat = next ? lat + (next[1] - lat) * ratio : lat;
      const pointElevation = next
        ? elevation + (next[2] - elevation) * ratio
        : elevation;
      const pointIndex = index * 5 + step;

      return {
        id: `track-point-${pointIndex}`,
        segmentId: index < 11 ? "segment-northbound" : "segment-alpine",
        timeUtc: isoAtMinutes(base, pointIndex * 6.2),
        original: { lat: pointLat, lon: pointLon, crs: "GCJ02" as const },
        normalized: { lat: pointLat, lon: pointLon },
        elevation: Math.round(pointElevation),
        hdop: 0.8 + (pointIndex % 7) * 0.12,
      };
    });
  });
}

function buildPhotos(trackPoints: TrackPoint[]): PhotoRecord[] {
  const captureBase = new Date("2024-10-03T01:12:45.000Z");

  return photoNames.map((fileName, index) => {
    const captureUtc =
      statuses[index] === "NO_CAPTURE_TIME"
        ? undefined
        : isoAtMinutes(captureBase, index * 21.3);

    return {
      id: `photo-${index + 1}`,
      path: `/Photos/2024/1003/${fileName}`,
      relativePath: `1003/${fileName}`,
      fileName,
      captureLocal: captureUtc?.replace("Z", ""),
      captureUtc,
      timezoneSource:
        statuses[index] === "NO_CAPTURE_TIME" ? "MISSING" : "PROJECT_DEFAULT",
      existingGps:
        index === 6
          ? {
              lat: trackPoints[34].normalized.lat,
              lon: trackPoints[34].normalized.lon,
              altitude: trackPoints[34].elevation,
            }
          : undefined,
      fileSize: 12_600_000 + index * 381_117,
      modifiedAt: captureUtc ?? "2024-10-03T06:18:00.000Z",
      sourceHash: `demo-sha256-${String(index + 1).padStart(2, "0")}`,
      thumbnailTone: [
        "#7ba3a0",
        "#798f62",
        "#b79059",
        "#627d6d",
        "#7a7771",
        "#557587",
      ][index % 6],
    };
  });
}

function buildMatches(
  photos: PhotoRecord[],
  trackPoints: TrackPoint[],
): PhotoMatch[] {
  return photos.map((photo, index) => {
    const status = statuses[index];
    const trackPoint = trackPoints[Math.min(8 + index * 6, trackPoints.length - 1)];
    const matched = status.startsWith("MATCHED");
    const confidence =
      status === "MATCHED_HIGH"
        ? 0.93 - (index % 3) * 0.03
        : status === "MATCHED_MEDIUM"
          ? 0.68
          : status === "MATCHED_LOW"
            ? 0.38
            : 0;

    return {
      photoId: photo.id,
      trackId: matched ? "track-day3" : undefined,
      lat: matched ? trackPoint.normalized.lat : undefined,
      lon: matched ? trackPoint.normalized.lon : undefined,
      elevation: matched ? trackPoint.elevation : undefined,
      method: matched ? "INTERPOLATED" : undefined,
      confidence,
      status,
      reason:
        status === "OUT_OF_RANGE"
          ? "拍摄时间晚于所选轨迹范围"
          : status === "SEGMENT_GAP"
            ? "拍摄时间位于两个轨迹分段之间"
            : status === "NO_CAPTURE_TIME"
              ? "照片没有可用的拍摄时间"
              : status === "MATCHED_LOW"
                ? "相邻轨迹点时间间隔较大"
                : undefined,
      beforeDeltaSeconds: matched ? 2 + (index % 4) : undefined,
      afterDeltaSeconds: matched ? 3 + (index % 5) : undefined,
      estimatedErrorMeters: matched
        ? Math.round((1 - confidence) * 62 + 4)
        : undefined,
    };
  });
}

export function createDemoProject(): ProjectSnapshot {
  const points = buildTrackPoints();
  const photos = buildPhotos(points);

  return {
    schemaVersion: 1,
    project: {
      id: "project-xinjiang-demo",
      name: "新疆自驾 2024.10",
      createdAt: "2026-07-25T00:00:00.000Z",
      updatedAt: "2026-07-25T00:00:00.000Z",
      directory: "/Users/demo/Projects/xinjiang-2024",
      projectFile: "/Users/demo/Projects/xinjiang-2024/project.geotagger.json",
    },
    settings: {
      photoTimezone: "Asia/Shanghai",
      fixedOffsetMs: -12_000,
      mapProvider: "maplibre",
      outputMode: "COPY_TO_DIRECTORY",
      outputDirectory: "/Users/demo/Projects/xinjiang-2024/output",
      existingGpsPolicy: "SKIP",
      automaticUpdates: false,
    },
    tracks: [
      {
        id: "track-day3",
        name: "DAY3_喀纳斯环线.gpx",
        sourcePath: "/Tracks/DAY3_喀纳斯环线.gpx",
        sourceCrs: "GCJ02",
        hash: "demo-track-sha256",
        startUtc: points[0].timeUtc,
        endUtc: points.at(-1)?.timeUtc ?? points[0].timeUtc,
        pointCount: 3_586,
        segmentCount: 2,
        distanceMeters: 215_700,
        elevationMin: 987,
        elevationMax: 2_156,
        points,
      },
    ],
    photos,
    matches: buildMatches(photos, points),
    writeHistory: [],
    dirty: false,
  };
}
