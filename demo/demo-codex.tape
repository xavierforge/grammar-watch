# grammar-watch 的 Codex demo（vhs + herdr，全程跑在假 HOME 裡）
#
# 左邊：真的 Codex TUI（gpt-5.6-sol low，AGENTS.md 限一句話回覆）
# 右邊：grammar-watch 從 picker 開始：Tab 從 Claude Code 分頁切到 Codex 分頁
#       → 打字過濾出 playground → 等待新 session → 接上左邊的新 Codex
# 中段除了講評，也展示「不會被講評」的訊息：中文夾英文、$greet skill 呼叫
# （整份 SKILL.md 注入也不評）。假 HOME 由 setup-codex.sh 搭好，畫面上
# 零真實路徑；picker 的互動由 setup-codex.sh 的背景編排照節奏送鍵。
#
# 用法：在 demo/ 資料夾、shell 已 export ANTHROPIC_API_KEY 的狀態下：
#   vhs demo-codex.tape
# 產出 demo.gif 和 demo.mp4。

# 輸出用自己的檔名，確定滿意後再手動取代 demo.gif（README 引用的是那個）
Output demo-codex.gif
Output demo-codex.mp4

Set FontSize 22
Set Width 1800
Set Height 950
Set Padding 16
Set Theme "Catppuccin Mocha"
Set TypingSpeed 55ms
# 中文和符號要用有 CJK 字面的字型 fallback，不然全是亂碼豆腐；
# Nerd Font 放第一位，zsh prompt 的圖示才有字面
Set FontFamily "MesloLGLDZ Nerd Font, JetBrains Mono, PingFang TC, Hiragino Sans GB, Heiti TC"

# 隱藏階段：搭假 HOME、開 herdr、右邊 picker 就位
Hide
Type "./setup-codex.sh"
Enter
# 側邊欄由假 HOME 的 herdr 設定（sidebar_start_collapsed）直接收合，不用送 chord
Sleep 8
# 對時信號：這行打在左 pane 的 shell（隱藏階段，不入鏡），
# 背景編排等到這個檔案出現才開演 picker，Show 才拍得到 Tab 切換
Type "touch /private/tmp/gw-demo-home/.attached"
Enter
Sleep 0.5
Type "clear"
Enter
Sleep 0.7
Show

# 開場：背景編排在右邊演 picker（Tab 換到 Codex 分頁 → 過濾 → 等待新 session），
# 接著左邊自動打 codex 啟動。這段 vhs 只旁觀。
Sleep 13

# 四句示範。vhs 打進左邊（focus 一直在左）：
# 1. 有錯的英文 → 右邊講評
Type "please help me fix this bug, it very hard to solve"
Enter
Sleep 10

# 2. 中文夾英文 → 右邊安靜（不是在練英文，不評）
Type "幫我 review 這段 code，順便跑一下 tests"
Enter
Sleep 7

# 3. $greet skill：整份 SKILL.md 注入 → 右邊一樣安靜
Type "$greet"
Sleep 1.2
Enter
Sleep 0.5
Enter
Sleep 8

# 4. 再一句有錯的英文 → 右邊講評（證明它還在看）
Type "The retry logic seem not work when network is unstable"
Enter
Sleep 11

# 收尾：prefix+q detach，setup-codex.sh 接著收 session、刪假 HOME
Hide
Ctrl+S
Type "q"
Sleep 3
Type "clear"
Enter
Show
Type "brew install xavierforge/tap/grammar-watch"
Sleep 5
