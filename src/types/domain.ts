export type CoordinateSystem = "WGS84" | "GCJ02" | "BD09" | "UNKNOWN";

export type MatchStatus =
  | "MATCHED_HIGH"
  | "MATCHED_MEDIUM"
  | "MATCHED_LOW"
  | "OUT_OF_RANGE"
  | "NO_CAPTURE_TIME"
  | "SEGMENT_GAP";

export type AppView =
  | "welcome"
  | "import"
  | "workspace"
  | "write"
  | "tasks"
  | "settings";

export type TaskStatus = "QUEUED" | "RUNNING" | "COMPLETED" | "FAILED" | "CANCELLED";

export interface GeoPoint {
  lat: number;
  lon: number;
}

export interface TrackPoint {
  id: string;
  segmentId: string;
  timeUtc: string;
  original: GeoPoint & { crs: CoordinateSystem };
  normalized: GeoPoint;
  elevation?: number;
  hdop?: number;
}

export interface TrackSummary {
  id: string;
  name: string;
  sourcePath: string;
  sourceCrs: CoordinateSystem;
  hash: string;
  startUtc: string;
  endUtc: string;
  pointCount: number;
  segmentCount: number;
  distanceMeters: number;
  elevationMin?: number;
  elevationMax?: number;
  points: TrackPoint[];
}

export interface ExistingGps extends GeoPoint {
  altitude?: number;
}

export interface PhotoRecord {
  id: string;
  path: string;
  relativePath: string;
  fileName: string;
  captureLocal?: string;
  captureUtc?: string;
  timezoneSource: "EXIF_OFFSET" | "PROJECT_DEFAULT" | "USER_CONFIRMED" | "MISSING";
  existingGps?: ExistingGps;
  fileSize: number;
  modifiedAt: string;
  sourceHash?: string;
  thumbnailUrl?: string;
  thumbnailTone?: string;
}

export interface PhotoMatch {
  photoId: string;
  trackId?: string;
  lat?: number;
  lon?: number;
  elevation?: number;
  method?: "EXACT" | "INTERPOLATED";
  confidence: number;
  status: MatchStatus;
  reason?: string;
  beforeDeltaSeconds?: number;
  afterDeltaSeconds?: number;
  estimatedErrorMeters?: number;
}

export interface ProjectSettings {
  photoTimezone: string;
  fixedOffsetMs: number;
  mapProvider: "maplibre";
  mapStyleUrl?: string;
  outputMode: "COPY_TO_DIRECTORY";
  outputDirectory?: string;
  existingGpsPolicy: "SKIP" | "OVERWRITE" | "PRESERVE_COPY";
  automaticUpdates: boolean;
}

export interface ProjectMeta {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  directory: string;
  projectFile?: string;
}

export interface ProjectSnapshot {
  schemaVersion: 1;
  project: ProjectMeta;
  settings: ProjectSettings;
  tracks: TrackSummary[];
  photos: PhotoRecord[];
  matches: PhotoMatch[];
  writeHistory: WriteJobSummary[];
  dirty: boolean;
}

export interface WritePlanItem {
  photoId: string;
  sourcePath: string;
  outputPath: string;
  action: "WRITE" | "SKIP";
  reason?: string;
  oldGps?: ExistingGps;
  newGps?: ExistingGps;
}

export interface WritePlan {
  id: string;
  createdAt: string;
  outputDirectory: string;
  items: WritePlanItem[];
  warnings: string[];
  sourceFilesUnchanged: boolean;
}

export interface WriteJobSummary {
  id: string;
  writePlanId: string;
  status: TaskStatus;
  startedAt: string;
  finishedAt?: string;
  succeeded: number;
  failed: number;
}

export interface TaskInfo {
  id: string;
  label: string;
  stage: string;
  status: TaskStatus;
  completed: number;
  total: number;
  message: string;
  startedAt: string;
  warnings: string[];
}

export interface AppError {
  code:
    | "PROJECT_INVALID"
    | "TRACK_PARSE_FAILED"
    | "TRACK_NO_TIME"
    | "CRS_UNCONFIRMED"
    | "PHOTO_METADATA_FAILED"
    | "PHOTO_TIME_AMBIGUOUS"
    | "MATCH_NOT_FOUND"
    | "EXIFTOOL_NOT_AVAILABLE"
    | "WRITE_PERMISSION_DENIED"
    | "WRITE_VERIFY_FAILED"
    | "TASK_CANCELLED";
  message: string;
  suggestion: string;
}

export interface ImportTrackResult {
  tracks: TrackSummary[];
  warnings: string[];
}

export interface ScanPhotosResult {
  taskId?: string;
  photos?: PhotoRecord[];
}

export interface MatchResult {
  matches: PhotoMatch[];
  summary: {
    high: number;
    medium: number;
    low: number;
    failed: number;
  };
}

export interface WriteExecutionResult {
  job: WriteJobSummary;
  failedPhotoIds: string[];
}
