use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::{Display, Formatter};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ProjectInvalid,
    TrackParseFailed,
    TrackNoTime,
    CrsUnconfirmed,
    PhotoMetadataFailed,
    PhotoTimeAmbiguous,
    MatchNotFound,
    ExiftoolNotAvailable,
    WritePermissionDenied,
    WriteVerifyFailed,
    TaskCancelled,
    InvalidRequest,
    PathOutsideScope,
    OutputConflict,
    IoError,
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub suggestion: String,
    pub recoverable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl AppError {
    pub fn new(
        code: ErrorCode,
        message: impl Into<String>,
        suggestion: impl Into<String>,
        recoverable: bool,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            suggestion: suggestion.into(),
            recoverable,
            details: None,
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::InvalidRequest,
            message,
            "请检查输入后重试。",
            true,
        )
    }

    pub fn project_invalid(message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::ProjectInvalid,
            message,
            "请选择有效的 GeoTagger 项目文件，或从备份恢复。",
            true,
        )
    }

    pub fn io(context: impl Into<String>, error: impl Display) -> Self {
        Self::new(
            ErrorCode::IoError,
            format!("{}：{}", context.into(), error),
            "请确认文件仍存在且当前用户具有所需权限。",
            true,
        )
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::InternalError,
            message,
            "请导出诊断信息并重试。",
            false,
        )
    }

    pub fn cancelled() -> Self {
        Self::new(
            ErrorCode::TaskCancelled,
            "任务已取消。",
            "可以在确认输入文件未变化后安全重试。",
            true,
        )
    }
}

impl Display for AppError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}
