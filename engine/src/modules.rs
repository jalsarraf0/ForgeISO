use crate::{
    config::{BuildConfig, ModuleSpec, ModuleType},
    error::{EngineError, EngineResult},
};

#[derive(Debug, Clone)]
pub struct ModuleAction {
    pub module_type: ModuleType,
    pub description: String,
    pub shell_snippets: Vec<String>,
}

pub fn evaluate_modules(cfg: &BuildConfig) -> EngineResult<Vec<ModuleAction>> {
    let mut actions = Vec::new();

    for module in &cfg.modules {
        if !module.enabled {
            continue;
        }
        actions.push(build_action(module, cfg.dangerous_mode.enabled)?);
    }

    Ok(actions)
}

fn build_action(module: &ModuleSpec, dangerous_mode_enabled: bool) -> EngineResult<ModuleAction> {
    if module.dangerous && !dangerous_mode_enabled {
        return Err(EngineError::PolicyViolation(
            "dangerous module requested while dangerous mode is disabled".to_string(),
        ));
    }

    let (description, shell_snippets) = match module.module_type {
        ModuleType::Packages => {
            let install_values = module
                .config
                .get("install")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let install = install_values
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect::<Vec<_>>();

            let remove_values = module
                .config
                .get("remove")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let remove = remove_values
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect::<Vec<_>>();

            (
                "Package operations".to_string(),
                vec![
                    format!("# install: {}", install.join(",")),
                    format!("# remove: {}", remove.join(",")),
                ],
            )
        }
        ModuleType::Files => (
            "File injection".to_string(),
            vec!["# copy managed files into target rootfs".to_string()],
        ),
        ModuleType::Systemd => (
            "Systemd service policy".to_string(),
            vec!["# systemctl enable/disable units based on module config".to_string()],
        ),
        ModuleType::Users => (
            "User provisioning".to_string(),
            vec!["# apply user/group policy from config.users".to_string()],
        ),
        ModuleType::Ssh => (
            "SSH policy".to_string(),
            vec!["# apply hardened sshd_config directives".to_string()],
        ),
        ModuleType::Desktop => (
            "Desktop customization".to_string(),
            vec!["# apply wallpaper/theme/icon/cursor settings".to_string()],
        ),
        ModuleType::Browser => (
            "Browser customization".to_string(),
            vec!["# configure browser settings and extensions".to_string()],
        ),
        ModuleType::Drivers => (
            "Driver package policy".to_string(),
            vec!["# install selected driver packages".to_string()],
        ),
        ModuleType::Fonts => (
            "Font pack policy".to_string(),
            vec!["# install selected fonts".to_string()],
        ),
        ModuleType::Codecs => (
            "Codec package policy".to_string(),
            vec!["# install selected media codecs".to_string()],
        ),
        ModuleType::CustomScript => {
            let script_path = module
                .config
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    EngineError::InvalidConfig(
                        "custom_script module requires config.path".to_string(),
                    )
                })?;
            (
                "Custom script execution".to_string(),
                vec![format!("bash {}", script_path)],
            )
        }
    };

    Ok(ModuleAction {
        module_type: module.module_type.clone(),
        description,
        shell_snippets,
    })
}
