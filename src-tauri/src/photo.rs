use crate::domain::{Photo, PhotoMetadataStatus, TimezoneSource};
use crate::error::{AppError, AppResult};
use crate::fs_utils::{canonical_directory, fingerprint};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;
use walkdir::WalkDir;

const DEFAULT_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "tif", "tiff", "heic", "heif", "dng", "cr2", "cr3", "nef", "arw",
    "rw2", "orf", "raf", "png", "webp",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPhotosRequest {
    pub directory: String,
    #[serde(default = "default_recursive")]
    pub recursive: bool,
    #[serde(default)]
    pub extensions: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoScanWarning {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoScanResult {
    pub root_directory: String,
    pub photos: Vec<Photo>,
    pub warnings: Vec<PhotoScanWarning>,
}

pub fn scan_photo_directory<F>(
    request: &ScanPhotosRequest,
    cancelled: &AtomicBool,
    mut progress: F,
) -> AppResult<PhotoScanResult>
where
    F: FnMut(u64, u64, &str),
{
    if request.directory.trim().is_empty() || !Path::new(&request.directory).is_absolute() {
        return Err(AppError::invalid("照片目录必须是非空绝对路径。"));
    }
    let root = canonical_directory(Path::new(&request.directory))?;
    let extensions = normalized_extensions(request.extensions.as_deref());
    let walker = if request.recursive {
        WalkDir::new(&root).follow_links(false)
    } else {
        WalkDir::new(&root).follow_links(false).max_depth(1)
    };

    let mut paths = Vec::<PathBuf>::new();
    let mut warnings = Vec::new();
    for entry in walker.into_iter() {
        match entry {
            Ok(entry) if entry.file_type().is_file() && is_supported(entry.path(), &extensions) => {
                paths.push(entry.path().to_path_buf());
            }
            Ok(_) => {}
            Err(error) => warnings.push(PhotoScanWarning {
                path: error
                    .path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                message: error.to_string(),
            }),
        }
    }
    paths.sort_by(|left, right| {
        left.to_string_lossy()
            .cmp(&right.to_string_lossy())
    });

    let total = paths.len() as u64;
    let mut photos = Vec::with_capacity(paths.len());
    for (index, path) in paths.into_iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            return Err(AppError::cancelled());
        }
        match build_photo(&root, &path) {
            Ok(photo) => photos.push(photo),
            Err(error) => warnings.push(PhotoScanWarning {
                path: path.to_string_lossy().into_owned(),
                message: error.message,
            }),
        }
        progress(
            index as u64 + 1,
            total,
            &format!(
                "正在扫描 {}",
                path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("照片")
            ),
        );
    }

    Ok(PhotoScanResult {
        root_directory: root.to_string_lossy().into_owned(),
        photos,
        warnings,
    })
}

fn build_photo(root: &Path, path: &Path) -> AppResult<Photo> {
    let canonical = path
        .canonicalize()
        .map_err(|error| AppError::io("无法访问照片", error))?;
    let fingerprint = fingerprint(&canonical)?;
    let relative = canonical
        .strip_prefix(root)
        .map_err(|_| AppError::invalid("照片不在所选扫描目录内。"))?;
    let relative_path = relative.to_string_lossy().into_owned();
    let file_name = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AppError::invalid("照片文件名不是有效 Unicode。"))?
        .to_owned();
    let extension = canonical
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let stable_key = format!(
        "{}|{}|{}",
        canonical.to_string_lossy(),
        fingerprint.sha256,
        fingerprint.size_bytes
    );
    Ok(Photo {
        id: Uuid::new_v5(&Uuid::NAMESPACE_URL, stable_key.as_bytes()),
        path: canonical.to_string_lossy().into_owned(),
        relative_path,
        file_name,
        extension,
        fingerprint,
        capture_local: None,
        capture_utc: None,
        timezone_source: TimezoneSource::Unknown,
        existing_gps: None,
        // The source path is persisted so the frontend can request a thumbnail
        // through a narrowly scoped application command without copying the
        // original into a web-accessible directory.
        thumbnail: Some(canonical.to_string_lossy().into_owned()),
        metadata_status: PhotoMetadataStatus::Pending,
        metadata_error: None,
    })
}

fn normalized_extensions(values: Option<&[String]>) -> BTreeSet<String> {
    match values {
        Some(values) if !values.is_empty() => values
            .iter()
            .map(|value| value.trim_start_matches('.').to_ascii_lowercase())
            .collect(),
        _ => DEFAULT_EXTENSIONS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    }
}

fn is_supported(path: &Path, extensions: &BTreeSet<String>) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| extensions.contains(&value.to_ascii_lowercase()))
        .unwrap_or(false)
}

const fn default_recursive() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_supported_extensions_case_insensitively() {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join("one.JPG"), b"jpeg").expect("fixture");
        std::fs::write(directory.path().join("ignore.txt"), b"text").expect("fixture");
        let cancelled = AtomicBool::new(false);
        let result = scan_photo_directory(
            &ScanPhotosRequest {
                directory: directory.path().to_string_lossy().into_owned(),
                recursive: true,
                extensions: None,
            },
            &cancelled,
            |_, _, _| {},
        )
        .expect("scan");
        assert_eq!(result.photos.len(), 1);
        assert_eq!(result.photos[0].extension, "jpg");
    }

    #[test]
    fn respects_non_recursive_scan() {
        let directory = tempfile::tempdir().expect("temp dir");
        let nested = directory.path().join("nested");
        std::fs::create_dir(&nested).expect("nested");
        std::fs::write(nested.join("hidden.jpg"), b"jpeg").expect("fixture");
        let cancelled = AtomicBool::new(false);
        let result = scan_photo_directory(
            &ScanPhotosRequest {
                directory: directory.path().to_string_lossy().into_owned(),
                recursive: false,
                extensions: None,
            },
            &cancelled,
            |_, _, _| {},
        )
        .expect("scan");
        assert!(result.photos.is_empty());
    }
}
