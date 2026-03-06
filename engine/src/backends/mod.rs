use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::{BuildConfig, Distro};
use crate::error::{EngineError, EngineResult};
use crate::modules::ModuleAction;
use crate::runtime::{uid_gid, ContainerRunSpec};
use crate::workspace::Workspace;

pub mod arch;
pub mod fedora;
pub mod mint;
pub mod ubuntu;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendPlan {
    pub backend_name: String,
    pub base_iso_url: Option<String>,
    pub base_iso_checksum: Option<String>,
    pub commands: Vec<ContainerRunSpec>,
    pub warnings: Vec<String>,
    pub produced_iso: PathBuf,
}

pub fn plan_build(
    cfg: &BuildConfig,
    workspace: &Workspace,
    actions: &[ModuleAction],
) -> EngineResult<BackendPlan> {
    match cfg.distro {
        Distro::Ubuntu => ubuntu::plan(cfg, workspace, actions),
        Distro::Mint => mint::plan(cfg, workspace, actions),
        Distro::Fedora => fedora::plan(cfg, workspace, actions),
        Distro::Arch => arch::plan(cfg, workspace, actions),
    }
}

pub fn ensure_base_iso_url(cfg: &BuildConfig) -> EngineResult<String> {
    cfg.release
        .base_iso_url
        .clone()
        .ok_or_else(|| EngineError::InvalidConfig("release.base_iso_url is required".to_string()))
}

pub fn default_env() -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert("LC_ALL".to_string(), "C.UTF-8".to_string());
    env.insert("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string());
    env
}

pub fn shared_mounts(workspace: &Workspace) -> Vec<(PathBuf, PathBuf)> {
    vec![
        (workspace.root.clone(), PathBuf::from("/workspace")),
        (workspace.logs.clone(), PathBuf::from("/workspace/logs")),
    ]
}

pub fn spec(image: &str, command: String, workspace: &Workspace) -> ContainerRunSpec {
    ContainerRunSpec {
        image: image.to_string(),
        args: vec!["bash".to_string(), "-lc".to_string(), command],
        env: default_env(),
        mounts: shared_mounts(workspace),
        workdir: Some(PathBuf::from("/workspace")),
        uid_gid_map: uid_gid(),
        timeout_seconds: 14_400,
    }
}

pub fn module_snippets(actions: &[ModuleAction]) -> String {
    let mut snippets = Vec::new();
    for action in actions {
        snippets.push(format!("echo '[module] {}'", shell_escape(&action.description)));
        snippets.extend(action.shell_snippets.iter().cloned());
    }
    snippets.join("; ")
}

fn shell_escape(input: &str) -> String {
    input.replace('\'', "''")
}
