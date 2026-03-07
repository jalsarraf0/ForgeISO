use sha_crypt::{sha512_simple, Sha512Params};

use crate::config::InjectConfig;
use crate::error::{EngineError, EngineResult};

/// Hash a plaintext password to SHA512-crypt format ($6$...)
pub fn hash_password(plaintext: &str) -> EngineResult<String> {
    let params = Sha512Params::new(10_000)
        .map_err(|e| EngineError::Runtime(format!("Failed to create SHA512 params: {:?}", e)))?;
    sha512_simple(plaintext, &params)
        .map_err(|e| EngineError::Runtime(format!("Failed to hash password: {:?}", e)))
}

/// Generate a complete autoinstall YAML document from InjectConfig.
/// Returns a YAML string prefixed with `#cloud-config\n`.
pub fn generate_autoinstall_yaml(cfg: &InjectConfig) -> EngineResult<String> {
    let mut root = serde_yaml::Mapping::new();
    root.insert("cloud-config".into(), serde_yaml::Value::Null);

    let mut autoinstall = serde_yaml::Mapping::new();

    // version
    autoinstall.insert("version".into(), serde_yaml::Value::Number(1.into()));

    // locale
    let locale = cfg.locale.as_deref().unwrap_or("en_US.UTF-8");
    autoinstall.insert(
        "locale".into(),
        serde_yaml::Value::String(locale.to_string()),
    );

    // keyboard
    let mut keyboard = serde_yaml::Mapping::new();
    keyboard.insert(
        "layout".into(),
        serde_yaml::Value::String(cfg.keyboard_layout.as_deref().unwrap_or("us").to_string()),
    );
    autoinstall.insert("keyboard".into(), serde_yaml::Value::Mapping(keyboard));

    // timezone
    let timezone = cfg.timezone.as_deref().unwrap_or("UTC");
    autoinstall.insert(
        "timezone".into(),
        serde_yaml::Value::String(timezone.to_string()),
    );

    // identity (if hostname or username is set)
    if cfg.hostname.is_some()
        || cfg.username.is_some()
        || cfg.password.is_some()
        || cfg.realname.is_some()
    {
        let mut identity = serde_yaml::Mapping::new();
        identity.insert(
            "hostname".into(),
            serde_yaml::Value::String(cfg.hostname.as_deref().unwrap_or("ubuntu").to_string()),
        );
        identity.insert(
            "username".into(),
            serde_yaml::Value::String(cfg.username.as_deref().unwrap_or("ubuntu").to_string()),
        );

        if let Some(pwd) = &cfg.password {
            let hashed = hash_password(pwd)?;
            identity.insert("password".into(), serde_yaml::Value::String(hashed));
        }

        if let Some(realname) = &cfg.realname {
            identity.insert(
                "realname".into(),
                serde_yaml::Value::String(realname.clone()),
            );
        }

        autoinstall.insert("identity".into(), serde_yaml::Value::Mapping(identity));
    }

    // SSH
    let mut ssh = serde_yaml::Mapping::new();

    // install-server defaults to true unless keys are present
    let install_server = cfg
        .ssh
        .install_server
        .unwrap_or(cfg.ssh.authorized_keys.is_empty());
    ssh.insert(
        "install-server".into(),
        serde_yaml::Value::Bool(install_server),
    );

    // authorized-keys
    if !cfg.ssh.authorized_keys.is_empty() {
        let keys: Vec<serde_yaml::Value> = cfg
            .ssh
            .authorized_keys
            .iter()
            .map(|k| serde_yaml::Value::String(k.clone()))
            .collect();
        ssh.insert("authorized-keys".into(), serde_yaml::Value::Sequence(keys));
    }

    // allow-pw: false if keys present, else true (unless explicitly set)
    let allow_pw = cfg
        .ssh
        .allow_password_auth
        .unwrap_or(cfg.ssh.authorized_keys.is_empty());
    ssh.insert("allow-pw".into(), serde_yaml::Value::Bool(allow_pw));

    autoinstall.insert("ssh".into(), serde_yaml::Value::Mapping(ssh));

    // network (only if dns_servers non-empty)
    if !cfg.network.dns_servers.is_empty() {
        let mut network = serde_yaml::Mapping::new();
        network.insert("version".into(), serde_yaml::Value::Number(2.into()));

        let mut ethernets = serde_yaml::Mapping::new();
        let mut any = serde_yaml::Mapping::new();

        let mut match_obj = serde_yaml::Mapping::new();
        match_obj.insert("name".into(), serde_yaml::Value::String("en*".to_string()));
        any.insert("match".into(), serde_yaml::Value::Mapping(match_obj));

        any.insert("dhcp4".into(), serde_yaml::Value::Bool(true));

        let mut nameservers = serde_yaml::Mapping::new();
        let addrs: Vec<serde_yaml::Value> = cfg
            .network
            .dns_servers
            .iter()
            .map(|d| serde_yaml::Value::String(d.clone()))
            .collect();
        nameservers.insert("addresses".into(), serde_yaml::Value::Sequence(addrs));

        any.insert(
            "nameservers".into(),
            serde_yaml::Value::Mapping(nameservers),
        );

        ethernets.insert("any".into(), serde_yaml::Value::Mapping(any));
        network.insert("ethernets".into(), serde_yaml::Value::Mapping(ethernets));

        autoinstall.insert("network".into(), serde_yaml::Value::Mapping(network));
    }

    // storage (only if storage_layout set)
    if let Some(layout) = &cfg.storage_layout {
        let mut storage = serde_yaml::Mapping::new();
        let mut layout_map = serde_yaml::Mapping::new();
        layout_map.insert("name".into(), serde_yaml::Value::String(layout.clone()));
        storage.insert("layout".into(), serde_yaml::Value::Mapping(layout_map));
        autoinstall.insert("storage".into(), serde_yaml::Value::Mapping(storage));
    }

    // apt (only if apt_mirror set)
    if let Some(mirror) = &cfg.apt_mirror {
        let mut apt = serde_yaml::Mapping::new();
        let mut primary_seq = serde_yaml::Sequence::new();
        let mut primary_entry = serde_yaml::Mapping::new();

        let arches: serde_yaml::Sequence = vec![serde_yaml::Value::String("amd64".to_string())];
        primary_entry.insert("arches".into(), serde_yaml::Value::Sequence(arches));

        primary_entry.insert("uri".into(), serde_yaml::Value::String(mirror.clone()));

        primary_seq.push(serde_yaml::Value::Mapping(primary_entry));
        apt.insert("primary".into(), serde_yaml::Value::Sequence(primary_seq));

        autoinstall.insert("apt".into(), serde_yaml::Value::Mapping(apt));
    }

    // packages
    let mut all_packages = cfg.extra_packages.clone();
    if cfg.wallpaper.is_some() {
        all_packages.push("dconf-cli".to_string());
        all_packages.sort();
        all_packages.dedup();
    }
    if !all_packages.is_empty() {
        let pkgs: Vec<serde_yaml::Value> = all_packages
            .iter()
            .map(|p| serde_yaml::Value::String(p.clone()))
            .collect();
        autoinstall.insert("packages".into(), serde_yaml::Value::Sequence(pkgs));
    }

    // late-commands
    let mut late_commands = Vec::new();

    // NTP servers
    if !cfg.network.ntp_servers.is_empty() {
        let ntp_list = cfg.network.ntp_servers.join(" ");
        late_commands.push(format!(
            "printf '[Time]\\nNTP={ntp_list}\\n' > /target/etc/systemd/timesyncd.conf"
        ));
        late_commands.push("chroot /target systemctl enable systemd-timesyncd".to_string());
    }

    // Wallpaper
    if let Some(wallpaper_path) = &cfg.wallpaper {
        if let Some(filename) = wallpaper_path.file_name() {
            if let Some(filename_str) = filename.to_str() {
                let ext = wallpaper_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("jpg");
                late_commands.push(format!(
                    "cp /cdrom/wallpaper/{} /target/usr/share/backgrounds/forgeiso-wallpaper.{}",
                    filename_str, ext
                ));
                late_commands.push("mkdir -p /target/etc/dconf/db/local.d".to_string());
                late_commands.push(
                    "printf '[org/gnome/desktop/background]\\npicture-uri=\"file:///usr/share/backgrounds/forgeiso-wallpaper.{}\\\"\\n' > /target/etc/dconf/db/local.d/00-forgeiso-background".to_string()
                );
                late_commands.push("chroot /target dconf update".to_string());
            }
        }
    }

    // Extra late commands
    late_commands.extend(cfg.extra_late_commands.clone());

    if !late_commands.is_empty() {
        let cmds: Vec<serde_yaml::Value> = late_commands
            .iter()
            .map(|c| serde_yaml::Value::String(c.clone()))
            .collect();
        autoinstall.insert("late-commands".into(), serde_yaml::Value::Sequence(cmds));
    }

    // interactive-sections (only if no_user_interaction = true)
    if cfg.no_user_interaction {
        autoinstall.insert(
            "interactive-sections".into(),
            serde_yaml::Value::Sequence(vec![]),
        );
    }

    root.insert(
        "autoinstall".into(),
        serde_yaml::Value::Mapping(autoinstall),
    );

    // Serialize and prepend cloud-config header
    let yaml_str = serde_yaml::to_string(&root)
        .map_err(|e| EngineError::Runtime(format!("Failed to serialize YAML: {}", e)))?;

    // Remove the "cloud-config: null" line that serde_yaml adds
    let lines: Vec<&str> = yaml_str.lines().collect();
    let filtered: Vec<&str> = lines
        .iter()
        .filter(|line| !line.contains("cloud-config:"))
        .copied()
        .collect();

    Ok(format!("#cloud-config\n{}", filtered.join("\n")))
}

