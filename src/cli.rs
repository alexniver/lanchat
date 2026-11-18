use std::path::PathBuf;

use anyhow::{bail, Result};

#[derive(Debug, Clone, Default)]
pub struct Cli {
    pub config: Option<PathBuf>,
    pub keymap: Option<PathBuf>,
    pub no_config: bool,
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
    "lanchat 0.1.0 — 单机聊天 TUI\n\
\n\
用法: lanchat [选项]\n\
\n\
选项:\n\
  --config <PATH>   使用指定 config.toml\n\
  --keymap <PATH>   使用指定 keymap.toml\n\
  --no-config       忽略用户配置和键位（使用默认）\n\
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
    fn rejects_unknown_argument() {
        assert!(parse_from(args(&["--nope"])).is_err());
    }

    #[test]
    fn rejects_missing_value() {
        assert!(parse_from(args(&["--config"])).is_err());
    }
}
