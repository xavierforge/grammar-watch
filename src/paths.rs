// paths.rs
//
// 路徑顯示的共用工具：印給使用者看的路徑一律把家目錄縮成 ~。
// 除了短一點好讀，更重要的是使用者錄影、截圖分享畫面時，
// 不會把自己的帳號名稱（/Users/某某）洩漏出去。
//
// 邏輯本體都是「吃 home 參數」的純函式，對外的版本再去讀 $HOME，
// 測試才不用碰全域環境變數。

use std::path::{Path, PathBuf};

/// 家目錄：Unix 讀 HOME；Windows 的 PowerShell 沒有 HOME，退而讀 USERPROFILE
pub fn home() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}

/// $HOME 前綴縮成 ~。
/// Claude Code 會把專案路徑編碼成資料夾名稱（/Users/xxx → -Users-xxx），
/// 那個形式一樣會洩漏帳號名稱，所以編碼過的家目錄也一併縮成 ~。
pub fn tildify(path: &str) -> String {
    match home() {
        Some(home) => tildify_with(path, &home),
        None => path.to_string(),
    }
}

fn tildify_with(path: &str, home: &str) -> String {
    let mut out = match path.strip_prefix(home) {
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

/// 反向：「~/x」展開成家目錄路徑。設定檔裡寫 ~ 很自然，直接支援。
pub fn expand_tilde(path: &str) -> PathBuf {
    match home() {
        Some(home) => expand_tilde_with(path, &home),
        None => PathBuf::from(path),
    }
}

fn expand_tilde_with(path: &str, home: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => PathBuf::from(home).join(rest),
        None => PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/Users/somebody";

    #[test]
    fn home_prefix_becomes_tilde() {
        assert_eq!(tildify_with("/Users/somebody/proj/x", HOME), "~/proj/x");
    }

    #[test]
    fn encoded_home_in_dir_names_also_shortened() {
        // Claude Code 的專案資料夾名稱是編碼過的路徑，帳號名稱藏在裡面
        assert_eq!(
            tildify_with("/Users/somebody/.claude/projects/-Users-somebody-proj/a.jsonl", HOME),
            "~/.claude/projects/~-proj/a.jsonl"
        );
    }

    #[test]
    fn paths_outside_home_untouched() {
        assert_eq!(tildify_with("/tmp/x", HOME), "/tmp/x");
    }

    #[test]
    fn tilde_expands_to_home() {
        assert_eq!(expand_tilde_with("~/j.md", HOME), PathBuf::from("/Users/somebody/j.md"));
    }

    #[test]
    fn absolute_path_not_expanded() {
        assert_eq!(expand_tilde_with("/var/log/j.md", HOME), PathBuf::from("/var/log/j.md"));
    }
}
