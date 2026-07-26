use crate::error::{AppError, AppResult, ErrorCode};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/portable_exiftool_payload.rs"));
}

const ARCHIVE_MAGIC: &[u8; 8] = b"GTEXIF01";

pub fn prepare(app_local_data_dir: &Path) -> AppResult<PathBuf> {
    verify_payload()?;
    let payload_id = format!(
        "{}-{}",
        embedded::EXIFTOOL_VERSION,
        &embedded::EXIFTOOL_PAYLOAD_SHA256[..16]
    );
    let root = app_local_data_dir.join("portable-exiftool");
    let destination = root.join(&payload_id);
    if validate_installation(&destination) {
        return Ok(destination);
    }

    if destination.exists() {
        fs::remove_dir_all(&destination)
            .map_err(|error| portable_error("无法移除损坏的 ExifTool 缓存", error))?;
    }
    fs::create_dir_all(&root)
        .map_err(|error| portable_error("无法创建 ExifTool 缓存目录", error))?;

    let staging = root.join(format!(".{payload_id}.tmp-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|error| portable_error("无法清理未完成的 ExifTool 缓存", error))?;
    }
    if let Err(error) = extract_payload(&staging).and_then(|_| write_marker(&staging)) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    match fs::rename(&staging, &destination) {
        Ok(()) => Ok(destination),
        Err(_) if validate_installation(&destination) => {
            let _ = fs::remove_dir_all(&staging);
            Ok(destination)
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            Err(portable_error("无法完成 ExifTool 缓存初始化", error))
        }
    }
}

pub fn smoke_test(app_local_data_dir: &Path) -> AppResult<()> {
    let root = prepare(app_local_data_dir)?;
    if !validate_installation(&root) {
        return Err(AppError::new(
            ErrorCode::ExiftoolNotAvailable,
            "内嵌 ExifTool 自检失败。",
            "请重新下载完整的 Windows 便携版 EXE。",
            true,
        ));
    }
    Ok(())
}

fn verify_payload() -> AppResult<()> {
    let actual = format!("{:x}", Sha256::digest(embedded::EXIFTOOL_PAYLOAD));
    if actual == embedded::EXIFTOOL_PAYLOAD_SHA256 {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::ExiftoolNotAvailable,
            "内嵌 ExifTool 载荷校验失败。",
            "请重新下载完整的 Windows 便携版 EXE。",
            true,
        ))
    }
}

fn extract_payload(destination: &Path) -> AppResult<()> {
    fs::create_dir_all(destination)
        .map_err(|error| portable_error("无法创建 ExifTool 临时目录", error))?;
    let mut offset = 0usize;
    let payload = embedded::EXIFTOOL_PAYLOAD;
    if take(payload, &mut offset, ARCHIVE_MAGIC.len())? != ARCHIVE_MAGIC {
        return Err(payload_error("内嵌 ExifTool 载荷格式无效。"));
    }
    let entry_count = read_u32(payload, &mut offset)?;
    for _ in 0..entry_count {
        let path_length = read_u16(payload, &mut offset)? as usize;
        let path = std::str::from_utf8(take(payload, &mut offset, path_length)?)
            .map_err(|_| payload_error("内嵌 ExifTool 载荷包含无效路径。"))?;
        let length = usize::try_from(read_u64(payload, &mut offset)?)
            .map_err(|_| payload_error("内嵌 ExifTool 文件过大。"))?;
        let expected_hash = take(payload, &mut offset, 32)?;
        let contents = take(payload, &mut offset, length)?;
        if Sha256::digest(contents).as_slice() != expected_hash {
            return Err(payload_error("内嵌 ExifTool 文件校验失败。"));
        }
        let relative_path = safe_relative_path(path)?;
        let target = destination.join("exiftool").join(relative_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| portable_error("无法创建 ExifTool 文件目录", error))?;
        }
        let mut file = File::create(&target)
            .map_err(|error| portable_error("无法写入 ExifTool 文件", error))?;
        file.write_all(contents)
            .map_err(|error| portable_error("无法写入 ExifTool 文件", error))?;
        file.flush()
            .map_err(|error| portable_error("无法完成 ExifTool 文件写入", error))?;
    }
    if offset != payload.len() {
        return Err(payload_error("内嵌 ExifTool 载荷包含多余数据。"));
    }
    Ok(())
}

fn write_marker(destination: &Path) -> AppResult<()> {
    fs::write(
        destination.join(".complete"),
        format!(
            "version={}\npayload={}\n",
            embedded::EXIFTOOL_VERSION,
            embedded::EXIFTOOL_PAYLOAD_SHA256
        ),
    )
    .map_err(|error| portable_error("无法写入 ExifTool 缓存标记", error))
}

