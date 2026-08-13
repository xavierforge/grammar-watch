# grammar-watch 真互動 demo（vhs + herdr）
#
# 左邊：真的 Claude Code（cld + haiku，playground/CLAUDE.md 限制它一句話回覆）
# 右邊：grammar-watch 先進「等待下一個新 Session」，第一句 prompt 就開始講評
# 場景由 setup.sh 在隱藏階段用 herdr 搭好，vhs 只負責把七句打進左邊。全程真實互動。
#
# 用法：在 demo/ 資料夾、shell 已 export ANTHROPIC_API_KEY 的狀態下：
#   vhs demo.tape
# 產出 demo.gif 和 demo.mp4。

Output demo.gif
Output demo.mp4

Set FontSize 22
Set Width 1800
Set Height 950
Set Padding 16
Set Theme "Catppuccin Mocha"
Set TypingSpeed 55ms
# 中文和符號要用有 CJK 字面的字型 fallback，不然全是亂碼豆腐
Set FontFamily "JetBrains Mono, PingFang TC, Hiragino Sans GB, Heiti TC"

# 隱藏階段：setup.sh 用 herdr 搭場景（選專案、進等待狀態、開 Claude Code）
Hide
Type "./setup.sh"
Enter
Sleep 18
Show

# 七句示範：每句一種台灣工程師常見錯誤，打完等左邊回覆＋右邊講評
Type "please help me fix this bug, it very hard to solve"
Enter
Sleep 13

Type "This function I think have problem when input is empty"
Enter
Sleep 13

Type "Despite the test is passed, the feature still broken on production"
Enter
Sleep 13

Type "when user click button, need to show error message"
Enter
Sleep 13

Type "I refactor it yesterday but it break other tests, can you check it"
Enter
Sleep 13

Type "Here is some problems I found: the api return wrong datas when page is zero, and loading spinner not disappear after finish, please fix all of them"
Enter
Sleep 14

Type "Add a retry with exponential backoff to the fetch helper, capped at three attempts."
Enter
Sleep 13

# 等 setup.sh 的定時收場把 demo session 停掉（時間互相對齊），shell 拿回控制權
Sleep 22
Hide
Sleep 3
Type "clear"
Enter
Show
Type "brew install xavierforge/tap/grammar-watch"
Sleep 5
