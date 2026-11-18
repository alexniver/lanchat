use std::path::PathBuf;

use anyhow::{bail, Result};

#[derive(Debug, Clone, Default)]
pub struct Cli {
    pub config: Option<PathBuf>,
    pub keymap: Option<PathBuf>,
    pub no_config: bool,
    pub no_network: bool,
    pub port: Option<u16>,
    pub connect: Vec<String>,
    pub debug: bool,
    pub version: bool,
    pub help: bool,
}

pub fn parse() -> Result<Cli> {
    parse_from(std::env::args().skip(1))
}

pub fn parse_from<I>(args: I) -> Result<Cli>
where
    I: IntoIterator<Item = String>,
{
    let mut cli = Cli::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => cli.config = Some(PathBuf::from(take_value("--config", &mut args)?)),
            "--keymap" => cli.keymap = Some(PathBuf::from(take_value("--keymap", &mut args)?)),
            "--no-config" => cli.no_config = true,
            "--no-network" => cli.no_network = true,
            "--port" => {
                let value = take_value("--port", &mut args)?;
                cli.port = Some(
                    value
                        .parse()
                        .map_err(|_| anyhow::anyhow!("无效端口: {value}"))?,
                );
            }
            "--connect" => cli.connect.push(take_value("--connect", &mut args)?),
            "--debug" => cli.debug = true,
            "--version" | "-V" => cli.version = true,
            "--help" | "-h" => cli.help = true,
            other => bail!("未知参数: {other}（使用 --help 查看用法）"),
        }
    }
    Ok(cli)
}

fn take_value<I>(name: &str, args: &mut I) -> Result<String>
where
    I: Iterator<Item = String>,
{
    args.next()
        .ok_or_else(|| anyhow::anyhow!("{name} 需要一个参数"))
}

pub fn help_text() -> &'static str {
    "lanchat 0.1.6 — 终端聊天 TUI\n\
\n\
用法: lanchat [选项]\n\
\n\
选项:\n\
  --config <PATH>   使用指定 config.toml\n\
  --keymap <PATH>   使用指定 keymap.toml\n\
  --no-config       忽略用户配置和键位（使用默认）\n\
  --no-network      完全使用本地模式，不启动网络\n\
  --port <PORT>     TCP 监听端口（默认 47731）\n\
  --connect <ADDR>  手动连接 host:port（可重复）\n\
  --debug           输出 debug 日志到文件\n\
  --version, -V     显示版本\n\
  --help, -h        显示帮助\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_flags_and_paths() {
        let cli = parse_from(args(&[
            "--config",
            "/tmp/c.toml",
            "--keymap",
            "/tmp/k.toml",
            "--no-config",
            "--debug",
        ]))
        .unwrap();
        assert_eq!(cli.config, Some(PathBuf::from("/tmp/c.toml")));
        assert_eq!(cli.keymap, Some(PathBuf::from("/tmp/k.toml")));
        assert!(cli.no_config);
        assert!(cli.debug);
    }

    #[test]
    fn parses_network_options() {
        let cli = parse_from(args(&[
            "--no-network",
            "--port",
            "47732",
            "--connect",
            "127.0.0.1:47731",
            "--connect",
            "192.168.1.2:47731",
        ]))
        .unwrap();
        assert!(cli.no_network);
        assert_eq!(cli.port, Some(47732));
        assert_eq!(cli.connect.len(), 2);
    }

    #[test]
    fn rejects_unknown_argument() {
        assert!(parse_from(args(&["--nope"])).is_err());
    }

    #[test]
    fn rejects_missing_value() {
        assert!(parse_from(args(&["--config"])).is_err());
    }
}
