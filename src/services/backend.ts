import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { createDemoProject } from "@/data/demoProject";
import type {
  CoordinateSystem,
  ImportTrackResult,
  MatchResult,
  MatchStatus,
  ProjectSnapshot,
  ScanPhotosResult,
  TaskStatus,
  WriteExecutionResult,
  WriteJobSummary,
  WritePlan,
} from "@/types/domain";

interface RawGeoPoint {
  lat: number;
  lon: number;
}

interface RawTrackPoint {
  timeUtc: string;
  original: RawGeoPoint & { crs: CoordinateSystem };
  normalized: RawGeoPoint;
  elevation?: number;
  hdop?: number;
}

interface RawTrack {
  id: string;
  name: string;
  sourcePath: string;
  relativePath: string;
  hashSha256: string;
  sourceCrs: CoordinateSystem;
  segments: Array<{
    id: string;
    sourceIndex: number;
    points: RawTrackPoint[];
  }>;
  startUtc: string;
  endUtc: string;
  pointCount: number;
  statistics: {
    distanceMeters: number;
    durationSeconds: number;
    minElevation?: number;
    maxElevation?: number;
    segmentCount: number;
  };
  warnings: Array<{ code: string; message: string }>;
}

interface RawPhoto {
  id: string;
  path: string;
  relativePath: string;
  fileName: string;
  fingerprint: {
    sha256: string;
    sizeBytes: number;
    modifiedUnixMs: number;
  };
  captureLocal?: string;
  captureUtc?: string;
  timezoneSource:
    | "METADATA_OFFSET"
    | "PROJECT_DEFAULT"
    | "USER_OVERRIDE"
    | "UNKNOWN";
  existingGps?: RawGeoPoint & { altitude?: number };
  thumbnail?: string;
}

type RawMatchStatus = MatchStatus | "ALREADY_HAS_GPS";

interface RawPhotoMatch {
  photoId: string;
  trackId?: string;
  segmentId?: string;
  lat?: number;
  lon?: number;
  elevation?: number;
  method: string;
  confidence?: number;
  status: RawMatchStatus;
  qualityStatus?: MatchStatus;
  reason: string;
  existingGpsConflict: boolean;
  matchedTimeUtc?: string;
  previousPointTimeUtc?: string;
  nextPointTimeUtc?: string;
  intervalSeconds?: number;
  estimatedErrorMeters?: number;
}

interface RawWriteItemResult {
  photoId: string;
  outputPath: string;
  status: "WRITTEN_VERIFIED" | "SKIPPED" | "FAILED" | "CANCELLED";
  message: string;
}

interface RawWriteJob {
  id: string;
  writePlanId: string;
  status:
    | "RUNNING"
    | "COMPLETED"
    | "COMPLETED_WITH_ERRORS"
    | "CANCELLED"
    | "FAILED";
  startedAt: string;
  finishedAt?: string;
  results: RawWriteItemResult[];
}

interface RawProjectSnapshot {
  schemaVersion: number;
  project: {
    id: string;
    name: string;
    createdAt: string;
    updatedAt: string;
  };
  settings: {
    photoTimezone: string;
    fixedOffsetMs: number;
    mapProvider: "maplibre" | "amap";
    outputMode: "COPY_TO_DIRECTORY";
    defaultOutputDirectory?: string;
  };
  calibration: {
    timezone: string;
    fixedOffsetMs: number;
    syncPoints: unknown[];
    driftModel?: string;
  };
  tracks: RawTrack[];
  photos: RawPhoto[];
  matches: RawPhotoMatch[];
  writeHistory: RawWriteJob[];
}

interface RawProjectSummary {
  projectPath: string;
}

interface RawTaskRecord {
  id: string;
  status: "PENDING" | "RUNNING" | "COMPLETED" | "FAILED" | "CANCELLED";
  message: string;
  error?: {
    code: string;
    message: string;
    suggestion: string;
  };
}

interface RawWritePlan {
  id: string;
  createdAt: string;
  outputDirectory: string;
  items: Array<{
    photoId: string;
    sourcePath: string;
    outputPath: string;
    action:
      | "WRITE_GPS"
      | "SKIP_EXISTING_GPS"
      | "PRESERVE_EXISTING_GPS"
      | "SKIP_NO_MATCH"
      | "SKIP_UNSUPPORTED_FORMAT"
      | "CONFLICT";
    oldGps?: RawGeoPoint & { altitude?: number };
    newGps?: RawGeoPoint & { altitude?: number };
    warnings: string[];
  }>;
  conflictCount: number;
}

let rawSnapshot: RawProjectSnapshot | undefined;
let activeProjectPath = "";
let rememberedExistingGpsPolicy:
  | "SKIP"
  | "OVERWRITE"
  | "PRESERVE_COPY" = "SKIP";
