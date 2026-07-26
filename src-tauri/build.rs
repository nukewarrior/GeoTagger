use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const PORTABLE_ARCHIVE_MAGIC: &[u8; 8] = b"GTEXIF01";
const WINDOWS_EXIFTOOL_VERSION: &str = "13.59";

fn main() {
    println!("cargo:rerun-if-changed=resources/exiftool");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_exiftool().expect("failed to embed Windows ExifTool payload");
    }

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "create_project",
            "open_project",
            "get_project_snapshot",
            "get_project_summary",
            "is_project_dirty",
            "save_project",
            "import_tracks",
            "preview_coordinate_conversion",
            "scan_photos",
            "read_photo_metadata",
            "calculate_matches",
            "build_write_plan",
            "execute_write_plan",
            "cancel_task",
            "get_task",
            "list_tasks",
            "export_report",
            "get_exiftool_status",
        ]),
    ))
    .expect("failed to generate Tauri build context")
}

fn embed_windows_exiftool() -> io::Result<()> {
    let source_root = PathBuf::from("resources/exiftool");
    let executable = source_root.join("exiftool.exe");
    let support_directory = source_root.join("exiftool_files");
    if !executable.is_file() || !support_directory.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Windows ExifTool payload is missing; run the CI preparation step before building",
        ));
    }

    let mut files = Vec::new();
    collect_files(&source_root, &source_root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut payload = Vec::new();
    payload.extend_from_slice(PORTABLE_ARCHIVE_MAGIC);
    payload.extend_from_slice(&(files.len() as u32).to_le_bytes());
    for (relative_path, absolute_path) in files {
        let contents = fs::read(&absolute_path)?;
        let path = relative_path.as_bytes();
        let path_length = u16::try_from(path.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("ExifTool resource path is too long: {relative_path}"),
            )
        })?;
        payload.extend_from_slice(&path_length.to_le_bytes());
        payload.extend_from_slice(path);
        payload.extend_from_slice(&(contents.len() as u64).to_le_bytes());
        payload.extend_from_slice(&Sha256::digest(&contents));
        payload.extend_from_slice(&contents);
        println!("cargo:rerun-if-changed={}", absolute_path.display());
    }

    let payload_hash = format!("{:x}", Sha256::digest(&payload));
    let output_directory = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let payload_path = output_directory.join("embedded-exiftool.bin");
    let generated_module_path = output_directory.join("portable_exiftool_payload.rs");
    fs::write(&payload_path, payload)?;
    let mut generated_module = fs::File::create(generated_module_path)?;
    writeln!(
        generated_module,
        "pub const EXIFTOOL_PAYLOAD: &[u8] = include_bytes!({:?});",
        payload_path
    )?;
    writeln!(
        generated_module,
        "pub const EXIFTOOL_PAYLOAD_SHA256: &str = \"{payload_hash}\";"
    )?;
    writeln!(
        generated_module,
        "pub const EXIFTOOL_VERSION: &str = \"{WINDOWS_EXIFTOOL_VERSION}\";"
    )?;
    Ok(())
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
        } else if file_type.is_file() {
            let relative = path.strip_prefix(root).expect("resource is beneath root");
            let normalized = relative.to_string_lossy().replace('\\', "/");
            files.push((normalized, path));
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ExifTool payload contains unsupported filesystem entry: {}",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}
