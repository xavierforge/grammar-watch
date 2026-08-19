// config.rs
//
// 選用的設定檔：~/.config/grammar-watch/config.toml（設了 XDG_CONFIG_HOME 就用它）。
// 全部欄位都可省略；優先序是 CLI 旗標 > 設定檔 > 內建預設。
//
//   provider = "openrouter"               # anthropic / openrouter / gemini / openai
//   model = "anthropic/claude-haiku-4.5"  # 省略就用該供應商的預設
//   log = "~/gw-journal.md"               # 等同 --log：講評日誌
//   preamble = """自訂講評的 system prompt（進階）"""

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize, Default, Debug, PartialEq)]
// 打錯欄位名要直接報錯，不能沉默忽略讓人以為設定有生效
#[serde(deny_unknown_fields)]
pub struct Config {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub preamble: Option<String>,
    pub log: Option<String>,
}

/// 設定檔位置：$XDG_CONFIG_HOME（或 ~/.config）/grammar-watch/config.toml
fn config_path() -> Option<PathBuf> {
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(x).join("grammar-watch").join("config.toml"));
    }
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config").join("grammar-watch").join("config.toml"))
}

/// 讀設定檔。檔案不存在就當全預設；存在但格式錯誤要報錯，不能吞掉。
pub fn load() -> Result<Config> {
    let Some(path) = config_path() else {
        return Ok(Config::default());
    };
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Ok(Config::default()),
    };
    toml::from_str(&raw).with_context(|| format!("設定檔解析失敗：{}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_config_parses() {
        let cfg: Config = toml::from_str(
            r#"
provider = "openrouter"
model = "anthropic/claude-haiku-4.5"
log = "~/j.md"
preamble = "自訂"
"#,
        )
        .unwrap();
        assert_eq!(cfg.provider.as_deref(), Some("openrouter"));
        assert_eq!(cfg.model.as_deref(), Some("anthropic/claude-haiku-4.5"));
        assert_eq!(cfg.log.as_deref(), Some("~/j.md"));
        assert_eq!(cfg.preamble.as_deref(), Some("自訂"));
    }

    #[test]
    fn empty_config_is_all_defaults() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn unknown_field_is_an_error() {
        // 打錯欄位名（例如 provder）必須報錯，不能讓人以為有生效
        assert!(toml::from_str::<Config>(r#"provder = "openai""#).is_err());
    }
}
