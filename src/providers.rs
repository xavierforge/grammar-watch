// providers.rs
//
// 把四家供應商（Anthropic / OpenRouter / Gemini / OpenAI）統一成一個介面。
// rig 的 Agent 是泛型型別、每家 provider 都不同，直接存起來型別會寫不完，
// 所以這裡把它包成「丟一句 String 進去、拿回覆 String 出來」的閉包（Asker），
// main 那邊完全不用碰泛型。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::ValueEnum;
use rig::client::{CompletionClient, ProviderClient};
use rig::completion::{Prompt, PromptError};
use rig::providers::{anthropic, gemini, openai, openrouter};

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Provider {
    Anthropic,
    Openrouter,
    Gemini,
    Openai,
}

impl Provider {
    /// 這家供應商的 API key 要讀哪個環境變數
    pub fn env_key(self) -> &'static str {
        match self {
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::Openrouter => "OPENROUTER_API_KEY",
            Provider::Gemini => "GEMINI_API_KEY",
            Provider::Openai => "OPENAI_API_KEY",
        }
    }

    /// 沒指定 --model 時用的預設模型：都挑「便宜、快」的那一階，
    /// 這工具每句 prompt 都要打一次 API，用大模型太浪費。
    pub fn default_model(self) -> &'static str {
        match self {
            Provider::Anthropic => "claude-haiku-4-5",
            Provider::Openrouter => "anthropic/claude-haiku-4.5",
            Provider::Gemini => "gemini-2.5-flash",
            Provider::Openai => "gpt-4.1-mini",
        }
    }

    /// 設定檔用：從名字（不分大小寫）解析供應商
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "anthropic" => Some(Provider::Anthropic),
            "openrouter" => Some(Provider::Openrouter),
            "gemini" => Some(Provider::Gemini),
            "openai" => Some(Provider::Openai),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::Openrouter => "openrouter",
            Provider::Gemini => "gemini",
            Provider::Openai => "openai",
        }
    }
}

/// 統一後的介面：一個可以重複呼叫的非同步閉包。
pub type Asker =
    Box<dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<String, PromptError>> + Send>> + Send + Sync>;

/// 把任何一個 rig Agent 包成 Asker。Arc 是因為閉包每次呼叫都要 clone 一份
/// 進 async block（future 要 'static，不能借用閉包裡的 agent）。
fn wrap<A>(agent: A) -> Asker
where
    A: Prompt + Send + Sync + 'static,
{
    let agent = Arc::new(agent);
    Box::new(move |msg| {
        let agent = agent.clone();
        Box::pin(async move { agent.prompt(msg).await })
    })
}

/// 講評要的是穩定一致的輸出，不是創意發揮，所以溫度壓低
const TEMPERATURE: f64 = 0.3;

/// 泛型建構：不同供應商只差 client 型別，其餘（from_env、agent 鏈）都一樣。
/// where 的最後一條直接寫出 wrap 需要的能力，剩下讓編譯器推。
fn agent_from_env<C>(model: &str, preamble: &str) -> Result<Asker>
where
    C: ProviderClient + CompletionClient,
    C::Error: std::error::Error + Send + Sync + 'static,
    rig::agent::Agent<<C as CompletionClient>::CompletionModel>: Prompt + Send + Sync + 'static,
{
    let client = C::from_env()?;
    Ok(wrap(
        client
            .agent(model)
            .preamble(preamble)
            .temperature(TEMPERATURE)
            .build(),
    ))
}

/// 建立指定供應商的 agent。金鑰一律走環境變數（rig 的 from_env），
/// 這裡先自己檢查一次，缺了就給出比 rig 錯誤更直白的訊息。
pub fn build(provider: Provider, model: &str, preamble: &str) -> Result<Asker> {
    let env = provider.env_key();
    if std::env::var(env).is_err() {
        anyhow::bail!(
            "找不到 {env}。請先 export {env}=你的key\n\
             也可以用 --provider 換別家供應商：anthropic / openrouter / gemini / openai\n\
             （金鑰分別讀 ANTHROPIC_API_KEY / OPENROUTER_API_KEY / GEMINI_API_KEY / OPENAI_API_KEY，\
             模型用 --model 指定，省略就用該家的預設）"
        );
    }

    match provider {
        Provider::Anthropic => agent_from_env::<anthropic::Client>(model, preamble),
        Provider::Openrouter => agent_from_env::<openrouter::Client>(model, preamble),
        Provider::Gemini => agent_from_env::<gemini::Client>(model, preamble),
        Provider::Openai => agent_from_env::<openai::Client>(model, preamble),
    }
    .with_context(|| format!("建立 {} client 失敗", provider.name()))
}
