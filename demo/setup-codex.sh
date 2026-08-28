#!/bin/bash
# Codex demo 的場景搭建：整套跑在「假 HOME」裡，畫面上零真實路徑、零真實專案名。
# 左邊真的 Codex TUI，右邊 grammar-watch 從 picker 開始（Tab 切到 Codex 分頁給觀眾看）。
#
# 假 HOME（/tmp/gw-demo-home）內容：
#   demo/playground/AGENTS.md     讓 Codex 一句話回覆、不用工具
#   .agents/skills/greet/SKILL.md 迷你 skill：展示 $greet 注入整份 SKILL.md 也不會被講評
#   .claude/projects/…            捏造的 Claude 專案（撐起第一個分頁的清單）
#   .codex/sessions/…             捏造的 Codex 專案 + playground 的舊 session
#   .codex/auth.json              從真 HOME 複製（錄完整個假 HOME 會刪掉）
#
# 由 demo-codex.tape 在隱藏階段呼叫；結尾 attach 進 herdr 讓 vhs 接手打字。
set -euo pipefail

cd "$(dirname "$0")"
REAL_HOME="$HOME"
# 用 /private/tmp 的「解析後」寫法：Codex 記錄的 cwd 是解析後的路徑，
# HOME 與 cwd 字串一致，Codex 的 directory 顯示和 grammar-watch 的 tildify 才都會縮成 ~
FAKE="/private/tmp/gw-demo-home"

# ── 假 HOME 建置 ──────────────────────────────────────────────
rm -rf "$FAKE"
mkdir -p "$FAKE/demo/playground" "$FAKE/.agents/skills/greet" "$FAKE/.codex" "$FAKE/.config"

cat > "$FAKE/demo/playground/AGENTS.md" <<'EOF'
# Demo playground

This is a screen-recording demo. For every message:
- Reply with ONE short, friendly sentence.
- Never use tools. Never read or write files.
EOF

cat > "$FAKE/.agents/skills/greet/SKILL.md" <<'EOF'
---
name: greet
description: Greet the user warmly
---

Greet the user in one short cheerful sentence, then compliment their
excellent taste in command-line tools. Keep it to one line total.
EOF

# Codex 的憑證與設定：auth 從真 HOME 複製；設定極簡（信任 playground、低推理省時間）
cp "$REAL_HOME/.codex/auth.json" "$FAKE/.codex/auth.json"
cp "$REAL_HOME/.codex/version.json" "$FAKE/.codex/version.json" 2>/dev/null || true
cp "$REAL_HOME/.codex/installation_id" "$FAKE/.codex/installation_id" 2>/dev/null || true
cat > "$FAKE/.codex/config.toml" <<EOF
model = "gpt-5.6-sol"
model_reasoning_effort = "low"

[projects."$FAKE/demo/playground"]
trust_level = "trusted"

[tui.model_availability_nux]
"gpt-5.6-sol" = 1
EOF

# herdr 在假 HOME 下讀這份設定：側邊欄一開始就收合，
# 不用再靠 vhs 送 prefix+b chord（時機抓不準，翻過兩次車）
mkdir -p "$FAKE/.config/herdr"
cat > "$FAKE/.config/herdr/config.toml" <<'EOF'
[ui]
sidebar_start_collapsed = true

# tape 結尾用 prefix+q detach；假 HOME 讀不到真設定，prefix 要在這裡重申，
# 不然 Ctrl+S 會穿透進 pane（上次 qclear 打進 Codex 的原因）
[keys]
prefix = "ctrl+s"
EOF

# 假 HOME 下的 zsh：乾淨的極簡 prompt，並把真 HOME 底下的 bin 目錄補回 PATH。
# repo 本地編譯的 grammar-watch 排最前面：demo 常常帶著還沒發版的過濾修正
REPO_ROOT="$(dirname "$PWD")"
cat > "$FAKE/.zshrc" <<EOF
export PATH="$REPO_ROOT/target/release:\$PATH:$REAL_HOME/.local/bin:/opt/homebrew/bin"
PROMPT='%F{blue}%1~%f ❯ '
EOF

# 捏造兩個分頁的專案資料（名字走工程師迷因路線）：
#   Claude 分頁最近有動靜 → picker 開場停在 Claude Code 分頁，Tab 切過去才是 Codex
python3 - "$FAKE" <<'PYEOF'
import json, os, sys, time
fake = sys.argv[1]
now = time.time()

def claude_project(dirname, cwd, prompt, age_min):
    d = os.path.join(fake, ".claude", "projects", dirname)
    os.makedirs(d, exist_ok=True)
    p = os.path.join(d, "a1b2c3d4-0000-0000-0000-000000000000.jsonl")
    with open(p, "w") as f:
        f.write(json.dumps({"type": "user", "cwd": cwd,
            "message": {"role": "user", "content": prompt}}) + "\n")
    t = now - age_min * 60
    os.utime(p, (t, t))

