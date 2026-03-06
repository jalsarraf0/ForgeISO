use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("policy violation: {0}")]
    PolicyViolation(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("tooling missing: {0}")]
    MissingTool(String),
    #[error("filesystem safety violation: {0}")]
    PathSafety(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("yaml error: {0}")]
    SerdeYaml(#[from] serde_yaml::Error),
    #[error("http error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

pub type EngineResult<T> = Result<T, EngineError>;
