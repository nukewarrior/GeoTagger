# GeoTagger Tauri backend

This directory contains the Tauri 2 shell and the Rust MVP-1 application
service. The repository policy for this project is intentionally strict:

- do not install Rust, Node, native libraries, ExifTool, PROJ, or any other
  dependency on the development Mac;
- do not run Cargo, Tauri, npm, lint, typecheck, tests, or builds locally;
- dependency resolution, tests, packaging, sidecar preparation, signing, and
  notarization run only in GitHub Actions.

## Runtime boundaries

- All persisted and EXIF GPS coordinates are WGS84.
- GCJ-02 and BD-09 conversion occurs in the Rust coordinate module.
- GPX segments remain independent; the matcher never interpolates across a
  segment boundary.
- Original photos are read-only. A write plan copies each allowed item to a
  temporary file in the selected output directory, writes and verifies that
  copy, then commits it with an atomic rename.
- ExifTool is never discovered from `PATH` and is never downloaded or installed
  at runtime. macOS uses the bundled application resource directory; the Windows
  portable EXE embeds the verified payload and extracts it on first launch into
  the current user's local application-data cache. Missing or non-runnable
  resources return `EXIFTOOL_NOT_AVAILABLE`.

## Commands

The registered commands are:

`create_project`, `open_project`, `get_project_snapshot`,
`get_project_summary`, `is_project_dirty`, `save_project`, `import_tracks`,
`preview_coordinate_conversion`, `scan_photos`, `read_photo_metadata`,
`calculate_matches`, `build_write_plan`, `execute_write_plan`, `cancel_task`,
`get_task`, `list_tasks`, `export_report`, and `get_exiftool_status`.

Long scans, match calculations, and write jobs return a `taskId` and emit
`task://progress`, `task://finished`, or `task://failed`. Project mutations
emit `project://dirty`.

### IPC request and response shapes

Tauri's JavaScript call wraps each structured input in `request`, unless the
table shows a direct argument.

| Command | JavaScript arguments | Immediate result |
| --- | --- | --- |
| `create_project` | `{ request: { name, projectDirectory, defaultOutputDirectory? } }` (`directory` is accepted as an alias) | `ProjectSummary` |
| `open_project` | `{ projectPath }` | `ProjectSnapshot` |
| `get_project_snapshot` | `{}` | `ProjectSnapshot` |
| `get_project_summary` | `{}` | `ProjectSummary` |
| `is_project_dirty` | `{}` | `boolean` |
| `save_project` | `{ request: { projectPath?, snapshot? } }` | `SaveResult` |
| `import_tracks` | `{ request: { paths, sourceCrs } }` | `{ tracks, warningCount }` |
| `preview_coordinate_conversion` | `{ request: { trackId, sourceCrs, limit? } }` | `ConvertedPreview` |
| `scan_photos` | `{ request: { directory, recursive?, extensions? } }` | `{ taskId }` |
| `read_photo_metadata` | `{ request: { photoIds, timezone? } }` | `PhotoMetadata[]` |
| `calculate_matches` | `{ request: { trackIds?, photoIds?, calibration } }` | `{ taskId }` |
| `build_write_plan` | `{ request: { photoIds, outputDirectory, options } }` | `WritePlan` |
| `execute_write_plan` | `{ request: { writePlanId } }` | `{ taskId }` |
| `cancel_task` | `{ taskId }` | `boolean` |
| `get_task` | `{ taskId }` | `TaskRecord \| null` |
| `list_tasks` | `{}` | `TaskRecord[]` |
| `export_report` | `{ request: { format, targetPath } }` | `ExportResult` |
| `get_exiftool_status` | `{}` | `ExifToolStatus` |

`calibration` currently accepts `{ timezone, fixedOffsetMs, syncPoints: [],
driftModel: null }`. MVP-1 deliberately rejects non-empty sync points or a
drift model. `build_write_plan.options` is
`{ existingGpsPolicy, includeAltitude, preserveRelativePaths,
overwriteOutput }`.

## ExifTool bundle layout

The CI release job prepares a pinned, checksum-verified executable at one of:

- `resources/exiftool/exiftool` for macOS/Linux;
- `resources/exiftool/exiftool.exe` for Windows.

Any support files required by that platform build stay below the same
directory. macOS copies the complete directory into application resources.
Windows embeds the complete directory in the distributed EXE, then safely
extracts it to a version-and-payload-hash-specific local cache on first launch.
