use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StepId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpInfo {
    pub name: String,
    pub weight_bytes: u64,
    pub requires_elevation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}: {message}", .code.orki_code())]
pub struct OpError {
    pub code: ErrorCode,
    pub message: String,
}

impl OpError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    BadPayload,
    BadSignature,
    NoDiskSpace,
    AccessDenied,
    FilesLocked,
    PrereqMissing,
    DownloadFailed,
    GpuFailure,
    ScriptError,
    Internal,
}

impl ErrorCode {
    pub fn orki_code(self) -> &'static str {
        match self {
            Self::BadPayload => "ORKI-1001",
            Self::BadSignature => "ORKI-1002",
            Self::NoDiskSpace => "ORKI-2001",
            Self::AccessDenied => "ORKI-2002",
            Self::FilesLocked => "ORKI-2003",
            Self::PrereqMissing => "ORKI-3001",
            Self::DownloadFailed => "ORKI-3002",
            Self::GpuFailure => "ORKI-4001",
            Self::ScriptError => "ORKI-5001",
            Self::Internal => "ORKI-9000",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Ctx {
    pub install_dir: PathBuf,
}

#[derive(Debug, Default)]
pub struct EventSink;

impl EventSink {
    pub fn progress(&self, _step: StepId, _done: u64, _total: u64) {}
    pub fn log(&self, _message: &str) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepInfo {
    pub id: StepId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Cancelled,
    Failed,
    RebootRequired,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    PlanReady { steps: Vec<StepInfo>, bytes: u64 },
    StepStarted(StepId),
    StepProgress { id: StepId, done: u64, total: u64 },
    Log(Arc<str>),
    NeedsElevation,
    RebootRequired,
    Finished(Outcome),
    RolledBack { cause: String },
}

pub trait Operation: Send {
    fn describe(&self) -> OpInfo;
    fn validate(&self, cx: &Ctx) -> Result<(), OpError>;
    fn apply(&mut self, cx: &mut Ctx, ev: &EventSink) -> Result<(), OpError>;
    fn rollback(&mut self, cx: &mut Ctx) -> Result<(), OpError>;
    fn verify(&self, _cx: &Ctx) -> Result<(), OpError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorCode, OpError};

    #[test]
    fn error_codes_match_catalog() {
        assert_eq!(ErrorCode::BadPayload.orki_code(), "ORKI-1001");
        assert_eq!(ErrorCode::FilesLocked.orki_code(), "ORKI-2003");
        assert_eq!(ErrorCode::Internal.orki_code(), "ORKI-9000");
        let e = OpError::new(ErrorCode::NoDiskSpace, "no space left");
        assert_eq!(e.to_string(), "ORKI-2001: no space left");
    }
}
