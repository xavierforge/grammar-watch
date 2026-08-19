// select.rs
//
// 極簡單選選單，用 crossterm 直接畫。不用現成的選單套件，因為需要完全掌控按鍵
// （←/→ 當導覽鍵）、選項行距和捲動行為，這些一般套件都不給改。按鍵：
//   ↑/↓ 移動、Enter/→ 確認、Esc 返回、打字過濾、Backspace 修改過濾、Ctrl-C 離開。
//   ← 也是返回，但只在 back_enabled 的選單生效：最上層按 ← 不該直接把程式關掉。

use std::io::{self, Write};

use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType},
};
use owo_colors::OwoColorize;
use unicode_width::UnicodeWidthChar;

/// 一次最多顯示幾個選項（每個選項佔兩行：內容＋空行）
const PAGE: usize = 8;

pub enum Outcome<T> {
    Chosen(T),
    Back,
}

/// RAII：不管怎麼離開（含錯誤提早 return），都要把 raw mode 關掉、游標顯示回來，
/// 不然使用者的終端機就壞了。
struct RawGuard;

impl RawGuard {
    fn new() -> Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), cursor::Hide)?;
        Ok(RawGuard)
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), cursor::Show);
        let _ = terminal::disable_raw_mode();
    }
}

/// 選單的完整狀態：過濾、游標、捲動視窗、上次畫了幾行。
/// 重繪和按鍵處理都對著它做，不用在函式之間搬一長串參數。
struct Menu<'a, T> {
    title: &'a str,
    help: &'a str,
    entries: Vec<(String, T)>,
    filter: String,
    /// 游標在 filtered 裡的位置
    cur: usize,
    /// 捲動視窗起點（filtered 的 index）
    win: usize,
    /// 上一次畫了幾行，重繪時要先回去清掉
    drawn: usize,
}

impl<T> Menu<'_, T> {
    /// 通過目前過濾字串的項目（回傳 entries 的 index 清單）
    fn filtered(&self) -> Vec<usize> {
        let needle = self.filter.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, (label, _))| needle.is_empty() || label.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect()
    }

    /// 過濾結果變短或游標移動後，把游標和視窗拉回合法範圍
    fn clamp(&mut self, filtered_len: usize) {
        if filtered_len > 0 && self.cur >= filtered_len {
            self.cur = filtered_len - 1;
        }
        if self.cur < self.win {
            self.win = self.cur;
        }
        if self.cur >= self.win + PAGE {
            self.win = self.cur + 1 - PAGE;
        }
    }

    /// 把整個選單畫出來：先回到上次畫的起點、清掉、再重印。
    /// raw mode 下換行要自己送 \r\n。
    fn draw(&mut self, out: &mut impl Write, filtered: &[usize]) -> Result<()> {
        // size() 在某些假終端（pty 沒設大小）會回 0，寬度太小就當 80 用，
        // 不然所有字都會被截成「…」
        let width = match terminal::size() {
            Ok((w, _)) if w >= 20 => w as usize,
            _ => 80,
        };

        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("{} {}{}", "?".green().bold(), self.title.bold(), self.filter));
        lines.push(String::new());

        if filtered.is_empty() {
            lines.push("  （沒有符合的項目）".dimmed().to_string());
            lines.push(String::new());
        }
        let end = (self.win + PAGE).min(filtered.len());
        for (row, &idx) in filtered[self.win..end].iter().enumerate() {
            let pos = self.win + row;
            let selected = pos == self.cur;
            // 前綴：選到的畫 ❯；視窗外還有東西時，最上/最下列改畫捲動提示
            let prefix = if selected {
                "❯"
            } else if row == 0 && self.win > 0 {
                "↑"
            } else if pos == end - 1 && end < filtered.len() {
                "↓"
            } else {
                " "
            };
            let body = fit(&self.entries[idx].0, width.saturating_sub(3));
            let line = format!("{prefix} {body}");
            lines.push(if selected { line.cyan().bold().to_string() } else { line });
            lines.push(String::new());
        }
        lines.push(format!("[{}]", self.help).dimmed().to_string());

        if self.drawn > 0 {
            queue!(
                out,
                cursor::MoveToColumn(0),
                cursor::MoveUp(self.drawn as u16),
                Clear(ClearType::FromCursorDown)
            )?;
        } else {
            queue!(out, cursor::MoveToColumn(0), Clear(ClearType::FromCursorDown))?;
        }
        for l in &lines {
            queue!(out, Print(l), Print("\r\n"))?;
        }
        out.flush()?;
        self.drawn = lines.len();
        Ok(())
    }
}

