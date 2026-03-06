use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::error::{EngineError, EngineResult};

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub input: PathBuf,
    pub work: PathBuf,
    pub output: PathBuf,
    pub reports: PathBuf,
    pub scans: PathBuf,
    pub logs: PathBuf,
}

impl Workspace {
    pub fn create(base: &Path, run_name: &str) -> EngineResult<Self> {
        std::fs::create_dir_all(base)?;

        let sanitized_name = sanitize_run_name(run_name);
        let root = base.join(format!("{}-{}", sanitized_name, Uuid::new_v4()));
        let input = root.join("input");
        let work = root.join("work");
        let output = root.join("output");
        let reports = root.join("reports");
        let scans = root.join("scans");
        let logs = root.join("logs");

        for dir in [&root, &input, &work, &output, &reports, &scans, &logs] {
            std::fs::create_dir_all(dir)?;
        }

        Ok(Self {
            root,
            input,
            work,
            output,
            reports,
            scans,
            logs,
        })
    }

    pub fn safe_join(&self, relative: &Path) -> EngineResult<PathBuf> {
        safe_join(&self.root, relative)
    }
}

pub fn safe_join(root: &Path, candidate: &Path) -> EngineResult<PathBuf> {
    let root = root
        .canonicalize()
        .map_err(|e| EngineError::PathSafety(format!("canonicalize root failed: {e}")))?;

    let mut joined = root.clone();

    if candidate.is_absolute() {
        let absolute = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.to_path_buf());
        if !absolute.starts_with(&root) {
            return Err(EngineError::PathSafety(format!(
                "path escapes workspace: {}",
                absolute.display()
            )));
        }
        joined = absolute;
    } else {
        for component in candidate.components() {
            use std::path::Component;
            match component {
                Component::CurDir => {}
                Component::Normal(seg) => joined.push(seg),
                Component::ParentDir => {
                    if !joined.pop() || !joined.starts_with(&root) {
                        return Err(EngineError::PathSafety(format!(
                            "path escapes workspace: {}",
                            candidate.display()
                        )));
                    }
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(EngineError::PathSafety(format!(
                        "invalid component in relative path: {}",
                        candidate.display()
                    )))
                }
            }
        }
    }

    if let Some(parent) = joined.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if !joined.starts_with(&root) {
        return Err(EngineError::PathSafety(format!(
            "path escapes workspace: {}",
            joined.display()
        )));
    }

    Ok(joined)
}

fn sanitize_run_name(input: &str) -> String {
    let cleaned = input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();

    cleaned.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn safe_join_rejects_parent_escape() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        std::fs::create_dir_all(root.join("ok")).expect("mk root");

        let escaped = safe_join(root, Path::new("../etc/passwd"));
        assert!(escaped.is_err());
    }

    #[test]
    fn safe_join_allows_child_path() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        std::fs::create_dir_all(root.join("ok")).expect("mk root");

        let child = safe_join(root, Path::new("ok/file.txt")).expect("safe path");
        assert!(child.starts_with(root));
    }
}
