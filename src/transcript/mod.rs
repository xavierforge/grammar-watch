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
    Some(text)
}
