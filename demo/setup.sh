#!/bin/bash
# demo 錄影的場景搭建：tmux 左右分割，左邊真的 Claude Code（haiku）、
# 右邊 grammar-watch 進入「等待下一個新 Session」狀態。
# 由 demo.tape 在隱藏階段呼叫，結尾 attach 進 tmux 讓 vhs 接手打字。
set -euo pipefail

cd "$(dirname "$0")/playground"

# 這個資料夾若從沒開過 Claude Code，~/.claude/projects 底下就沒有它的資料夾，
# 選單會找不到專案。先用 print 模式跑一次把它生出來（也順便信任資料夾）。
if ! ls ~/.claude/projects | grep -q "grammar-watch-demo-playground"; then
    claude --model haiku -p "回 ok 就好" > /dev/null
fi

tmux kill-session -t gwdemo 2>/dev/null || true
tmux new-session -d -s gwdemo -c "$PWD"
tmux split-window -h -t gwdemo -c "$PWD"

# 右邊：開 grammar-watch，用過濾字串鎖定本資料夾（也避免其他專案名稱入鏡），
# 選完專案後第一個選項就是「等待下一個新 Session」，直接 Enter。
tmux send-keys -t gwdemo.1 'grammar-watch' Enter
sleep 2
tmux send-keys -t gwdemo.1 'demo/playground'
sleep 1
tmux send-keys -t gwdemo.1 Enter
sleep 1
tmux send-keys -t gwdemo.1 Enter

# 左邊：開真的 Claude Code，用 haiku 讓回應快
tmux send-keys -t gwdemo.0 'claude --model haiku' Enter
sleep 6
# 若跳出「信任這個資料夾嗎」對話框就按掉；已信任的話這一下沒有作用
tmux send-keys -t gwdemo.0 Enter
sleep 1

tmux select-pane -t gwdemo.0
exec tmux attach -t gwdemo
