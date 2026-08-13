# grammar-watch 的純腳本 demo（vhs）：不開 Claude Code，由 feed.sh 餵假 session。
# 想要有真的 Claude Code 互動畫面，用 demo.tape。
#
# 用法：在 demo/ 資料夾、且 shell 已 export ANTHROPIC_API_KEY 的狀態下：
#   vhs demo.tape
# 產出 demo.gif 和 demo.mp4。全程自動：feed.sh 在背景照節奏餵 prompt，
# 畫面只錄 grammar-watch 的講評輸出，大字特寫。

Output demo.gif
Output demo.mp4

Set FontSize 26
Set Width 1400
Set Height 900
Set Padding 24
Set Theme "Catppuccin Mocha"
Set TypingSpeed 60ms
# 中文和符號要用有 CJK 字面的字型 fallback，不然全是亂碼豆腐
Set FontFamily "MesloLGLDZ Nerd Font, JetBrains Mono, PingFang TC, Hiragino Sans GB, Heiti TC"

# 背景啟動餵稿器（觀眾看不到這行）
Hide
Type "./feed.sh /tmp/gw-demo.jsonl >/dev/null 2>&1 & clear"
Enter
Sleep 1
Show

# 正片：啟動監看，講評一句句出現
Type "grammar-watch /tmp/gw-demo.jsonl"
Enter
Sleep 78

# 收尾：安裝指令當 CTA
Hide
Ctrl+C
Type "clear"
Enter
Show
Type "brew install xavierforge/tap/grammar-watch"
Sleep 5
