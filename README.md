# grammar-watch

監看單一 Claude Code session 的 jsonl，偵測到你新打的 prompt 就送給 LLM，
在終端機印出「你打了什麼 / 建議怎麼打 / 文法或單字的改進點」。

![demo：左邊打 prompt 給 Claude Code，右邊即時講評英文](demo/demo.gif)

## 安裝

不需要 Rust 工具鏈，依平台擇一即可：

```bash
# macOS / Linux：Homebrew
brew install xavierforge/tap/grammar-watch

# macOS / Linux：安裝腳本（自動偵測平台，含 Apple Silicon 和 Linux aarch64）
curl -LsSf https://github.com/xavierforge/grammar-watch/releases/latest/download/grammar-watch-installer.sh | sh
```

```powershell
# Windows：PowerShell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/xavierforge/grammar-watch/releases/latest/download/grammar-watch-installer.ps1 | iex"
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
grammar-watch --provider openrouter --model google/gemini-2.5-flash
```

### 設定檔（選用）

`~/.config/grammar-watch/config.toml`（設了 `XDG_CONFIG_HOME` 就以它為準）。
全部欄位都可省略，優先序是 CLI 旗標 > 設定檔 > 內建預設：

```toml
provider = "openrouter"                 # anthropic / openrouter / gemini / openai
model = "anthropic/claude-haiku-4.5"    # 省略就用該供應商的預設
log = "~/gw-journal.md"                 # 等同 --log：講評日誌
# preamble = """完全自訂講評的 system prompt（進階）"""
```

### 講評日誌

把每則講評附時間戳追加到一個本地檔案，拿來回顧自己常犯的錯最有用。開啟方式擇一：

```bash
# 臨時：加旗標
grammar-watch --log ~/gw-journal.md

# 常駐：寫進設定檔，之後每次自動記
mkdir -p ~/.config/grammar-watch
echo 'log = "~/gw-journal.md"' >> ~/.config/grammar-watch/config.toml
```

有生效的話，啟動時「模型：」下面會多一行「日誌：~/gw-journal.md」。

檔案是純文字（markdown 風格），每則講評一段：

```
## 2026-08-21 14:03:21

原文：<你實際打的字，多行照原樣保留>
建議：<建議句>
講評：<講評，含另一種說法>
```

常用看法：

```bash
tail -f ~/gw-journal.md            # 另開視窗即時看著它長
grep -c '^## ' ~/gw-journal.md     # 數一共記了幾則
```

行為細節：只有講評成功才寫入（API 錯誤不記）；檔案在第一筆寫入時才建立，
啟動當下不存在是正常的；寫入失敗只會印警告，不會中斷監看。

日誌累積一段時間後，直接餵給任何 LLM 就是你的「常犯錯誤週報」，例如：

```bash
cat ~/gw-journal.md | claude -p "歸納這份英文講評日誌最常見的錯誤模式：每類給出現次數、兩三個原文與建議的對照例句、一句針對性的改進建議"
```

搭配 cron 每週一早上跑一次，就是全自動的學習回顧。

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
