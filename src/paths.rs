// paths.rs
//
// 路徑顯示的共用工具：印給使用者看的路徑一律把家目錄縮成 ~。
// 除了短一點好讀，更重要的是使用者錄影、截圖分享畫面時，
// 不會把自己的帳號名稱（/Users/某某）洩漏出去。

use std::path::Path;

/// $HOME 前綴縮成 ~。
/// Claude Code 會把專案路徑編碼成資料夾名稱（/Users/xxx → -Users-xxx），
/// 那個形式一樣會洩漏帳號名稱，所以編碼過的家目錄也一併縮成 ~。
pub fn tildify(path: &str) -> String {
    let Ok(home) = std::env::var("HOME") else {
        return path.to_string();
    };
    let mut out = match path.strip_prefix(&home) {
        Some(rest) => format!("~{rest}"),
        None => path.to_string(),
    };
    let encoded: String = home
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    if !encoded.is_empty() {
        out = out.replace(&encoded, "~");
    }
    out
}

/// Path 版：display 之後 tildify
pub fn display(path: &Path) -> String {
    tildify(&path.display().to_string())
}