def codex_rollout(day_offset, name, cwd, prompt, age_min):
    day = time.strftime("%Y/%m/%d", time.localtime(now - day_offset * 86400))
    d = os.path.join(fake, ".codex", "sessions", day)
    os.makedirs(d, exist_ok=True)
    p = os.path.join(d, name)
    with open(p, "w") as f:
        f.write(json.dumps({"timestamp": "t", "type": "session_meta",
            "payload": {"cwd": cwd}}) + "\n")
        f.write(json.dumps({"type": "response_item", "payload": {"type": "message",
            "role": "user", "content": [{"type": "input_text", "text": prompt}]}}) + "\n")
    t = now - age_min * 60
    os.utime(p, (t, t))

home = fake
claude_project("-work-todo-app-final-v2-FINAL", f"{home}/work/todo-app-final-v2-FINAL",
    "add one more feature and then we ship, I promise", 4)
claude_project("-work-legacy-do-not-touch", f"{home}/work/legacy-do-not-touch",
    "who wrote this mess... git blame says it was me", 27)
claude_project("-side-ai-pet-rock", f"{home}/side/ai-pet-rock",
    "make the rock respond with more enthusiasm", 55)

codex_rollout(0, "rollout-2026-a-01aaaaaa-1111-7000-8000-b10ckcha1n01.jsonl",
    f"{home}/work/blockchain-pivot-no3",
    "pivot the pivot back to the first pivot", 130)
codex_rollout(0, "rollout-2026-b-01bbbbbb-2222-7000-8000-notskynet002.jsonl",
    f"{home}/side/definitely-not-skynet",
    "the model keeps refusing to open the pod bay doors", 300)
codex_rollout(1, "rollout-2026-c-01cccccc-3333-7000-8000-p1ayground03.jsonl",
    f"{home}/demo/playground",
    "hello there, quick test session", 1600)
PYEOF

# 只建假 HOME（給無頭驗證用），不開錄影場景
if [[ "${1:-}" == "--build-only" ]]; then
    echo "fakehome ready: $FAKE"
    exit 0
fi

# ── herdr 場景（全程 HOME=假 HOME，herdr 狀態也隔離在裡面）──────────
# vhs 的錄影終端機是從 herdr pane 裡生出來的，帶著 HERDR_* 環境變數，
# herdr 會拒絕巢狀啟動；清掉之後才能開獨立的 demo session。
unset HERDR_ENV HERDR_PANE_ID HERDR_SOCKET_PATH HERDR_TAB_ID HERDR_WORKSPACE_ID
export HOME="$FAKE"
cd "$FAKE/demo/playground"

herdr session stop gwdemo 2>/dev/null || true
herdr session delete gwdemo 2>/dev/null || true

# 背景編排：搭場景 + 照節奏演 picker（vhs 那邊只管左邊打字，不用切 focus）
(
    export HERDR_SOCKET_PATH="$HOME/.config/herdr/sessions/gwdemo/herdr.sock"
    sleep 3

    LEFT=$(herdr pane list | python3 -c "import json,sys; print(json.load(sys.stdin)['result']['panes'][0]['pane_id'])")
    RIGHT=$(herdr pane split "$LEFT" --direction right --ratio 0.5 \
        | python3 -c "import json,sys; print(json.load(sys.stdin)['result']['pane']['pane_id'])")
    sleep 1

    # 右邊：picker 開起來停在 Claude Code 分頁，等 vhs 的 Show 之後才開演
    herdr pane run "$RIGHT" 'grammar-watch' > /dev/null

    # 等 vhs 的信號檔（tape 在 Hide 階段 touch 它）出現才開演，不賭秒數：
    # 上次搶跑，Tab 切分頁發生在 Show 之前，開場重點沒入鏡
    for _ in $(seq 1 60); do
        [[ -f "$FAKE/.attached" ]] && break
        sleep 0.5
    done
    sleep 2.5

    # ── 這裡開始入鏡 ──
    # Tab 切到 Codex 分頁 → 打字過濾出 playground → 選定 → 等待新 session
    herdr pane send-keys "$RIGHT" Tab > /dev/null
    sleep 2
    herdr pane send-text "$RIGHT" 'play' > /dev/null
    sleep 1.5
    herdr pane send-keys "$RIGHT" Enter > /dev/null
    sleep 2
    herdr pane send-keys "$RIGHT" Enter > /dev/null
    sleep 1

    # 左邊：開 Codex，grammar-watch 隨即「接上新 session」。
    # 先清掉輸入行：萬一有按鍵漂進左邊（例如 chord 沒成立漏出的字），指令才不會歪掉
    herdr pane send-keys "$LEFT" ctrl+c > /dev/null 2>&1 || true
    herdr pane send-text "$LEFT" 'codex' > /dev/null
    herdr pane send-keys "$LEFT" Enter > /dev/null
) &

# 不能用 exec：attach 結束（tape 按 prefix+q detach）之後，
# 還要靠這個 shell 收掉 session、刪掉假 HOME（裡面有 auth.json 複本）
herdr --session gwdemo
herdr session stop gwdemo > /dev/null 2>&1 || true
rm -rf "$FAKE"
