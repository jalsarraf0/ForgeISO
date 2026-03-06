use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::config::RuntimePreference;
use crate::error::{EngineError, EngineResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRunSpec {
    pub image: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub mounts: Vec<(PathBuf, PathBuf)>,
    pub workdir: Option<PathBuf>,
    pub uid_gid_map: Option<(u32, u32)>,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub program: String,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeManager {
    preference: RuntimePreference,
}

impl RuntimeManager {
    pub fn new(preference: RuntimePreference) -> Self {
        Self { preference }
    }

    pub fn selected_binary(&self) -> EngineResult<&'static str> {
        match self.preference {
            RuntimePreference::Docker => {
                if which::which("docker").is_ok() {
                    return Ok("docker");
                }
                if which::which("podman").is_ok() {
                    return Ok("podman");
                }
            }
            RuntimePreference::Podman => {
                if which::which("podman").is_ok() {
                    return Ok("podman");
                }
                if which::which("docker").is_ok() {
                    return Ok("docker");
                }
            }
        }

        Err(EngineError::MissingTool(
            "neither docker nor podman is available".to_string(),
        ))
    }

    pub async fn run_container(&self, spec: &ContainerRunSpec) -> EngineResult<CommandOutput> {
        let runtime_bin = self.selected_binary()?;

        let mut args = vec!["run".to_string(), "--rm".to_string()];

        if let Some((uid, gid)) = spec.uid_gid_map {
            args.push("--user".to_string());
            args.push(format!("{uid}:{gid}"));
        }

        for (host, container) in &spec.mounts {
            args.push("-v".to_string());
            args.push(format!("{}:{}", host.display(), container.display()));
        }

        if let Some(workdir) = &spec.workdir {
            args.push("-w".to_string());
            args.push(workdir.display().to_string());
        }

        for (k, v) in &spec.env {
            args.push("-e".to_string());
            args.push(format!("{k}={v}"));
        }

        args.push(spec.image.clone());
        args.extend(spec.args.clone());

        self.run_host(runtime_bin, &args).await
    }

    pub async fn run_host(&self, program: &str, args: &[String]) -> EngineResult<CommandOutput> {
        let output = Command::new(program)
            .args(args)
            .output()
            .await
            .map_err(|e| EngineError::Runtime(format!("failed to run {program}: {e}")))?;

        Ok(CommandOutput {
            program: program.to_string(),
            status: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

pub fn uid_gid() -> Option<(u32, u32)> {
    let uid = std::env::var("UID").ok()?.parse::<u32>().ok()?;
    let gid = std::env::var("GID").ok()?.parse::<u32>().ok()?;
    Some((uid, gid))
}

fn default_timeout_seconds() -> u64 {
    3600
}
