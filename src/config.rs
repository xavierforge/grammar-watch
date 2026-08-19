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

use crate::paths;
use crate::providers::Provider;

#[derive(Deserialize, Default, Debug, PartialEq)]
// 打錯欄位名要直接報錯，不能沉默忽略讓人以為設定有生效
#[serde(deny_unknown_fields)]
pub struct Config {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub preamble: Option<String>,
    pub log: Option<String>,
}

/// CLI 旗標、設定檔、內建預設三層合併後的定案值
pub struct Resolved {
    pub provider: Provider,
    pub model: String,
    /// None = 用內建的 PREAMBLE
    pub preamble: Option<String>,
    pub log: Option<PathBuf>,
}

impl Config {
    /// 套用優先序：CLI 旗標 > 設定檔 > 內建預設。
    /// 設定檔裡亂寫的 provider 要報錯，不能沉默退回預設。
    pub fn resolve(
        self,
        cli_provider: Option<Provider>,
        cli_model: Option<String>,
        cli_log: Option<PathBuf>,
    ) -> Result<Resolved> {
        let provider = match cli_provider {
            Some(p) => p,
            None => match &self.provider {
                Some(name) => Provider::from_name(name).with_context(|| {
                    format!(
                        "設定檔的 provider 不認得：{name}（可用：anthropic / openrouter / gemini / openai）"
                    )
                })?,
                None => Provider::Anthropic,
            },
        };
        let model = cli_model
            .or(self.model)
            .unwrap_or_else(|| provider.default_model().to_string());
        let log = cli_log.or_else(|| self.log.as_deref().map(paths::expand_tilde));
        Ok(Resolved { provider, model, preamble: self.preamble, log })
    }
}

/// 設定檔位置：$XDG_CONFIG_HOME（或 ~/.config）/grammar-watch/config.toml。
/// Windows 沒有 XDG 慣例，就直接用家目錄下的 .config，路徑各平台一致最好記。
fn config_path() -> Option<PathBuf> {
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(x).join("grammar-watch").join("config.toml"));
    }
    paths::home()
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

    #[test]
    fn cli_flag_beats_config() {
        let cfg = Config { provider: Some("openrouter".into()), ..Default::default() };
        let r = cfg.resolve(Some(Provider::Gemini), None, None).unwrap();
        assert!(matches!(r.provider, Provider::Gemini));
        // model 未指定 → 跟著定案後的 provider 走預設
        assert_eq!(r.model, Provider::Gemini.default_model());
    }

    #[test]
    fn config_beats_builtin_default() {
        let cfg = Config {
            provider: Some("openrouter".into()),
            model: Some("some/model".into()),
            ..Default::default()
        };
        let r = cfg.resolve(None, None, None).unwrap();
        assert!(matches!(r.provider, Provider::Openrouter));
        assert_eq!(r.model, "some/model");
    }

    #[test]
    fn everything_omitted_falls_back_to_anthropic() {
        let r = Config::default().resolve(None, None, None).unwrap();
        assert!(matches!(r.provider, Provider::Anthropic));
        assert_eq!(r.model, Provider::Anthropic.default_model());
        assert!(r.log.is_none());
        assert!(r.preamble.is_none());
    }

    #[test]
    fn bad_provider_in_config_is_an_error() {
        let cfg = Config { provider: Some("gpt".into()), ..Default::default() };
        assert!(cfg.resolve(None, None, None).is_err());
    }
}
