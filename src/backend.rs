extern crate alloc;

use alloc::string::{String, ToString};
use core::fmt;

use crate::io::ProviderError;

pub type BackendResult<T> = Result<T, BackendError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendErrorKind {
    HostInit,
    Asset,
    Render,
    Storage,
    Dom,
    State,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendError {
    pub kind: BackendErrorKind,
    pub message: String,
}

impl BackendError {
    pub fn new(kind: BackendErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn host_init(message: impl Into<String>) -> Self {
        Self::new(BackendErrorKind::HostInit, message)
    }

    pub fn asset(message: impl Into<String>) -> Self {
        Self::new(BackendErrorKind::Asset, message)
    }

    pub fn render(message: impl Into<String>) -> Self {
        Self::new(BackendErrorKind::Render, message)
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::new(BackendErrorKind::Storage, message)
    }

    pub fn dom(message: impl Into<String>) -> Self {
        Self::new(BackendErrorKind::Dom, message)
    }

    pub fn state(message: impl Into<String>) -> Self {
        Self::new(BackendErrorKind::State, message)
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(BackendErrorKind::Unsupported, message)
    }

    pub fn from_provider(kind: BackendErrorKind, context: &str, error: ProviderError) -> Self {
        Self::new(kind, alloc::format!("{context}: {error}"))
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

#[cfg(not(feature = "uefi"))]
impl std::error::Error for BackendError {}

impl From<&str> for BackendError {
    fn from(value: &str) -> Self {
        Self::new(BackendErrorKind::State, value.to_string())
    }
}