fn validate_installation(destination: &Path) -> bool {
    let expected_marker = format!(
        "version={}\npayload={}\n",
        embedded::EXIFTOOL_VERSION,
        embedded::EXIFTOOL_PAYLOAD_SHA256
    );
    if fs::read_to_string(destination.join(".complete"))
        .ok()
        .as_deref()
        != Some(&expected_marker)
    {
        return false;
    }
    let executable = destination.join("exiftool").join("exiftool.exe");
    if !executable.is_file()
        || !destination
            .join("exiftool")
            .join("exiftool_files")
            .is_dir()
    {
        return false;
    }
    if !payload_files_match(destination) {
        return false;
    }
    Command::new(executable)
        .arg("-ver")
        .stdin(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout).trim() == embedded::EXIFTOOL_VERSION
        })
        .unwrap_or(false)
}

fn payload_files_match(destination: &Path) -> bool {
    let mut offset = 0usize;
    let payload = embedded::EXIFTOOL_PAYLOAD;
    let Ok(magic) = take(payload, &mut offset, ARCHIVE_MAGIC.len()) else {
        return false;
    };
    if magic != ARCHIVE_MAGIC {
        return false;
    }
    let Ok(entry_count) = read_u32(payload, &mut offset) else {
        return false;
    };
    for _ in 0..entry_count {
        let Ok(path_length) = read_u16(payload, &mut offset) else {
            return false;
        };
        let Ok(path_bytes) = take(payload, &mut offset, path_length as usize) else {
            return false;
        };
        let Ok(path) = std::str::from_utf8(path_bytes) else {
            return false;
        };
        let Ok(length) = read_u64(payload, &mut offset) else {
            return false;
        };
        let Ok(length) = usize::try_from(length) else {
            return false;
        };
        let Ok(expected_hash) = take(payload, &mut offset, 32) else {
            return false;
        };
        if take(payload, &mut offset, length).is_err() {
            return false;
        }
        let Ok(relative_path) = safe_relative_path(path) else {
            return false;
        };
        let Ok(contents) = fs::read(destination.join("exiftool").join(relative_path)) else {
            return false;
        };
        if Sha256::digest(contents).as_slice() != expected_hash {
            return false;
        }
    }
    offset == payload.len()
}

fn safe_relative_path(value: &str) -> AppResult<PathBuf> {
    let path = Path::new(value);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(payload_error("内嵌 ExifTool 载荷包含不安全路径。"));
    }
    Ok(path.to_path_buf())
}

fn take<'a>(payload: &'a [u8], offset: &mut usize, length: usize) -> AppResult<&'a [u8]> {
    let end = offset
        .checked_add(length)
        .filter(|end| *end <= payload.len())
        .ok_or_else(|| payload_error("内嵌 ExifTool 载荷意外结束。"))?;
    let value = &payload[*offset..end];
    *offset = end;
    Ok(value)
}

fn read_u16(payload: &[u8], offset: &mut usize) -> AppResult<u16> {
    let bytes: [u8; 2] = take(payload, offset, 2)?.try_into().expect("fixed length");
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(payload: &[u8], offset: &mut usize) -> AppResult<u32> {
    let bytes: [u8; 4] = take(payload, offset, 4)?.try_into().expect("fixed length");
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> AppResult<u64> {
    let bytes: [u8; 8] = take(payload, offset, 8)?.try_into().expect("fixed length");
    Ok(u64::from_le_bytes(bytes))
}

fn payload_error(message: impl Into<String>) -> AppError {
    AppError::new(
        ErrorCode::ExiftoolNotAvailable,
        message,
        "请重新下载完整的 Windows 便携版 EXE。",
        true,
    )
}

fn portable_error(context: impl Into<String>, error: io::Error) -> AppError {
    AppError::new(
        ErrorCode::ExiftoolNotAvailable,
        format!("{}：{error}", context.into()),
        "请确认当前用户可以写入本地应用数据目录后重试。",
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_payload_paths() {
        assert!(safe_relative_path("../exiftool.exe").is_err());
        assert!(safe_relative_path("C:\\exiftool.exe").is_err());
        assert!(safe_relative_path("exiftool_files/config").is_ok());
    }

    #[test]
    fn extracts_and_reuses_embedded_payload() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let first = prepare(directory.path()).expect("first extraction");
        assert!(validate_installation(&first));
        let second = prepare(directory.path()).expect("cache reuse");
        assert_eq!(first, second);
    }

    #[test]
    fn rebuilds_a_tampered_cache() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = prepare(directory.path()).expect("first extraction");
        fs::write(root.join(".complete"), "tampered\n").expect("tamper marker");
        let rebuilt = prepare(directory.path()).expect("cache rebuild");
        assert_eq!(root, rebuilt);
        assert!(validate_installation(&rebuilt));
    }
}
