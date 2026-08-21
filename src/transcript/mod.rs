// transcript/mod.rs
//
// 解析各家 coding agent 的 session jsonl，抽出「真正是使用者手打的文字」。
// 每家 agent 的格式知識關在自己的子模組裡（serde 結構、雜訊標記、測試同檔），
// 這裡只做兩件事：把一行交給各家解析器認領、套用各家共通的最終檢查。
// 之後要支援新的 agent：加一個子模組、在下面的認領鏈補一個 or_else。

mod claude;
mod codex;

/// 從一行 jsonl 抽出使用者手打的文字。回傳 None 代表這行不是使用者的 prompt。
///
/// 逐家嘗試是安全的：各家的 type 值域不相交（Claude Code 是 "user" /
/// "queue-operation"，Codex 是 "response_item" 等），一行絕不會被兩家同時認領，
/// 也不會被錯的那家認成 prompt。
pub fn extract_user_text(raw: &str) -> Option<String> {
    let text = claude::extract(raw).or_else(|| codex::extract(raw))?;

    let text = text.trim().to_string();
    if text.is_empty() {
        return None;
    }
    // 沒有任何一個 ASCII 英文字母就當作純中文/純指令，跳過
    if !text.chars().any(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    // 只講評「純英文」的行：夾任何中日韓字元（含全形標點）就跳過。
    // 中文句夾英文詞（「我先本地 build 新版本」）是日常，送去講評只會逼模型
    // 腦補翻譯；含少量中文的英文句（"How do I say 蟑螂"）是取捨後放棄的少數。
    if text.chars().any(is_cjk) {
        return None;
    }
    Some(text)
}

/// 中日韓字元（含全形標點）。範圍夠用就好，不追求 Unicode 完備：
/// 目的只是判斷「這行的主人是不是在打英文」。
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3000..=0x303F   // CJK 標點（。「」等）
        | 0x3040..=0x30FF // 日文假名
        | 0x3400..=0x4DBF // CJK 統一表意文字擴充 A
        | 0x4E00..=0x9FFF // CJK 統一表意文字
        | 0xAC00..=0xD7AF // 韓文音節
        | 0xF900..=0xFAFF // CJK 相容表意文字
        | 0xFF00..=0xFFEF // 全形字元（，！？等）
    )
}

#[cfg(test)]
mod tests {
    use super::extract_user_text;

    fn user_line(content: &str) -> String {
        format!(r#"{{"type":"user","message":{{"role":"user","content":"{content}"}}}}"#)
    }

    #[test]
    fn mixed_chinese_with_english_words_skipped() {
        // 中文句夾技術名詞是日常，不是在練英文，不該送講評
        assert_eq!(extract_user_text(&user_line("我先本地 build 新版本，已經確認並沒有抓到 shell 結果了")), None);
        assert_eq!(extract_user_text(&user_line("幫我 review 這段 code")), None);
    }

    #[test]
    fn english_with_a_few_cjk_chars_also_skipped() {
        // 「純英文才講評」的取捨：夾了任何中日韓字元就跳過
        assert_eq!(extract_user_text(&user_line("How do I say 蟑螂 in English?")), None);
    }

    #[test]
    fn fullwidth_punctuation_counts_as_cjk() {
        assert_eq!(extract_user_text(&user_line("ok，好")), None);
    }

    #[test]
    fn pure_english_reviewed() {
        assert_eq!(
            extract_user_text(&user_line("Please fix the login bug.")).as_deref(),
            Some("Please fix the login bug.")
        );
    }
}
