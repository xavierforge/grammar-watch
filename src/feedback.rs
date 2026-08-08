// feedback.rs
//
// 模型回覆的「原句 / 建議 / 講評」三段：parse 負責解析成結構、print 負責上色印出。
// 拆成兩步是為了測試：解析是純函式，也最容易因為模型回覆的格式變化而出錯，
// 分開才有辦法直接餵字串驗證。

use owo_colors::OwoColorize;
use similar::{ChangeTag, TextDiff};

/// 模型講評解析後的結果。任何一段都可能缺（模型沒照格式時），印的時候各自處理。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Feedback {
    pub orig: Option<String>,
    pub sugg: Option<String>,
    pub comments: Vec<String>,
}

/// 解析模型回傳的「原句 / 建議 / 講評」。每一段都可能「跨多行」，
/// 尤其使用者原文本身就有換行時，原句會被模型貼成多行。
/// 所以要把沒有標籤的後續行「累加回目前所在的段落」，
/// 不能只抓標籤那一行，否則原句／建議會只剩第一行。
pub fn parse(text: &str) -> Feedback {
    #[derive(PartialEq)]
    enum Sec {
        None,
        Orig,
        Sugg,
        Review,
    }
    let mut sec = Sec::None;
    let mut orig = String::new();
    let mut sugg = String::new();
    let mut comments: Vec<String> = Vec::new();

    for line in text.trim().lines() {
        let l = line.trim_end();
        if let Some(rest) = strip_label(l, "原句") {
            sec = Sec::Orig;
            append_field(&mut orig, rest);
        } else if let Some(rest) = strip_label(l, "建議") {
            sec = Sec::Sugg;
            append_field(&mut sugg, &strip_bullet(rest));
        } else if let Some(rest) = strip_label(l, "講評") {
            sec = Sec::Review;
            if !rest.trim().is_empty() {
                comments.push(rest.to_string());
            }
        } else if l.is_empty() {
            // 空行略過
        } else {
            match sec {
                Sec::Orig => append_field(&mut orig, l),
                Sec::Sugg => append_field(&mut sugg, &strip_bullet(l)),
                Sec::Review => comments.push(l.to_string()),
                Sec::None => {} // 標籤前的雜訊，略過
            }
        }
    }

    Feedback {
        orig: (!orig.is_empty()).then_some(orig),
        sugg: (!sugg.is_empty()).then_some(sugg),
        comments,
    }
}

/// 把講評上色印出：小修時用「紅底原句＋綠底建議」上下對照，一眼看到改在哪；
/// 大改寫時 diff 會碎成一塊塊落在奇怪位置，退回整行上色比較清楚。
pub fn print(fb: &Feedback) {
    println!("{}", "─".repeat(56).dimmed());

    match (&fb.orig, &fb.sugg) {
        // 小修：紅底原句＋綠底建議上下對照，最清楚。
        (Some(o), Some(s)) if worth_diffing(o, s) => {
            println!("{}{}{}", "原句".cyan().bold(), "：".cyan(), highlight_deletions(o, s));
            println!("{}{}{}", "建議".green().bold(), "：".green(), highlight_insertions(o, s));
        }
        // 大改寫：原句與建議各印整行。
        (Some(o), Some(s)) => {
            println!("{}{}", "原句".cyan().bold(), format!("：{o}").cyan());
            println!("{}{}", "建議".green().bold(), format!("：{s}").green());
        }
        // 少了原句或建議，能印什麼印什麼。
        _ => {
            if let Some(o) = &fb.orig {
                println!("{}{}", "原句".cyan().bold(), format!("：{o}").cyan());
            }
            if let Some(s) = &fb.sugg {
                println!("{}{}", "建議".green().bold(), format!("：{s}").green());
            }
        }
    }

    // 講評可能多行（例如接了一句「另一種說法：…」）；第一行加標籤，其餘照印。
    for (i, line) in fb.comments.iter().enumerate() {
        if i == 0 {
            println!("{}{}", "講評".yellow().bold(), format!("：{line}").yellow());
        } else {
            println!("{}", line.yellow());
        }
    }
    println!();
}

/// 把 "原句：xxx" 這種標籤行的標籤和冒號去掉，只留內容。不是這個標籤就回 None。
fn strip_label<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(label)?;
    // 全形「：」和半形 ":" 都吃，前後空白也去掉
    Some(rest.trim_start_matches(['：', ':', ' ']).trim_end())
}

/// 把 "- xxx" / "1. xxx" / "• xxx" 這種項目符號前綴去掉，只留內容。
fn strip_bullet(line: &str) -> String {
    line.trim_start()
        .trim_start_matches(|c: char| {
            matches!(c, '-' | '*' | '•' | '・' | '–' | '·' | '.' | ')' | ' ') || c.is_ascii_digit()
        })
        .trim_end()
        .to_string()
}

