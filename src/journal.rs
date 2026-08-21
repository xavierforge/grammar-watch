// journal.rs
//
// 講評日誌：--log（或設定檔的 log）有指定檔案時，把每一則講評附時間戳追加進去。
// 終端機的輸出捲走就沒了；學英文真正值錢的是「自己犯過什麼錯」的紀錄，
// 這個檔案就是之後回顧、統計常犯錯誤的素材。

use std::io::Write;
use std::path::Path;

use crate::feedback::Feedback;

/// 組一則日誌（純函式，好測）。「原文」用使用者實際打的字、保留換行，
/// 不用模型貼回的原句（那是被併成一行的版本）。
fn entry(timestamp: &str, prompt: &str, fb: &Feedback) -> String {
    let mut out = format!("## {timestamp}\n\n原文：{prompt}\n");
    if let Some(s) = &fb.sugg {
        out.push_str("建議：");
        out.push_str(s);
        out.push('\n');
    }
    for (i, c) in fb.comments.iter().enumerate() {
        if i == 0 {
            out.push_str("講評：");
        }
        out.push_str(c);
        out.push('\n');
    }
    out.push('\n');
    out
}

/// 追加到日誌檔。寫失敗不該打斷監看，印個警告繼續。
pub fn append(path: &Path, prompt: &str, fb: &Feedback) {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let text = entry(&ts, prompt, fb);
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    // 日誌存著所有 prompt 的副本：建檔時鎖成只有自己可讀寫（600）。
    // mode 只在「建立新檔」時生效，既有檔案的權限不會被動到。
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let res = opts.open(path).and_then(|mut f| f.write_all(text.as_bytes()));
    if let Err(e) = res {
        eprintln!("寫入日誌失敗（{}）：{e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_contains_all_sections() {
        let fb = Feedback {
            orig: Some("x".into()),
            sugg: Some("I have an apple".into()),
            comments: vec!["has 要改 have。".into(), "另一種說法：An apple is mine.".into()],
        };
        let e = entry("2026-08-19 12:00:00", "I has a apple", &fb);
        assert!(e.starts_with("## 2026-08-19 12:00:00\n"));
        assert!(e.contains("原文：I has a apple\n"));
        assert!(e.contains("建議：I have an apple\n"));
        assert!(e.contains("講評：has 要改 have。\n另一種說法：An apple is mine.\n"));
    }

    #[test]
    fn multiline_prompt_kept_verbatim() {
        let fb = Feedback::default();
        let e = entry("t", "line one\nline two", &fb);
        assert!(e.contains("原文：line one\nline two\n"));
    }

    #[cfg(unix)]
    #[test]
    fn log_file_created_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let path =
            std::env::temp_dir().join(format!("gw-journal-perm-test-{}.md", std::process::id()));
        let _ = std::fs::remove_file(&path);
        append(&path, "hello", &Feedback::default());
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let _ = std::fs::remove_file(&path);
        assert_eq!(mode, 0o600);
    }
}