/// Merge InjectConfig into an existing autoinstall YAML string.
/// CLI config fields override YAML fields. late-commands are appended, packages/keys are merged.
pub fn merge_autoinstall_yaml(existing: &str, cfg: &InjectConfig) -> EngineResult<String> {
    // Parse existing YAML
    let mut root: serde_yaml::Value = serde_yaml::from_str(existing)
        .map_err(|e| EngineError::Runtime(format!("Failed to parse YAML: {}", e)))?;

    // Get or create autoinstall mapping
    let autoinstall_map = if let Some(ai) = root.get_mut("autoinstall") {
        ai.as_mapping_mut()
            .ok_or_else(|| EngineError::Runtime("autoinstall must be a mapping".to_string()))?
    } else {
        // Create new autoinstall entry
        let mut new_root = serde_yaml::Mapping::new();
        new_root.insert(
            "autoinstall".into(),
            serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
        );
        root = serde_yaml::Value::Mapping(new_root);
        root.get_mut("autoinstall")
            .unwrap()
            .as_mapping_mut()
            .unwrap()
    };

    // Override scalar fields from cfg
    if let Some(locale) = &cfg.locale {
        autoinstall_map.insert("locale".into(), serde_yaml::Value::String(locale.clone()));
    }

    if let Some(timezone) = &cfg.timezone {
        autoinstall_map.insert(
            "timezone".into(),
            serde_yaml::Value::String(timezone.clone()),
        );
    }

    // keyboard
    if cfg.keyboard_layout.is_some() {
        let mut keyboard = autoinstall_map
            .remove("keyboard")
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();
        keyboard.insert(
            "layout".into(),
            serde_yaml::Value::String(cfg.keyboard_layout.as_deref().unwrap_or("us").to_string()),
        );
        autoinstall_map.insert("keyboard".into(), serde_yaml::Value::Mapping(keyboard));
    }

    // identity
    if cfg.hostname.is_some()
        || cfg.username.is_some()
        || cfg.password.is_some()
        || cfg.realname.is_some()
    {
        let mut identity = autoinstall_map
            .remove("identity")
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();

        if let Some(hostname) = &cfg.hostname {
            identity.insert(
                "hostname".into(),
                serde_yaml::Value::String(hostname.clone()),
            );
        }

        if let Some(username) = &cfg.username {
            identity.insert(
                "username".into(),
                serde_yaml::Value::String(username.clone()),
            );
        }

        if let Some(password) = &cfg.password {
            let hashed = hash_password(password)?;
            identity.insert("password".into(), serde_yaml::Value::String(hashed));
        }

        if let Some(realname) = &cfg.realname {
            identity.insert(
                "realname".into(),
                serde_yaml::Value::String(realname.clone()),
            );
        }

        autoinstall_map.insert("identity".into(), serde_yaml::Value::Mapping(identity));
    }

    // SSH
    if !cfg.ssh.authorized_keys.is_empty()
        || cfg.ssh.allow_password_auth.is_some()
        || cfg.ssh.install_server.is_some()
    {
        let mut ssh = autoinstall_map
            .remove("ssh")
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();

        if !cfg.ssh.authorized_keys.is_empty() {
            let keys: Vec<serde_yaml::Value> = cfg
                .ssh
                .authorized_keys
                .iter()
                .map(|k| serde_yaml::Value::String(k.clone()))
                .collect();
            ssh.insert("authorized-keys".into(), serde_yaml::Value::Sequence(keys));
        }

        if let Some(allow_pw) = cfg.ssh.allow_password_auth {
            ssh.insert("allow-pw".into(), serde_yaml::Value::Bool(allow_pw));
        }

        if let Some(install) = cfg.ssh.install_server {
            ssh.insert("install-server".into(), serde_yaml::Value::Bool(install));
        }

        autoinstall_map.insert("ssh".into(), serde_yaml::Value::Mapping(ssh));
    }

    // network
    if !cfg.network.dns_servers.is_empty() || !cfg.network.ntp_servers.is_empty() {
        let mut network = autoinstall_map
            .remove("network")
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();

        if !cfg.network.dns_servers.is_empty() {
            network.insert("version".into(), serde_yaml::Value::Number(2.into()));
            let mut ethernets = serde_yaml::Mapping::new();
            let mut any = serde_yaml::Mapping::new();

            let mut match_obj = serde_yaml::Mapping::new();
            match_obj.insert("name".into(), serde_yaml::Value::String("en*".to_string()));
            any.insert("match".into(), serde_yaml::Value::Mapping(match_obj));

            any.insert("dhcp4".into(), serde_yaml::Value::Bool(true));

            let mut nameservers = serde_yaml::Mapping::new();
            let addrs: Vec<serde_yaml::Value> = cfg
                .network
                .dns_servers
                .iter()
                .map(|d| serde_yaml::Value::String(d.clone()))
                .collect();
            nameservers.insert("addresses".into(), serde_yaml::Value::Sequence(addrs));

            any.insert(
                "nameservers".into(),
                serde_yaml::Value::Mapping(nameservers),
            );

            ethernets.insert("any".into(), serde_yaml::Value::Mapping(any));
            network.insert("ethernets".into(), serde_yaml::Value::Mapping(ethernets));
        }

        autoinstall_map.insert("network".into(), serde_yaml::Value::Mapping(network));
    }

    // storage
    if let Some(layout) = &cfg.storage_layout {
        let mut storage = autoinstall_map
            .remove("storage")
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();
        let mut layout_map = serde_yaml::Mapping::new();
        layout_map.insert("name".into(), serde_yaml::Value::String(layout.clone()));
        storage.insert("layout".into(), serde_yaml::Value::Mapping(layout_map));
        autoinstall_map.insert("storage".into(), serde_yaml::Value::Mapping(storage));
    }

    // apt
    if let Some(mirror) = &cfg.apt_mirror {
        let mut apt = autoinstall_map
            .remove("apt")
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();
        let mut primary_seq = serde_yaml::Sequence::new();
        let mut primary_entry = serde_yaml::Mapping::new();

        let arches: serde_yaml::Sequence = vec![serde_yaml::Value::String("amd64".to_string())];
        primary_entry.insert("arches".into(), serde_yaml::Value::Sequence(arches));

        primary_entry.insert("uri".into(), serde_yaml::Value::String(mirror.clone()));

        primary_seq.push(serde_yaml::Value::Mapping(primary_entry));
        apt.insert("primary".into(), serde_yaml::Value::Sequence(primary_seq));

        autoinstall_map.insert("apt".into(), serde_yaml::Value::Mapping(apt));
    }

    // packages: merge (dedup)
    let mut all_packages = cfg.extra_packages.clone();
    if cfg.wallpaper.is_some() {
        all_packages.push("dconf-cli".to_string());
    }

    if let Some(existing_pkgs) = autoinstall_map
        .get("packages")
        .and_then(|v| v.as_sequence())
    {
        for pkg_val in existing_pkgs {
            if let Some(pkg_str) = pkg_val.as_str() {
                all_packages.push(pkg_str.to_string());
            }
        }
    }

    all_packages.sort();
    all_packages.dedup();

    if !all_packages.is_empty() {
        let pkgs: Vec<serde_yaml::Value> = all_packages
            .iter()
            .map(|p| serde_yaml::Value::String(p.clone()))
            .collect();
        autoinstall_map.insert("packages".into(), serde_yaml::Value::Sequence(pkgs));
    }

    // late-commands: append (not override)
    let mut all_late_commands = Vec::new();

    // Existing commands
    if let Some(existing_cmds) = autoinstall_map
        .get("late-commands")
        .and_then(|v| v.as_sequence())
    {
        for cmd_val in existing_cmds {
            if let Some(cmd_str) = cmd_val.as_str() {
                all_late_commands.push(cmd_str.to_string());
            }
        }
    }

    // NTP servers
    if !cfg.network.ntp_servers.is_empty() {
        let ntp_list = cfg.network.ntp_servers.join(" ");
        all_late_commands.push(format!(
            "printf '[Time]\\nNTP={ntp_list}\\n' > /target/etc/systemd/timesyncd.conf"
        ));
        all_late_commands.push("chroot /target systemctl enable systemd-timesyncd".to_string());
    }

    // Wallpaper
    if let Some(wallpaper_path) = &cfg.wallpaper {
        if let Some(filename) = wallpaper_path.file_name() {
            if let Some(filename_str) = filename.to_str() {
                let ext = wallpaper_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("jpg");
                all_late_commands.push(format!(
                    "cp /cdrom/wallpaper/{} /target/usr/share/backgrounds/forgeiso-wallpaper.{}",
                    filename_str, ext
                ));
                all_late_commands.push("mkdir -p /target/etc/dconf/db/local.d".to_string());
                all_late_commands.push(
                    "printf '[org/gnome/desktop/background]\\npicture-uri=\"file:///usr/share/backgrounds/forgeiso-wallpaper.{}\\\"\\n' > /target/etc/dconf/db/local.d/00-forgeiso-background".to_string()
                );
                all_late_commands.push("chroot /target dconf update".to_string());
            }
        }
    }

    // Extra late commands
    all_late_commands.extend(cfg.extra_late_commands.clone());

    if !all_late_commands.is_empty() {
        let cmds: Vec<serde_yaml::Value> = all_late_commands
            .iter()
            .map(|c: &String| serde_yaml::Value::String(c.clone()))
            .collect();
        autoinstall_map.insert("late-commands".into(), serde_yaml::Value::Sequence(cmds));
    }

    // interactive-sections
    if cfg.no_user_interaction {
        autoinstall_map.insert(
            "interactive-sections".into(),
            serde_yaml::Value::Sequence(vec![]),
        );
    }

    // Serialize back
    let yaml_str = serde_yaml::to_string(&root)
        .map_err(|e| EngineError::Runtime(format!("Failed to serialize YAML: {}", e)))?;

    // Preserve cloud-config header if original had it
    if existing.starts_with("#cloud-config") {
        Ok(format!("#cloud-config\n{}", yaml_str))
    } else {
        Ok(yaml_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password_format() {
        let hashed = hash_password("testpass").unwrap();
        assert!(hashed.starts_with("$6$"), "Hash should start with $6$");
    }

    #[test]
    fn test_generate_minimal_yaml() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: Default::default(),
            network: Default::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.starts_with("#cloud-config"),
            "YAML should start with #cloud-config"
        );
        assert!(
            yaml.contains("autoinstall:"),
            "YAML should contain autoinstall section"
        );
        assert!(
            yaml.contains("version: 1"),
            "YAML should contain version: 1"
        );
    }

    #[test]
    fn test_generate_with_identity() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            hostname: Some("test-host".to_string()),
            username: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            realname: Some("Test User".to_string()),
            ssh: Default::default(),
            network: Default::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.contains("identity:"),
            "YAML should contain identity section"
        );
        assert!(yaml.contains("test-host"), "hostname should be in YAML");
        assert!(yaml.contains("testuser"), "username should be in YAML");
        assert!(yaml.contains("$6$"), "password should be hashed with $6$");
        assert!(yaml.contains("Test User"), "realname should be in YAML");
    }

    #[test]
    fn test_generate_with_ssh_keys() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: crate::config::SshConfig {
                authorized_keys: vec![
                    "ssh-ed25519 AAAA...".to_string(),
                    "ssh-rsa BBBB...".to_string(),
                ],
                allow_password_auth: None,
                install_server: None,
            },
            network: Default::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(yaml.contains("ssh:"), "YAML should contain ssh section");
        assert!(yaml.contains("AAAA"), "first key should be in YAML");
        assert!(yaml.contains("BBBB"), "second key should be in YAML");
        assert!(
            yaml.contains("allow-pw: false"),
            "allow-pw should be false when keys present"
        );
    }

    #[test]
    fn test_generate_with_dns() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: Default::default(),
            network: crate::config::NetworkConfig {
                dns_servers: vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()],
                ntp_servers: vec![],
            },
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.contains("network:"),
            "YAML should contain network section"
        );
        assert!(yaml.contains("1.1.1.1"), "DNS 1 should be in YAML");
        assert!(yaml.contains("8.8.8.8"), "DNS 2 should be in YAML");
    }

    #[test]
    fn test_generate_with_wallpaper() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: Default::default(),
            network: Default::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: Some(std::path::PathBuf::from("/tmp/bg.jpg")),
            extra_late_commands: vec![],
            no_user_interaction: false,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.contains("late-commands:"),
            "YAML should contain late-commands"
        );
        assert!(
            yaml.contains("cp /cdrom/wallpaper/bg.jpg"),
            "copy command should be present"
        );
        assert!(
            yaml.contains("dconf update"),
            "dconf update should be present"
        );
        assert!(
            yaml.contains("dconf-cli"),
            "dconf-cli should be in packages"
        );
    }

    #[test]
    fn test_merge_preserves_existing() {
        let existing = r#"
autoinstall:
  version: 1
  storage:
    layout:
      name: lvm
"#;
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            hostname: Some("newhost".to_string()),
            username: None,
            password: None,
            realname: None,
            ssh: Default::default(),
            network: Default::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
        };

        let result = merge_autoinstall_yaml(existing, &cfg).unwrap();
        assert!(
            result.contains("lvm"),
            "existing storage layout should be preserved"
        );
        assert!(result.contains("newhost"), "new hostname should be present");
    }

    #[test]
    fn test_merge_overrides_identity() {
        let existing = r#"
autoinstall:
  identity:
    username: olduser
    hostname: oldhost
"#;
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            hostname: Some("newhost".to_string()),
            username: Some("newuser".to_string()),
            password: None,
            realname: None,
            ssh: Default::default(),
            network: Default::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
        };

        let result = merge_autoinstall_yaml(existing, &cfg).unwrap();
        assert!(result.contains("newuser"), "new username should override");
        assert!(result.contains("newhost"), "new hostname should override");
        assert!(!result.contains("olduser"), "old username should be gone");
        assert!(!result.contains("oldhost"), "old hostname should be gone");
    }

    #[test]
    fn test_merge_appends_late_commands() {
        let existing = r#"
autoinstall:
  late-commands:
    - "echo existing"
"#;
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: Default::default(),
            network: Default::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec!["echo new".to_string()],
            no_user_interaction: false,
        };

        let result = merge_autoinstall_yaml(existing, &cfg).unwrap();
        assert!(
            result.contains("echo existing"),
            "existing command should be preserved"
        );
        assert!(
            result.contains("echo new"),
            "new command should be appended"
        );
    }
}