let rememberedAutomaticUpdates = false;

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

async function invokeDesktop<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (!isTauriRuntime()) {
    throw new Error("DESKTOP_RUNTIME_REQUIRED");
  }
  return invoke<T>(command, args);
}

function parentDirectory(path: string): string {
  const normalized = path.replaceAll("\\", "/");
  const index = normalized.lastIndexOf("/");
  return index > 0 ? normalized.slice(0, index) : normalized;
}

function secondsBetween(
  left: string | undefined,
  right: string | undefined,
): number | undefined {
  if (!left || !right) return undefined;
  return Math.abs(
    Math.round((new Date(left).getTime() - new Date(right).getTime()) / 1_000),
  );
}

function taskStatus(status: RawWriteJob["status"]): TaskStatus {
  if (status === "RUNNING") return "RUNNING";
  if (status === "CANCELLED") return "CANCELLED";
  if (status === "FAILED" || status === "COMPLETED_WITH_ERRORS") return "FAILED";
  return "COMPLETED";
}

function mapWriteJob(job: RawWriteJob): WriteJobSummary {
  return {
    id: job.id,
    writePlanId: job.writePlanId,
    status: taskStatus(job.status),
    startedAt: job.startedAt,
    finishedAt: job.finishedAt,
    succeeded: job.results.filter(
      (item) => item.status === "WRITTEN_VERIFIED",
    ).length,
    failed: job.results.filter((item) => item.status === "FAILED").length,
  };
}

function mapSnapshot(raw: RawProjectSnapshot, dirty = false): ProjectSnapshot {
  rawSnapshot = raw;
  const projectDirectory = activeProjectPath
    ? parentDirectory(activeProjectPath)
    : "";

  return {
    schemaVersion: 1,
    project: {
      id: raw.project.id,
      name: raw.project.name,
      createdAt: raw.project.createdAt,
      updatedAt: raw.project.updatedAt,
      directory: projectDirectory,
      projectFile: activeProjectPath || undefined,
    },
    settings: {
      photoTimezone: raw.settings.photoTimezone,
      fixedOffsetMs: raw.settings.fixedOffsetMs,
      mapProvider: "maplibre",
      outputMode: "COPY_TO_DIRECTORY",
      outputDirectory: raw.settings.defaultOutputDirectory,
      existingGpsPolicy: rememberedExistingGpsPolicy,
      automaticUpdates: rememberedAutomaticUpdates,
    },
    tracks: raw.tracks.map((track) => ({
      id: track.id,
      name: track.name,
      sourcePath: track.sourcePath,
      sourceCrs: track.sourceCrs,
      hash: track.hashSha256,
      startUtc: track.startUtc,
      endUtc: track.endUtc,
      pointCount: track.pointCount,
      segmentCount: track.statistics.segmentCount,
      distanceMeters: track.statistics.distanceMeters,
      elevationMin: track.statistics.minElevation,
      elevationMax: track.statistics.maxElevation,
      points: track.segments.flatMap((segment) =>
        segment.points.map((point, index) => ({
          id: `${segment.id}:${index}`,
          segmentId: segment.id,
          timeUtc: point.timeUtc,
          original: point.original,
          normalized: point.normalized,
          elevation: point.elevation,
          hdop: point.hdop,
        })),
      ),
    })),
    photos: raw.photos.map((photo, index) => ({
      id: photo.id,
      path: photo.path,
      relativePath: photo.relativePath,
      fileName: photo.fileName,
      captureLocal: photo.captureLocal,
      captureUtc: photo.captureUtc,
      timezoneSource:
        photo.timezoneSource === "METADATA_OFFSET"
          ? "EXIF_OFFSET"
          : photo.timezoneSource === "PROJECT_DEFAULT"
            ? "PROJECT_DEFAULT"
            : photo.timezoneSource === "USER_OVERRIDE"
              ? "USER_CONFIRMED"
              : "MISSING",
      existingGps: photo.existingGps,
      fileSize: photo.fingerprint.sizeBytes,
      modifiedAt: new Date(photo.fingerprint.modifiedUnixMs).toISOString(),
      sourceHash: photo.fingerprint.sha256,
      // Raw source paths are never exposed through an unrestricted asset
      // protocol. A future thumbnail command can return a scoped data URL.
      thumbnailUrl: undefined,
      thumbnailTone: [
        "#6f9180",
        "#7d8d66",
        "#a78c60",
        "#657f73",
        "#6d7d86",
      ][index % 5],
    })),
    matches: raw.matches.map((match) => ({
      photoId: match.photoId,
      trackId: match.trackId,
      lat: match.lat,
      lon: match.lon,
      elevation: match.elevation,
      method:
        match.method.toLocaleUpperCase() === "EXACT"
          ? "EXACT"
          : match.lat !== undefined
            ? "INTERPOLATED"
            : undefined,
      confidence: match.confidence ?? 0,
      status:
        match.qualityStatus ??
        (match.status === "ALREADY_HAS_GPS" ? "MATCHED_HIGH" : match.status),
      reason: match.reason || undefined,
      beforeDeltaSeconds: secondsBetween(
        match.matchedTimeUtc,
        match.previousPointTimeUtc,
      ),
      afterDeltaSeconds: secondsBetween(
        match.nextPointTimeUtc,
        match.matchedTimeUtc,
      ),
      estimatedErrorMeters: match.estimatedErrorMeters,
    })),
    writeHistory: raw.writeHistory.map(mapWriteJob).reverse(),
    dirty,
  };
}

