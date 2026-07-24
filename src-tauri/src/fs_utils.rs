use crate::domain::FileFingerprint;
use crate::error::{AppError, AppResult, ErrorCode};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

pub fn canonical_file(path: &Path) -> AppResult<PathBuf> {
    let canonical = path
        .canonicalize()
        .map_err(|error| AppError::io("无法访问文件", error))?;
    if !canonical.is_file() {
        return Err(AppError::invalid("所选路径不是文件。"));
    }
    Ok(canonical)
}

pub fn canonical_directory(path: &Path) -> AppResult<PathBuf> {
    let canonical = path
        .canonicalize()
        .map_err(|error| AppError::io("无法访问目录", error))?;
    if !canonical.is_dir() {
        return Err(AppError::invalid("所选路径不是目录。"));
    }
    Ok(canonical)
}

pub fn normalize_absolute(path: &Path) -> AppResult<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| AppError::io("无法确定当前目录", error))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(AppError::new(
                        ErrorCode::PathOutsideScope,
                        "路径包含越界的上级目录。",
                        "请选择明确的项目或输出目录。",
                        true,
                    ));
                }
            }
            Component::Normal(value) => normalized.push(value),
        }
    }
    Ok(normalized)
}

pub fn resolved_identity(path: &Path) -> AppResult<PathBuf> {
    let normalized = normalize_absolute(path)?;
    if normalized.exists() {
        return normalized
            .canonicalize()
            .map_err(|error| AppError::io("无法解析文件路径", error));
    }

    let mut existing = normalized.as_path();
    let mut suffix = Vec::new();
    while !existing.exists() {
        let name = existing
            .file_name()
            .ok_or_else(|| AppError::invalid("无法解析目标路径。"))?;
        suffix.push(name.to_os_string());
        existing = existing
            .parent()
            .ok_or_else(|| AppError::invalid("目标路径缺少可访问的上级目录。"))?;
    }
    let mut resolved = existing
        .canonicalize()
        .map_err(|error| AppError::io("无法解析目标上级目录", error))?;
    for component in suffix.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

pub fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = File::open(path).map_err(|error| AppError::io("无法读取文件", error))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| AppError::io("读取文件失败", error))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn fingerprint(path: &Path) -> AppResult<FileFingerprint> {
    let metadata = fs::metadata(path).map_err(|error| AppError::io("无法读取文件属性", error))?;
    let modified: DateTime<Utc> = metadata
        .modified()
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| DateTime::<Utc>::from(std::time::UNIX_EPOCH));
    Ok(FileFingerprint {
        sha256: sha256_file(path)?,
        size_bytes: metadata.len(),
        modified_unix_ms: modified.timestamp_millis(),
    })
}

pub fn fingerprint_matches(path: &Path, expected: &FileFingerprint) -> AppResult<bool> {
    let metadata = fs::metadata(path).map_err(|error| AppError::io("无法读取源文件属性", error))?;
    let modified: DateTime<Utc> = metadata
        .modified()
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| DateTime::<Utc>::from(std::time::UNIX_EPOCH));
    if metadata.len() != expected.size_bytes
        || modified.timestamp_millis() != expected.modified_unix_ms
    {
        return Ok(false);
    }
    Ok(sha256_file(path)? == expected.sha256)
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let normalized = normalize_absolute(path)?;
    let parent = normalized
        .parent()
        .ok_or_else(|| AppError::invalid("目标文件缺少父目录。"))?;
    fs::create_dir_all(parent).map_err(|error| {
        AppError::new(
            ErrorCode::WritePermissionDenied,
            format!("无法创建目标目录：{error}"),
            "请选择有写权限的独立输出目录。",
            true,
        )
    })?;

    let file_name = normalized
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("geotagger-data");
    let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    let write_result = (|| -> AppResult<()> {
        let mut file = File::create(&temporary).map_err(|error| {
            AppError::new(
                ErrorCode::WritePermissionDenied,
                format!("无法创建临时文件：{error}"),
                "请选择有写权限的独立输出目录。",
                true,
            )
        })?;
        file.write_all(bytes)
            .map_err(|error| AppError::io("写入临时文件失败", error))?;
        file.sync_all()
            .map_err(|error| AppError::io("同步临时文件失败", error))?;
        atomic_replace(&temporary, &normalized, true)
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

pub fn atomic_replace(temporary: &Path, target: &Path, overwrite: bool) -> AppResult<()> {
    if !target.exists() {
        return fs::rename(temporary, target)
            .map_err(|error| AppError::io("原子提交文件失败", error));
    }
    if !overwrite {
        return Err(AppError::new(
            ErrorCode::OutputConflict,
            format!("输出文件已存在：{}", target.display()),
            "请更改输出目录、启用明确覆盖，或从写入计划中移除冲突项。",
            true,
        ));
    }

    let parent = target
        .parent()
        .ok_or_else(|| AppError::invalid("目标文件缺少父目录。"))?;
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let backup = parent.join(format!(".{file_name}.{}.backup", Uuid::new_v4()));
    fs::rename(target, &backup).map_err(|error| AppError::io("备份现有输出失败", error))?;
    match fs::rename(temporary, target) {
        Ok(()) => {
            let _ = fs::remove_file(backup);
            Ok(())
        }
        Err(error) => {
            let _ = fs::rename(&backup, target);
            Err(AppError::io("原子替换输出失败", error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_complete_content() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("project.json");
        write_atomic(&path, b"first").expect("first write");
        write_atomic(&path, b"second").expect("replacement");
        assert_eq!(std::fs::read(&path).expect("read"), b"second");
    }

    #[test]
    fn normalize_rejects_parent_escape() {
        let root = if cfg!(windows) {
            PathBuf::from(r"C:\")
        } else {
            PathBuf::from("/")
        };
        assert!(normalize_absolute(&root.join("..").join("escape")).is_err());
    }
}
