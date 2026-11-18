use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::Cli;

pub const DEFAULT_MAX_MESSAGES: usize = 5_000;
pub const DEFAULT_MAX_FILE_MB: u64 = 10;

#[derive(Debug, Clone)]
pub struct Config {
    pub paths: Paths,
    pub identity: Identity,
    pub history: HistoryConfig,
    pub ui: UiConfig,
    pub network: NetworkConfig,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub node_id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct HistoryConfig {
    pub enabled: bool,
    pub max_messages: usize,
    pub max_file_mb: u64,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_messages: DEFAULT_MAX_MESSAGES,
            max_file_mb: DEFAULT_MAX_FILE_MB,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UiConfig {
    pub show_help_hint: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_help_hint: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub enabled: bool,
    pub port: u16,
    pub mdns: bool,
    pub reconnect: bool,
    pub seeds: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 47_731,
            mdns: true,
            reconnect: true,
            seeds: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub keymap_file: PathBuf,
    pub identity_file: PathBuf,
    pub data_dir: PathBuf,
    pub files_dir: PathBuf,
    pub history_dir: PathBuf,
    pub state_dir: PathBuf,
    pub log_dir: PathBuf,
    pub panic_log: PathBuf,
}

impl Paths {
    pub fn discover(
        config_override: Option<&Path>,
        keymap_override: Option<&Path>,
    ) -> Result<Self> {
        let xdg_config = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("无法确定配置目录"))?
            .join("lanchat");
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("无法确定数据目录"))?
            .join("lanchat");
        let state_root = dirs::state_dir()
            .unwrap_or_else(|| dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")));
        let state_dir = state_root.join("lanchat");

        let (config_dir, config_file) = match config_override {
            Some(path) => {
                let parent = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| xdg_config.clone());
                (parent, path.to_path_buf())
            }
            None => (xdg_config.clone(), xdg_config.join("config.toml")),
        };
        let keymap_file = keymap_override
            .map(Path::to_path_buf)
            .unwrap_or_else(|| config_dir.join("keymap.toml"));
        let identity_file = config_dir.join("identity.json");
        let files_dir = data_dir.join("files");
        let history_dir = data_dir.join("history");
        let log_dir = state_dir.join("logs");

        Ok(Self {
            config_dir,
            config_file,
            keymap_file,
            identity_file,
            data_dir,
            files_dir,
            history_dir,
            state_dir,
            log_dir: log_dir.clone(),
            panic_log: log_dir.join("panic.log"),
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        ensure_dir(&self.config_dir)?;
        ensure_dir(&self.data_dir)?;
        ensure_dir(&self.files_dir)?;
        ensure_dir(&self.history_dir)?;
        ensure_dir(&self.state_dir)?;
        ensure_dir(&self.log_dir)?;
        Ok(())
    }
}

pub fn load(cli: &Cli) -> Result<Config> {
    let paths = Paths::discover(cli.config.as_deref(), cli.keymap.as_deref())?;
    paths.ensure_dirs()?;

    let mut warnings = Vec::new();
    let file = if cli.no_config {
        ConfigFile::default()
    } else if paths.config_file.exists() {
        match fs::read_to_string(&paths.config_file)
            .with_context(|| format!("读取配置失败: {}", paths.config_file.display()))
            .and_then(|text| Ok(toml::from_str::<ConfigFile>(&text)?))
        {
            Ok(file) => file,
            Err(error) => {
                warnings.push(format!("配置读取失败，使用默认值: {error}"));
                ConfigFile::default()
            }
        }
    } else {
        ConfigFile::default()
    };

    let (identity, identity_warnings) =
        load_or_create_identity(&paths.identity_file, file.identity.name)?;
    warnings.extend(identity_warnings);

    let history = HistoryConfig {
        enabled: file.history.enabled.unwrap_or(true),
        max_messages: file
            .history
            .max_messages
            .unwrap_or(DEFAULT_MAX_MESSAGES)
            .clamp(100, 100_000),
        max_file_mb: file
            .history
            .max_file_mb
            .unwrap_or(DEFAULT_MAX_FILE_MB)
            .clamp(1, 1_024),
    };
    let ui = UiConfig {
        show_help_hint: file.ui.show_help_hint.unwrap_or(true),
    };
    let mut network = NetworkConfig {
        enabled: file.network.enabled.unwrap_or(true),
        port: file.network.port.unwrap_or(47_731),
        mdns: file.network.mdns.unwrap_or(true),
        reconnect: file.network.reconnect.unwrap_or(true),
        seeds: file.network.seeds.unwrap_or_default(),
    };
    if cli.no_network {
        network.enabled = false;
    }
    if let Some(port) = cli.port {
        network.port = port;
    }
    network.seeds.extend(cli.connect.iter().cloned());

    Ok(Config {
        paths,
        identity,
        history,
        ui,
        network,
        warnings,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    identity: IdentitySection,
    #[serde(default)]
    history: HistorySection,
    #[serde(default)]
    ui: UiSection,
    #[serde(default)]
    network: NetworkSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct IdentitySection {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HistorySection {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    max_messages: Option<usize>,
    #[serde(default)]
    max_file_mb: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UiSection {
    #[serde(default)]
    show_help_hint: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct NetworkSection {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    mdns: Option<bool>,
    #[serde(default)]
    reconnect: Option<bool>,
    #[serde(default)]
    seeds: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct IdentityFile {
    #[serde(default)]
    node_id: String,
    #[serde(default, alias = "display_name")]
    name: String,
}

fn load_or_create_identity(
    path: &Path,
    config_name: Option<String>,
) -> Result<(Identity, Vec<String>)> {
    let mut warnings = Vec::new();
    let mut identity_file = if path.exists() {
        match fs::read_to_string(path)
            .with_context(|| format!("读取身份失败: {}", path.display()))
            .and_then(|text| Ok(serde_json::from_str::<IdentityFile>(&text)?))
        {
            Ok(file) => file,
            Err(error) => {
                warnings.push(format!("身份文件损坏，重新生成: {error}"));
                IdentityFile::default()
            }
        }
    } else {
        IdentityFile::default()
    };

    let mut changed = false;
    if identity_file.node_id.trim().is_empty() {
        identity_file.node_id = uuid::Uuid::new_v4().to_string();
        changed = true;
    }
    if let Some(name) = config_name.filter(|name| !name.trim().is_empty()) {
        if identity_file.name != name {
            identity_file.name = name;
            changed = true;
        }
    }
    if identity_file.name.trim().is_empty() {
        identity_file.name = default_name();
        changed = true;
    }

    if changed || !path.exists() {
        save_identity(path, &identity_file)?;
    }

    Ok((
        Identity {
            node_id: identity_file.node_id,
            name: identity_file.name,
        },
        warnings,
    ))
}

fn save_identity(path: &Path, identity: &IdentityFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let text = serde_json::to_string_pretty(identity)?;
    fs::write(path, text).with_context(|| format!("写入身份失败: {}", path.display()))?;
    set_file_mode(path, 0o600);
    Ok(())
}

fn default_name() -> String {
    let user = whoami::username();
    let host = hostname::get()
        .ok()
        .and_then(|host| host.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());
    format!("{user}@{host}")
}

fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path).with_context(|| format!("创建目录失败: {}", path.display()))?;
    }
    set_dir_mode(path, 0o700);
    Ok(())
}

#[cfg(unix)]
fn set_dir_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_dir_mode(_path: &Path, _mode: u32) {}

#[cfg(unix)]
fn set_file_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_file_mode(_path: &Path, _mode: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_defaults_are_clamped() {
        let file: ConfigFile = toml::from_str(
            "[history]\nmax_messages = 1\nmax_file_mb = 0\n[ui]\nshow_help_hint = false\n",
        )
        .unwrap();
        assert_eq!(file.history.max_messages, Some(1));
        assert_eq!(file.history.max_file_mb, Some(0));
        assert_eq!(file.ui.show_help_hint, Some(false));
    }

    #[test]
    fn identity_accepts_old_display_name() {
        let file: IdentityFile =
            serde_json::from_str(r#"{"node_id":"n","display_name":"alice@host"}"#).unwrap();
        assert_eq!(file.name, "alice@host");
    }

    #[test]
    fn parses_network_section() {
        let file: ConfigFile = toml::from_str(
            "[network]\nenabled = true\nport = 47732\nmdns = false\nreconnect = false\nseeds = [\"127.0.0.1:47731\"]\n",
        )
        .unwrap();
        assert_eq!(file.network.port, Some(47732));
        assert_eq!(file.network.mdns, Some(false));
        assert_eq!(file.network.reconnect, Some(false));
        assert_eq!(
            file.network.seeds.as_deref(),
            Some(&["127.0.0.1:47731".to_string()][..])
        );
    }

    #[test]
    fn config_file_defaults() {
        let file: ConfigFile = toml::from_str("").unwrap();
        assert!(file.identity.name.is_none());
        assert!(file.history.enabled.is_none());
    }
}