/// 顯示一個單選選單，回傳選中的值；Esc（以及 back_enabled 時的 ←）回傳 Back，
/// 讓呼叫端決定退到哪層。entries 是 (顯示文字, 值)。
pub fn select<T>(
    title: &str,
    help: &str,
    entries: Vec<(String, T)>,
    back_enabled: bool,
) -> Result<Outcome<T>> {
    let mut out = io::stdout();
    let guard = RawGuard::new()?;
    let mut menu = Menu {
        title,
        help,
        entries,
        filter: String::new(),
        cur: 0,
        win: 0,
        drawn: 0,
    };

    loop {
        let filtered = menu.filtered();
        menu.clamp(filtered.len());
        menu.draw(&mut out, &filtered)?;

        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match (key.code, key.modifiers) {
            (KeyCode::Up, _) => menu.cur = menu.cur.saturating_sub(1),
            (KeyCode::Down, _) => {
                if menu.cur + 1 < filtered.len() {
                    menu.cur += 1;
                }
            }
            (KeyCode::Enter | KeyCode::Right, _) => {
                if let Some(&i) = filtered.get(menu.cur) {
                    clear_frame(&mut out, menu.drawn)?;
                    drop(guard);
                    let (label, value) = menu.entries.swap_remove(i);
                    println!("{} {}{}", "✔".green().bold(), menu.title.bold(), label.cyan());
                    return Ok(Outcome::Chosen(value));
                }
            }
            (KeyCode::Esc, _) => {
                clear_frame(&mut out, menu.drawn)?;
                drop(guard);
                return Ok(Outcome::Back);
            }
            // ← 只在有「上一層」可退的選單當返回鍵；最上層按到就當沒事
            (KeyCode::Left, _) if back_enabled => {
                clear_frame(&mut out, menu.drawn)?;
                drop(guard);
                return Ok(Outcome::Back);
            }
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                clear_frame(&mut out, menu.drawn)?;
                drop(guard);
                anyhow::bail!("已取消");
            }
            (KeyCode::Backspace, _) => {
                menu.filter.pop();
                (menu.cur, menu.win) = (0, 0);
            }
            (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => {
                menu.filter.push(c);
                (menu.cur, menu.win) = (0, 0);
            }
            _ => {}
        }
    }
}

/// 把上一次畫的選單整塊清掉：回到框的起點、往下全清
fn clear_frame(out: &mut impl Write, drawn: usize) -> Result<()> {
    if drawn > 0 {
        execute!(
            out,
            cursor::MoveToColumn(0),
            cursor::MoveUp(drawn as u16),
            Clear(ClearType::FromCursorDown)
        )?;
    }
    Ok(())
}

/// 依「顯示寬度」截斷（CJK 一字兩格），太長補 …。
/// 選單每列都不能折行，不然重繪時行數對不上、畫面會疊影。
fn fit(s: &str, max: usize) -> String {
    let total: usize = s.chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum();
    if total <= max {
        return s.to_string();
    }
    let mut w = 0;
    let mut out = String::new();
    for c in s.chars() {
        let cw = UnicodeWidthChar::width(c).unwrap_or(0);
        if w + cw > max.saturating_sub(1) {
            break;
        }
        w += cw;
        out.push(c);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu(labels: &[&str]) -> Menu<'static, usize> {
        Menu {
            title: "t",
            help: "h",
            entries: labels.iter().enumerate().map(|(i, l)| (l.to_string(), i)).collect(),
            filter: String::new(),
            cur: 0,
            win: 0,
            drawn: 0,
        }
    }

    #[test]
    fn filter_is_case_insensitive_substring() {
        let mut m = menu(&["Alpha One", "beta two", "gamma"]);
        m.filter = "ONE".into();
        assert_eq!(m.filtered(), vec![0]);
        m.filter = "a".into();
        assert_eq!(m.filtered(), vec![0, 1, 2]);
    }

    #[test]
    fn clamp_pulls_cursor_and_window_back() {
        let mut m = menu(&["a", "b", "c"]);
        m.cur = 10;
        m.clamp(3);
        assert_eq!(m.cur, 2);
        // 游標跑到視窗外，視窗要跟上
        m.cur = 0;
        m.win = 2;
        m.clamp(3);
        assert_eq!(m.win, 0);
    }

    #[test]
    fn fit_truncates_by_display_width() {
        assert_eq!(fit("hello", 10), "hello");
        assert_eq!(fit("hello world", 8), "hello w…");
        // CJK 一字兩格：寬度 7 只放得下三個全形字加刪節號
        assert_eq!(fit("繁體中文字", 7), "繁體中…");
    }
}
