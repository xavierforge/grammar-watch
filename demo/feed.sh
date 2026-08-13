#!/bin/bash
# demo 餵稿器：照節奏把示範 prompt 寫進一個假的 session jsonl，
# 讓 grammar-watch 即時講評。錄影時在背景跑，不需要真的開 Claude Code。
#
# 用法：./feed.sh /tmp/gw-demo.jsonl
set -euo pipefail

FILE="${1:?用法：feed.sh <jsonl路徑>}"
: > "$FILE"

# 等 vhs 那邊打完指令、grammar-watch 開始監看
sleep 6

emit() {
    printf '{"type":"user","message":{"role":"user","content":"%s"}}\n' "$1" >> "$FILE"
    sleep 9
}

# 每句展示一種台灣工程師常見錯誤，最後一句是正確的（展示不會硬挑毛病）
emit 'please help me fix this bug, it very hard to solve'
emit 'This function I think have problem when input is empty'
emit 'Despite the test is passed, the feature still broken on production'
emit 'when user click button, need to show error message'
emit 'I refactor it yesterday but it break other tests, can you check it'
emit 'Here is some problems I found:\n1. the api return wrong datas when page is zero\n2. loading spinner not disappear after finish\nplease fix all of them'
emit 'Add a retry with exponential backoff to the fetch helper, capped at three attempts.'