async function currentSnapshot(): Promise<ProjectSnapshot> {
  const [raw, dirty] = await Promise.all([
    invokeDesktop<RawProjectSnapshot>("get_project_snapshot"),
    invokeDesktop<boolean>("is_project_dirty"),
  ]);
  return mapSnapshot(raw, dirty);
}

async function waitForTask(taskId: string): Promise<RawTaskRecord> {
  const deadline = Date.now() + 10 * 60_000;

  while (Date.now() < deadline) {
    const task = await invokeDesktop<RawTaskRecord>("get_task", { taskId });
    if (task.status === "COMPLETED") return task;
    if (task.status === "FAILED" || task.status === "CANCELLED") {
      throw task.error ?? new Error(task.message);
    }
    await new Promise((resolve) => setTimeout(resolve, 120));
  }

  throw new Error("TASK_TIMEOUT");
}

export async function chooseProjectFile(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  const selected = await open({
    title: "打开照片地理标记项目",
    multiple: false,
    directory: false,
    filters: [{ name: "GeoTagger 项目", extensions: ["json"] }],
  });
  return typeof selected === "string" ? selected : null;
}

export async function chooseProjectDirectory(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  const selected = await open({
    title: "选择项目目录",
    multiple: false,
    directory: true,
  });
  return typeof selected === "string" ? selected : null;
}

