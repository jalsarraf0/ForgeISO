use chrono::{Datelike, NaiveDate, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    config::Distro,
    error::{EngineError, EngineResult},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyWarning {
    pub code: String,
    pub message: String,
}

pub fn enforce_release_policy(distro: &Distro, release: &str) -> EngineResult<()> {
    match distro {
        Distro::Ubuntu => enforce_ubuntu_lts(release),
        Distro::Mint => enforce_mint_lts(release),
        Distro::Fedora => enforce_fedora_supported(release),
        Distro::Arch => enforce_arch_snapshot(release),
    }
}

pub fn policy_warnings(distro: &Distro, release: &str) -> Vec<PolicyWarning> {
    let mut warnings = Vec::new();

    if matches!(distro, Distro::Fedora) {
        let msg = if let Ok(version) = release.parse::<u16>() {
            let latest = inferred_fedora_stable();
            if version + 1 < latest {
                format!(
                    "Fedora {release} may be at or near EOL. Fedora is short lifecycle and not LTS."
                )
            } else {
                "Fedora is short lifecycle and not LTS; monitor support windows in production."
                    .to_string()
            }
        } else {
            "Fedora is short lifecycle and not LTS; monitor support windows in production."
                .to_string()
        };

        warnings.push(PolicyWarning {
            code: "fedora_lifecycle".to_string(),
            message: msg,
        });
    }

    if matches!(distro, Distro::Arch) {
        warnings.push(PolicyWarning {
            code: "arch_rolling".to_string(),
            message: "Arch uses rolling snapshots; reproducibility is best effort.".to_string(),
        });
    }

    warnings
}

fn enforce_ubuntu_lts(release: &str) -> EngineResult<()> {
    let lts_re = Regex::new(r"^(20|22|24|26|28|30)\.04$")
        .map_err(|e| EngineError::PolicyViolation(format!("regex error: {e}")))?;

    if lts_re.is_match(release) {
        Ok(())
    } else {
        Err(EngineError::PolicyViolation(format!(
            "Ubuntu release '{release}' violates LTS-only policy"
        )))
    }
}

fn enforce_mint_lts(release: &str) -> EngineResult<()> {
    let allowed = ["20", "20.1", "20.2", "20.3", "21", "21.1", "21.2", "21.3", "22"];
    if allowed.contains(&release) {
        Ok(())
    } else {
        Err(EngineError::PolicyViolation(format!(
            "Linux Mint release '{release}' violates LTS-only policy"
        )))
    }
}

fn enforce_fedora_supported(release: &str) -> EngineResult<()> {
    let version = release.parse::<u16>().map_err(|_| {
        EngineError::PolicyViolation(format!(
            "Fedora release '{release}' must be a numeric stable release"
        ))
    })?;

    let latest = inferred_fedora_stable();
    if version + 2 < latest {
        return Err(EngineError::PolicyViolation(format!(
            "Fedora release '{release}' is outside supported stable window"
        )));
    }

    Ok(())
}

fn enforce_arch_snapshot(release: &str) -> EngineResult<()> {
    let date = NaiveDate::parse_from_str(release, "%Y.%m.%d").map_err(|_| {
        EngineError::PolicyViolation("Arch release must use YYYY.MM.DD snapshot format".to_string())
    })?;

    if date > Utc::now().date_naive() {
        return Err(EngineError::PolicyViolation(
            "Arch snapshot cannot be in the future".to_string(),
        ));
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    #[test]
    fn ubuntu_lts_only() {
        assert!(enforce_release_policy(&Distro::Ubuntu, "24.04").is_ok());
        assert!(enforce_release_policy(&Distro::Ubuntu, "23.10").is_err());
    }

    #[test]
    fn mint_lts_only() {
        assert!(enforce_release_policy(&Distro::Mint, "21.3").is_ok());
        assert!(enforce_release_policy(&Distro::Mint, "19.3").is_err());
    }

    #[test]
    fn arch_must_not_be_future() {
        let future = Utc::now().date_naive() + Duration::days(1);
        assert!(enforce_release_policy(
            &Distro::Arch,
            &future.format("%Y.%m.%d").to_string()
        )
        .is_err());
    }
}
