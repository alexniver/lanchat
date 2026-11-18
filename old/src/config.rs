use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 节点身份配置，存储在 ~/.config/lanchat/identity.json
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Identity {
    /// 全局唯一的节点标识（UUIDv4）
    pub node_id: String,
    /// 显示名称，默认 username@hostname，用户可手动修改
    pub display_name: String,
}

/// 程序运行配置
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// 节点身份
    pub identity: Identity,
    /// TCP 监听端口
    pub port: u16,
    /// 自定义身份文件路径
    pub identity_path: Option<PathBuf>,
}

/// 加载或创建 identity 文件
///
/// - 若 identity 文件已存在，读取并返回。
/// - 若不存在，生成新 identity 并持久化为 JSON。
pub fn load_or_create_identity() -> anyhow::Result<Identity> {
    let path = identity_path()?;

    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let identity: Identity = serde_json::from_str(&content)?;
        tracing::info!(
            "已加载节点身份: {} ({})",
            identity.display_name,
            identity.node_id
        );
        Ok(identity)
    } else {
        let user = whoami::username();
        let host = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string());
        let display_name = format!("{}@{}", user, host);

        let identity = Identity {
            node_id: uuid::Uuid::new_v4().to_string(),
            display_name,
        };

        // 确保目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&identity)?;
        fs::write(&path, json)?;

        tracing::info!(
            "已生成新节点身份: {} ({})",
            identity.display_name,
            identity.node_id
        );

        Ok(identity)
    }
}

/// identity.json 的完整路径
pub fn identity_path() -> anyhow::Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("无法确定系统配置目录"))?;
    Ok(base.join("lanchat").join("identity.json"))
}

/// 本地文件共享目录：~/.local/share/lanchat/
pub fn local_share_dir() -> anyhow::Result<PathBuf> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("无法确定本地数据目录"))?;
    Ok(base.join("lanchat"))
}

/// 确保本地文件共享目录存在
pub fn ensure_local_share_dir() -> anyhow::Result<PathBuf> {
    let dir = local_share_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
        tracing::info!("已创建本地共享目录: {}", dir.display());
    }
    Ok(dir)
}

/// 解析命令行参数
pub fn parse_args() -> AppConfig {
    let args: Vec<String> = std::env::args().collect();
    let mut port: u16 = 47731;
    let mut identity_path: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u16>() {
                        Ok(p) => port = p,
                        Err(_) => {
                            eprintln!("警告：无效的端口号 '{}'，使用默认端口 {}", args[i + 1], port);
                        }
                    }
                    i += 1;
                }
            }
            "--identity" => {
                if i + 1 < args.len() {
                    identity_path = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--name" => {
                // --name 将在后续版本用于覆盖 display_name
                if i + 1 < args.len() {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // 加载或创建 identity
    let identity = if let Some(ref path) = identity_path {
        load_identity_from_path(path).unwrap_or_else(|e| {
            eprintln!("警告：无法加载身份文件 {}: {}", path.display(), e);
            load_or_create_identity_fallback()
        })
    } else {
        load_or_create_identity().unwrap_or_else(|e| {
            eprintln!("警告：无法加载/创建身份配置: {}", e);
            load_or_create_identity_fallback()
        })
    };

    AppConfig { identity, port, identity_path }
}

fn load_or_create_identity_fallback() -> Identity {
    Identity {
        node_id: uuid::Uuid::new_v4().to_string(),
        display_name: format!(
            "{}@{}",
            whoami::username(),
            hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "unknown".to_string())
        ),
    }
}

fn load_identity_from_path(path: &PathBuf) -> anyhow::Result<Identity> {
    let content = fs::read_to_string(path)?;
    let identity: Identity = serde_json::from_str(&content)?;
    tracing::info!(
        "已从 {} 加载节点身份: {} ({})",
        path.display(),
        identity.display_name,
        identity.node_id
    );
    Ok(identity)
}