export async function chooseTrackFiles(): Promise<string[]> {
  if (!isTauriRuntime()) return [];
  const selected = await open({
    title: "导入 GPX 轨迹",
    multiple: true,
    directory: false,
    filters: [{ name: "GPX 轨迹", extensions: ["gpx"] }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function choosePhotoDirectory(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  const selected = await open({
    title: "选择照片目录",
    multiple: false,
    directory: true,
  });
  return typeof selected === "string" ? selected : null;
}

export async function createProject(request: {
  name: string;
  directory: string;
}): Promise<ProjectSnapshot> {
  if (!isTauriRuntime()) return createDemoProject();
  const summary = await invokeDesktop<RawProjectSummary>("create_project", {
    request: {
      name: request.name,
      projectDirectory: request.directory,
      defaultOutputDirectory: `${request.directory}/output`,
    },
  });
  activeProjectPath = summary.projectPath;
  return currentSnapshot();
}

export async function openProject(projectPath: string): Promise<ProjectSnapshot> {
  activeProjectPath = projectPath;
  const raw = await invokeDesktop<RawProjectSnapshot>("open_project", {
    projectPath,
  });
  return mapSnapshot(raw, false);
}

export async function saveProject(
  project: ProjectSnapshot,
): Promise<ProjectSnapshot> {
  if (!isTauriRuntime()) return { ...project, dirty: false };
  const latestRaw =
    rawSnapshot ??
    (await invokeDesktop<RawProjectSnapshot>("get_project_snapshot"));
  latestRaw.project.name = project.project.name;
  latestRaw.settings.photoTimezone = project.settings.photoTimezone;
  latestRaw.settings.fixedOffsetMs = project.settings.fixedOffsetMs;
  latestRaw.settings.defaultOutputDirectory = project.settings.outputDirectory;
  latestRaw.calibration.timezone = project.settings.photoTimezone;
  latestRaw.calibration.fixedOffsetMs = project.settings.fixedOffsetMs;
  rememberedExistingGpsPolicy = project.settings.existingGpsPolicy;
  rememberedAutomaticUpdates = project.settings.automaticUpdates;

  await invokeDesktop("save_project", {
    request: {
      projectPath: activeProjectPath || undefined,
      snapshot: latestRaw,
    },
  });
  return currentSnapshot();
}

export async function importTracks(
  paths: string[],
  sourceCrs: CoordinateSystem,
): Promise<ImportTrackResult> {
  const result = await invokeDesktop<{
    tracks: RawTrack[];
    warningCount: number;
  }>("import_tracks", {
    request: { paths, sourceCrs },
  });
  const mapped = await currentSnapshot();
  return {
    tracks: mapped.tracks.filter((track) =>
      result.tracks.some((rawTrack) => rawTrack.id === track.id),
    ),
    warnings:
      result.warningCount > 0
        ? [`轨迹解析完成，发现 ${result.warningCount} 项需要注意。`]
        : [],
  };
}

export async function scanPhotos(
  directory: string,
  recursive = true,
): Promise<ScanPhotosResult> {
  const accepted = await invokeDesktop<{ taskId: string }>("scan_photos", {
    request: { directory, recursive },
  });
  await waitForTask(accepted.taskId);
  let snapshot = await currentSnapshot();
  if (snapshot.photos.length > 0) {
    try {
      await invokeDesktop("read_photo_metadata", {
        request: {
          photoIds: snapshot.photos.map((photo) => photo.id),
          timezone: snapshot.settings.photoTimezone,
        },
      });
      snapshot = await currentSnapshot();
    } catch {
      // Scanned photos remain usable for manual time review when ExifTool is
      // unavailable; the desktop error is surfaced again before any write.
    }
  }
  return { taskId: accepted.taskId, photos: snapshot.photos };
}

export async function calculateMatches(request: {
  trackIds: string[];
  photoIds: string[];
  timezone: string;
  fixedOffsetMs: number;
}): Promise<MatchResult> {
  const accepted = await invokeDesktop<{ taskId: string }>(
    "calculate_matches",
    {
      request: {
        trackIds: request.trackIds,
        photoIds: request.photoIds,
        calibration: {
          timezone: request.timezone,
          fixedOffsetMs: request.fixedOffsetMs,
          syncPoints: [],
        },
      },
    },
  );
  await waitForTask(accepted.taskId);
  const snapshot = await currentSnapshot();
  const summary = { high: 0, medium: 0, low: 0, failed: 0 };
  for (const match of snapshot.matches) {
    if (match.status === "MATCHED_HIGH") summary.high += 1;
    else if (match.status === "MATCHED_MEDIUM") summary.medium += 1;
    else if (match.status === "MATCHED_LOW") summary.low += 1;
    else summary.failed += 1;
  }
  return { matches: snapshot.matches, summary };
}

export async function buildWritePlan(request: {
  photoIds: string[];
  outputDirectory: string;
  existingGpsPolicy: "SKIP" | "OVERWRITE" | "PRESERVE_COPY";
}): Promise<WritePlan> {
  rememberedExistingGpsPolicy = request.existingGpsPolicy;
  const raw = await invokeDesktop<RawWritePlan>("build_write_plan", {
    request: {
      photoIds: request.photoIds,
      outputDirectory: request.outputDirectory,
      options: {
        existingGpsPolicy:
          request.existingGpsPolicy === "PRESERVE_COPY"
            ? "PRESERVE"
            : request.existingGpsPolicy,
        includeAltitude: true,
        preserveRelativePaths: true,
        overwriteOutput: false,
      },
    },
  });
  return {
    id: raw.id,
    createdAt: raw.createdAt,
    outputDirectory: raw.outputDirectory,
    warnings: raw.items.flatMap((item) => item.warnings),
    sourceFilesUnchanged: raw.conflictCount === 0,
    items: raw.items.map((item) => ({
      photoId: item.photoId,
      sourcePath: item.sourcePath,
      outputPath: item.outputPath,
      action: item.action === "WRITE_GPS" ? "WRITE" : "SKIP",
      reason:
        item.action === "WRITE_GPS"
          ? undefined
          : item.warnings[0] ?? item.action.replaceAll("_", " "),
      oldGps: item.oldGps,
      newGps: item.newGps,
    })),
  };
}

export async function executeWritePlan(
  writePlanId: string,
): Promise<WriteExecutionResult> {
  const accepted = await invokeDesktop<{ taskId: string }>(
    "execute_write_plan",
    {
      request: { writePlanId },
    },
  );
  await waitForTask(accepted.taskId);
  await currentSnapshot();
  const rawJob = rawSnapshot?.writeHistory.at(-1);
  if (!rawJob) throw new Error("WRITE_JOB_MISSING");
  return {
    job: mapWriteJob(rawJob),
    failedPhotoIds: rawJob.results
      .filter((item) => item.status === "FAILED")
      .map((item) => item.photoId),
  };
}

export async function exportReport(
  format: "CSV" | "JSON",
  targetPath: string,
): Promise<{ path: string; rows: number }> {
  const result = await invokeDesktop<{
    targetPath: string;
    recordCount: number;
  }>("export_report", {
    request: { format, targetPath },
  });
  return { path: result.targetPath, rows: result.recordCount };
}

export async function cancelTask(taskId: string): Promise<boolean> {
  return invokeDesktop("cancel_task", { taskId });
}
