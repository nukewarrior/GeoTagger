# ExifTool runtime resource

The application never downloads or installs ExifTool at runtime.

Release workflows place the pinned, checksum-verified platform executable and
its required support files in this directory before the Tauri bundle step.
The application searches only its fixed bundled resource locations. If no
executable is present, commands return `EXIFTOOL_NOT_AVAILABLE`.