/// 把一段內容接到欄位緩衝區後面。原文若有換行，會被併成同一行（用空白隔開），
/// 這樣「原句 / 建議」永遠是一行、方便逐字對照。
fn append_field(buf: &mut String, s: &str) {
    let s = s.trim();
    if s.is_empty() {
        return;
    }
    if !buf.is_empty() {
        buf.push(' ');
    }
    buf.push_str(s);
}

/// 判斷「原句 → 建議」的改動是否小到適合逐字標記。
/// 太大幅的重寫、或建議塞了多個版本，diff 會碎成一塊塊落在奇怪位置，改用整行上色更清楚。
fn worth_diffing(orig: &str, sugg: &str) -> bool {
    // 建議裡列了多個版本（/ ／ 或）就不 diff
    if sugg.contains(" / ") || sugg.contains('／') || sugg.contains(" 或 ") {
        return false;
    }
    // 以字為單位算相似度，夠像（代表只是小修）才值得逐字標記
    TextDiff::from_words(orig, sugg).ratio() >= 0.6
}

/// 回傳原句字串，把「相對建議句被刪掉/改掉的字」用紅底標出來，其餘字暗掉。
/// from_words 會把空白也切成獨立 token；純空白就算被改動也不要上底色，
/// 否則背景框會多出頭尾的空格、看起來歪掉。只有「真的有字」的 token 才上底色。
fn highlight_deletions(orig: &str, sugg: &str) -> String {
    let mut out = String::new();
    for change in TextDiff::from_words(orig, sugg).iter_all_changes() {
        let v = change.value();
        match change.tag() {
            ChangeTag::Delete if !v.trim().is_empty() => {
                out.push_str(&v.on_red().white().bold().to_string())
            }
            ChangeTag::Delete | ChangeTag::Equal => out.push_str(&v.dimmed().to_string()),
            ChangeTag::Insert => {} // 新增的字只在建議行顯示
        }
    }
    out
}

/// 回傳建議句字串，把「相對原句新增/改成的字」用綠底標出來，其餘字暗掉（純空白不上底色）。
fn highlight_insertions(orig: &str, sugg: &str) -> String {
    let mut out = String::new();
    for change in TextDiff::from_words(orig, sugg).iter_all_changes() {
        let v = change.value();
        match change.tag() {
            ChangeTag::Insert if !v.trim().is_empty() => {
                out.push_str(&v.on_green().black().bold().to_string())
            }
            ChangeTag::Insert | ChangeTag::Equal => out.push_str(&v.dimmed().to_string()),
            ChangeTag::Delete => {} // 被刪掉的字只在原句行顯示
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_sections_parsed() {
        let fb = parse("原句：I has a apple\n建議：I have an apple\n講評：has 要改 have。");
        assert_eq!(fb.orig.as_deref(), Some("I has a apple"));
        assert_eq!(fb.sugg.as_deref(), Some("I have an apple"));
        assert_eq!(fb.comments, vec!["has 要改 have。"]);
    }

    #[test]
    fn halfwidth_colon_accepted() {
        let fb = parse("原句: one\n建議: two\n講評: three");
        assert_eq!(fb.orig.as_deref(), Some("one"));
        assert_eq!(fb.sugg.as_deref(), Some("two"));
    }

    #[test]
    fn multiline_orig_accumulates_into_one_line() {
        // 原文有換行時，模型會把原句貼成多行；後續行要接回原句、以空格相連
        let fb = parse("原句：line one\nline two\n建議：ok\n講評：good");
        assert_eq!(fb.orig.as_deref(), Some("line one line two"));
    }

    #[test]
    fn bullet_stripped_from_suggestion() {
        let fb = parse("原句：x\n建議：- Fixed sentence\n講評：ok");
        assert_eq!(fb.sugg.as_deref(), Some("Fixed sentence"));
    }

    #[test]
    fn alt_phrasing_joins_comments() {
        let fb = parse("原句：x\n建議：y\n講評：第一句。\n另一種說法：Another way.");
        assert_eq!(fb.comments, vec!["第一句。", "另一種說法：Another way."]);
    }

    #[test]
    fn noise_before_labels_ignored() {
        let fb = parse("好的，以下是講評\n\n原句：x\n建議：y\n講評：z");
        assert_eq!(fb.orig.as_deref(), Some("x"));
    }

    #[test]
    fn missing_sections_stay_empty() {
        let fb = parse("完全不照格式的回覆");
        assert_eq!(fb, Feedback::default());
    }
}
