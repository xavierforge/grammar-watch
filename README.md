# grammar-watch

監看單一 Claude Code session 的 jsonl，偵測到你新打的 prompt 就送給 LLM，
在終端機印出「你打了什麼 / 建議怎麼打 / 文法或單字的改進點」。

![demo：左邊打 prompt 給 Claude Code，右邊即時講評英文](demo/demo.gif)

## 安裝

不需要 Rust 工具鏈，擇一即可：

```bash
# Homebrew（macOS / Linux）
brew install xavierforge/tap/grammar-watch

# 或安裝腳本
curl -LsSf https://github.com/xavierforge/grammar-watch/releases/latest/download/grammar-watch-installer.sh | sh
```

有 Rust 的人也可以從原始碼裝：

```bash
cargo install --git https://github.com/xavierforge/grammar-watch
```

## 使用

```bash
export ANTHROPIC_API_KEY=sk-...

# 不帶參數：互動式選單（推薦）
grammar-watch

# 或直接指定檔案
grammar-watch /path/to/session.jsonl
```

在另一個 tmux window 或 Herdr 跑它，工作時瞄一眼即可。

### 互動式選單

不帶參數啟動會先列出 `~/.claude/projects` 底下的專案（顯示真實路徑、
session 數、最近活動時間，最新的排最前面），選了專案再選 session
（顯示時間、session id 前 8 碼、第一句 prompt 的預覽）。

按鍵：↑↓ 移動、Enter 或 → 確認、← 或 Esc 回上一層（← 在專案層沒作用，
Esc 在專案層是離開）、打字過濾、Ctrl-C 離開。

session 清單的第一個選項是「**等下一個新 session**」：session 的 jsonl
要等你打出第一句才會建檔，附著既有檔案永遠會漏掉第一句。選這個選項、
再去開新的 Claude Code，新檔一出現就自動接上並從頭讀，第一句也不會漏。

### 供應商與模型

預設 Anthropic（haiku）。用 `--provider` 換供應商、`--model` 換模型，
金鑰一律讀環境變數：

| provider     | 環境變數             | 預設模型                     |
|--------------|----------------------|------------------------------|
| `anthropic`  | `ANTHROPIC_API_KEY`  | `claude-haiku-4-5`           |
| `openrouter` | `OPENROUTER_API_KEY` | `anthropic/claude-haiku-4.5` |
| `gemini`     | `GEMINI_API_KEY`     | `gemini-2.5-flash`           |
| `openai`     | `OPENAI_API_KEY`     | `gpt-4.1-mini`               |

```bash
export OPENROUTER_API_KEY=sk-or-...
./target/release/grammar-watch --provider openrouter --model google/gemini-2.5-flash
```

### 其他

- 預設只看「啟動之後」的新 prompt。要從頭檢查整個 session 加 `--from-start`
  （`--from_start` 也可）；「等下一個新 session」模式一律從頭讀。
- **自動跟隨**：監看中若同一個專案出現新的 session（例如你按了 `/clear`
  或開了新對話），會自動切換過去從頭講評，不用重開。
- 純中文或純指令（沒有英文字母）的行會自動跳過。

## Claude Code 的 session jsonl 在哪

通常在：
```
~/.claude/projects/<專案路徑編碼>/<session-uuid>.jsonl
```
（互動式選單就是幫你翻這個資料夾，不用再手動撈檔名。）
