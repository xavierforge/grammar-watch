// transcript/codex.rs
//
// OpenAI Codex CLI 的 rollout jsonl：~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl。
// 行結構比 Claude Code 多一層包裝：{timestamp, ordinal, type, payload}。
// 使用者手打的字：type=="response_item"、payload.type=="message"、
// payload.role=="user"，文字在 content[] 的 input_text（實測固定單一區塊，
// 多行輸入的換行保留在同一個 text 裡）。
//
// 實測（codex-cli 0.148.0）確認過的簡化：插話（queue）在被處理時就存成
// 普通 user message，不需要特別撈；/model、/new 這類 slash 指令由 TUI
// 自己處理、不會寫成 user message；reasoning 和 tool call 是別的
// payload.type，過濾條件自然排除。

use serde::Deserialize;

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: Option<String>,
    // 實測過 payload 有非物件的變體行：整行解析失敗就當不是 prompt，寬容跳過
    payload: Option<Payload>,
}

#[derive(Deserialize)]
struct Payload {
    #[serde(rename = "type")]
    kind: Option<String>,
    role: Option<String>,
    content: Option<Vec<Block>>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<String>,
}

/// 機器塞進 user role 的文字。實測 0.148.0 只出現 environment_context
/// （permissions、skills、collaboration_mode 都是 developer role，
/// role 過濾就排除了），其餘幾個防禦性列入，防版本變動。
const INJECTED_MARKERS: &[&str] = &[
    "<environment_context>",
    "<permissions instructions>",
    "<collaboration_mode>",
    "<skills_instructions>",
    "<user_instructions>",
    // $skill 呼叫時整份 SKILL.md 注入成獨立 user message，包在 <skill>…</skill>
    "<skill>",
    // 推薦 plugin 清單也存成 user message（實測 2026-08 版本）
    "<recommended_plugins>",
    // 0.150.x 起 AGENTS.md 全文注入成 user message，開頭是這個標題
    // （舊版走 <user_instructions> 包裝，上面那個標記留著擋舊版）
    "# AGENTS.md instructions",
];

/// 認領並抽取一行 Codex rollout。不是 Codex 的 user prompt 就回 None。
pub(super) fn extract(raw: &str) -> Option<String> {
    let line: Line = serde_json::from_str(raw).ok()?;
    if line.kind.as_deref() != Some("response_item") {
        return None;
    }
    let payload = line.payload?;
    if payload.kind.as_deref() != Some("message") || payload.role.as_deref() != Some("user") {
        return None;
    }

    let mut buf = String::new();
    for b in payload.content? {
        if b.kind.as_deref() == Some("input_text") {
            if let Some(t) = b.text {
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(&t);
            }
        }
    }
    if INJECTED_MARKERS.iter().any(|m| buf.contains(m)) {
        return None;
    }
    // $skill 呼叫（$begin、$skill-name args…）是指令不是英文句子，整行跳過。
    // 限定「$ 後緊接字母」，才不會誤殺 "$100 is too much" 這種真的句子。
    let trimmed = buf.trim_start();
    if trimmed.starts_with('$')
        && trimmed[1..].chars().next().is_some_and(|c| c.is_ascii_alphabetic())
    {
        return None;
    }
    Some(buf)
}

#[cfg(test)]
mod tests {
    // 測的是完整管線（含共用的過濾），fixtures 手寫最小行
    use crate::transcript::extract_user_text;

    #[test]
    fn user_message_extracted() {
        let raw = r#"{"timestamp":"t","ordinal":6,"type":"response_item","payload":{"type":"message","id":"m1","role":"user","content":[{"type":"input_text","text":"fix the bug please"}]}}"#;
        assert_eq!(extract_user_text(raw).as_deref(), Some("fix the bug please"));
    }

    #[test]
    fn multiline_input_kept_whole() {
        let raw = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"line one\nalso line two"}]}}"#;
        assert_eq!(extract_user_text(raw).as_deref(), Some("line one\nalso line two"));
    }

    #[test]
    fn environment_context_skipped() {
        let raw = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>\n  <cwd>/x</cwd>\n</environment_context>"}]}}"#;
        assert_eq!(extract_user_text(raw), None);
    }

    #[test]
    fn skill_injection_skipped() {
        // $skill 呼叫時整份 SKILL.md 注入成獨立 user message，
        // 包在 <skill>…</skill>，不擋就整份被送去講評
        let raw = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<skill>\n<name>begin</name>\n<path>/x/SKILL.md</path>\n---\nname: begin\n---\nDo the workflow steps in order.\n</skill>"}]}}"#;
        assert_eq!(extract_user_text(raw), None);
    }

    #[test]
    fn agents_md_injection_skipped() {
        // 0.150.x 的 AGENTS.md 注入（實測形狀）：標題行 + <INSTRUCTIONS> 包全文
        let raw = r##"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions for /x/proj\n\n<INSTRUCTIONS>\nReply with one short sentence.\n</INSTRUCTIONS>"}]}}"##;
        assert_eq!(extract_user_text(raw), None);
    }

    #[test]
    fn dollar_skill_invocation_skipped() {
        let bare = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"$begin"}]}}"#;
        let with_args = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"$skill-name some args"}]}}"#;
        assert_eq!(extract_user_text(bare), None);
        assert_eq!(extract_user_text(with_args), None);
    }

    #[test]
    fn dollar_amount_sentence_kept() {
        // 開頭是金額不是 skill 呼叫（$ 後接數字），是真的英文句要講評
        let raw = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"$100 is too much for this."}]}}"#;
        assert_eq!(
            extract_user_text(raw).as_deref(),
            Some("$100 is too much for this.")
        );
    }

    #[test]
    fn assistant_message_skipped() {
        let raw = r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"model reply"}]}}"#;
        assert_eq!(extract_user_text(raw), None);
    }

    #[test]
    fn non_message_payload_types_skipped() {
        let meta = r#"{"type":"session_meta","payload":{"id":"x","cwd":"/y"}}"#;
        let event = r#"{"type":"event_msg","payload":{"type":"task_started"}}"#;
        let reasoning = r#"{"type":"response_item","payload":{"type":"reasoning","content":[]}}"#;
        assert_eq!(extract_user_text(meta), None);
        assert_eq!(extract_user_text(event), None);
        assert_eq!(extract_user_text(reasoning), None);
    }

    #[test]
    fn non_object_payload_tolerated() {
        // 實測遇過 payload 不是物件的行，要寬容跳過而不是 panic
        let raw = r#"{"type":"response_item","payload":[1,2,3]}"#;
        assert_eq!(extract_user_text(raw), None);
    }
}
