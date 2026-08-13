#!/bin/bash
# demo 錄影的場景搭建（herdr 版）：左右分割，左邊真的 Claude Code（cld、haiku）、
# 右邊 grammar-watch 進入「等待下一個新 Session」狀態。
# 由 demo.tape 在隱藏階段呼叫，結尾 attach 進 herdr 讓 vhs 接手打字。
set -euo pipefail

cd "$(dirname "$0")/playground"

# 這個資料夾若從沒開過 Claude Code，~/.claude/projects 底下就沒有它的資料夾，
# 選單會找不到專案。先用 print 模式跑一次把它生出來。
if ! ls ~/.claude/projects | grep -q "grammar-watch-demo-playground"; then
    claude --model haiku -p "回 ok 就好" > /dev/null
fi

# vhs 的錄影終端機是從 herdr pane 裡生出來的，帶著 HERDR_* 環境變數，
# herdr 會拒絕巢狀啟動；清掉之後才能開獨立的 demo session。
unset HERDR_ENV HERDR_PANE_ID HERDR_SOCKET_PATH HERDR_TAB_ID HERDR_WORKSPACE_ID

herdr session stop gwdemo 2>/dev/null || true
herdr session delete gwdemo 2>/dev/null || true

# 背景編排：等 attach 起來後搭場景。所有 herdr 指令都指向 demo session 的 socket，
# 不會碰到平常在用的那個 workspace。
(
    export HERDR_SOCKET_PATH="$HOME/.config/herdr/sessions/gwdemo/herdr.sock"
    sleep 3

    LEFT=$(herdr pane list | python3 -c "import json,sys; print(json.load(sys.stdin)['result']['panes'][0]['pane_id'])")
    RIGHT=$(herdr pane split "$LEFT" --direction right --ratio 0.5 \
        | python3 -c "import json,sys; print(json.load(sys.stdin)['result']['pane']['pane_id'])")
    sleep 1

    # 右邊：grammar-watch → 過濾出 playground（其他專案名稱不入鏡）→ 等待新 session
    herdr pane run "$RIGHT" 'grammar-watch' > /dev/null
    sleep 2
    herdr pane send-text "$RIGHT" 'demo/playground' > /dev/null
    sleep 1
    herdr pane send-keys "$RIGHT" Enter > /dev/null
    sleep 1
    herdr pane send-keys "$RIGHT" Enter > /dev/null

    # 左邊：cld（= claude --dangerously-skip-permissions）+ haiku，回應快
    herdr pane send-text "$LEFT" 'cld --model haiku' > /dev/null
    herdr pane send-keys "$LEFT" Enter > /dev/null
    sleep 6
    # 若跳出「信任這個資料夾嗎」對話框就按掉；已信任的話這一下沒有作用
    herdr pane send-keys "$LEFT" Enter > /dev/null
) &

# 不能用 exec：attach 結束（tape 按 prefix+q detach）之後，
# 還要靠這個 shell 收掉 session、讓 vhs 打結尾的 CTA
herdr --session gwdemo
herdr session stop gwdemo > /dev/null 2>&1 || true
