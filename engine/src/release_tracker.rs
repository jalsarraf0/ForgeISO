use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    config::Distro,
    error::{EngineError, EngineResult},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub distro: Distro,
    pub version: String,
    pub lts: bool,
    pub stable: bool,
    pub eol_warning: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReleaseTracker;

impl ReleaseTracker {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_releases(&self, distro: Distro) -> EngineResult<Vec<ReleaseInfo>> {
        let releases = match distro {
            Distro::Ubuntu => self.fetch_ubuntu_lts().await,
            Distro::Mint => self.fetch_mint_lts().await,
            Distro::Fedora => self.fetch_fedora_stable().await,
            Distro::Arch => self.fetch_arch_snapshot().await,
        };

        if releases.is_empty() {
            Err(EngineError::NotFound(format!(
                "no releases discovered for {:?}",
                distro
            )))
        } else {
            Ok(releases)
        }
    }

    async fn fetch_ubuntu_lts(&self) -> Vec<ReleaseInfo> {
        let mut discovered = Vec::new();

        if let Ok(resp) = reqwest::get("https://changelogs.ubuntu.com/meta-release-lts").await {
            if let Ok(body) = resp.text().await {
                for line in body.lines() {
                    if let Some(raw) = line.strip_prefix("Version:") {
                        let cleaned = raw.trim().replace(" LTS", "");
                        let version = cleaned.split_whitespace().next().unwrap_or("").to_string();
                        if !version.is_empty() {
                            discovered.push(ReleaseInfo {
                                distro: Distro::Ubuntu,
                                version,
                                lts: true,
                                stable: true,
                                eol_warning: None,
                            });
                        }
                    }
                }
            }
        }

        if discovered.is_empty() {
            discovered = vec![
                ReleaseInfo {
                    distro: Distro::Ubuntu,
                    version: "24.04".to_string(),
                    lts: true,
                    stable: true,
                    eol_warning: None,
                },
                ReleaseInfo {
                    distro: Distro::Ubuntu,
                    version: "22.04".to_string(),
                    lts: true,
                    stable: true,
                    eol_warning: None,
                },
            ];
        }

        discovered.sort_by(|a, b| b.version.cmp(&a.version));
        discovered.dedup_by(|a, b| a.version == b.version);
        discovered
    }

    async fn fetch_mint_lts(&self) -> Vec<ReleaseInfo> {
        vec![
            ReleaseInfo {
                distro: Distro::Mint,
                version: "22".to_string(),
                lts: true,
                stable: true,
                eol_warning: None,
            },
            ReleaseInfo {
                distro: Distro::Mint,
                version: "21.3".to_string(),
                lts: true,
                stable: true,
                eol_warning: None,
            },
            ReleaseInfo {
                distro: Distro::Mint,
                version: "21.2".to_string(),
                lts: true,
                stable: true,
                eol_warning: None,
            },
        ]
    }

    async fn fetch_fedora_stable(&self) -> Vec<ReleaseInfo> {
        let stable = inferred_fedora_stable();
        vec![ReleaseInfo {
            distro: Distro::Fedora,
            version: stable.to_string(),
            lts: false,
            stable: true,
            eol_warning: Some(
                "Fedora has a short support lifecycle and is not LTS. Validate upgrade cadence."
                    .to_string(),
            ),
        }]
    }

    async fn fetch_arch_snapshot(&self) -> Vec<ReleaseInfo> {
        let now = Utc::now();
        vec![ReleaseInfo {
            distro: Distro::Arch,
            version: format!("{:04}.{:02}.{:02}", now.year(), now.month(), now.day()),
            lts: false,
            stable: true,
            eol_warning: Some("Arch is rolling; snapshot reproducibility is best effort.".to_string()),
        }]
    }
}

fn inferred_fedora_stable() -> u16 {
    let year = Utc::now().year();
    let month = Utc::now().month();

    match (year, month) {
        (2025, 1..=4) => 41,
        (2025, 5..=10) => 42,
        (2025, _) => 43,
        (2026, 1..=4) => 43,
        (2026, 5..=10) => 44,
        (2026, _) => 45,
        (2027, 1..=4) => 45,
        (2027, 5..=10) => 46,
        _ => 46,
    }
}
