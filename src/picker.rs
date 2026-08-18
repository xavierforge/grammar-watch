// picker.rs
//
// 不帶參數啟動時的互動式選單：先選專案、再選 session，
// 免去「開 session → 打一句話 → 去 ~/.claude/projects 撈亂數檔名」的麻煩。
//
// 另外提供「等下一個新 session」：session 的 jsonl 要等你打出第一句才會出現，
// 先附著既有檔案永遠會漏掉第一句。這個模式反過來先監看專案資料夾，
// 新檔一出現就回傳給 main 從頭讀，第一句就不會漏。

use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Deserialize;

use crate::paths::tildify;
use crate::select::{select, Outcome};

/// 選單的結果。New 是「等到新檔案剛出現」才回傳的，
/// 呼叫端必須從頭讀（offset 0），否則第一句 prompt 就漏了。
pub enum Picked {
    Existing(PathBuf),
    New(PathBuf),
}

/// 互動式選單的進入點：選專案 → 選 session（或等新 session）。
/// 在 session 那層按 ← 或 Esc 會退回專案那層重選；在專案那層按才是離開。
pub fn pick() -> Result<Picked> {
    let root = projects_root()?;
    let mut projects = scan_projects(&root)?;
    if projects.is_empty() {
        anyhow::bail!("在 {} 底下找不到任何 session jsonl", root.display());
    }
    // 最近有動靜的排前面，直接按 Enter 就是最新的專案
    projects.sort_by_key(|p| std::cmp::Reverse(p.latest));

    loop {
        // 用 index 當選項的值，專案本身留在 projects 裡，退回來時才能重列
        let entries: Vec<(String, usize)> = projects
            .iter()
            .enumerate()
            .map(|(i, p)| (p.label(), i))
            .collect();
        let picked = match select(
            "選擇專案：",
            "↑↓ 移動，Enter/→ 確認，打字過濾，Esc 離開",
            entries,
            false, // 最上層，← 沒有上一層可退
        )? {
            Outcome::Chosen(i) => i,
            Outcome::Back => anyhow::bail!("已取消"),
        };

        match select_session(&projects[picked])? {
            Some(picked) => return Ok(picked),
            None => continue, // ←/Esc：回上一層重選專案
        }
    }
}

/// 列出一個專案的 session 讓使用者選。回傳 None 代表要回上一層。
fn select_session(project: &Project) -> Result<Option<Picked>> {
    let mut sessions = sessions_in(&project.dir)?;
    sessions.sort_by_key(|s| std::cmp::Reverse(s.mtime));

    // 第一個選項固定是「等新 session」：這是唯一能收到第一句 prompt 的方式
    let mut entries: Vec<(String, Option<PathBuf>)> = vec![(
        "⏳ 等待下一個新 Session (先選這個、再開 Claude Code，才不會漏掉第一句話)".to_string(),
        None,
    )];
    entries.extend(sessions.into_iter().map(|s| {
        let snippet = first_prompt_snippet(&s.path)
            .unwrap_or_else(|| "（還沒有 prompt）".to_string());
        (
            format!("{} · {} · {}", ago(s.mtime), short_id(&s.path), snippet),
            Some(s.path),
        )
    }));

    match select(
        "選擇 session：",
        "↑↓ 移動，Enter/→ 確認，打字過濾，←/Esc 回上一層",
        entries,
        true,
    )? {
        Outcome::Back => Ok(None),
        Outcome::Chosen(Some(path)) => Ok(Some(Picked::Existing(path))),
        Outcome::Chosen(None) => Ok(Some(Picked::New(wait_new_session(project)?))),
    }
}

// ── 專案掃描 ───────────────────────────────────────────────────

struct Project {
    dir: PathBuf,
    /// 從 jsonl 裡讀出來的真實工作目錄。資料夾名稱是把路徑裡的
    /// 符號全換成 '-' 的編碼，沒辦法可靠還原，所以直接看檔案內容。
    cwd: Option<String>,
    latest: SystemTime,
    n_sessions: usize,
}

impl Project {
    /// 給人看的專案位置：優先用 jsonl 裡的真實 cwd，沒有就退回資料夾名
    fn display_name(&self) -> String {
        match &self.cwd {
            Some(c) => tildify(c),
            None => self.dir.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        }
    }

    fn label(&self) -> String {
        format!(
            "{}（{} 個 session，最近 {}）",
            self.display_name(),
            self.n_sessions,
            ago(self.latest)
        )
    }
}

/// Claude Code 存所有 session 的根目錄：~/.claude/projects
fn projects_root() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("讀不到 HOME 環境變數")?;
    Ok(PathBuf::from(home).join(".claude").join("projects"))
}

/// 掃根目錄底下的專案資料夾，只收「至少有一個 session jsonl」的
fn scan_projects(root: &Path) -> Result<Vec<Project>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(root)
        .with_context(|| format!("讀不到 {}", root.display()))?
        .flatten()
    {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let mut sessions = sessions_in(&dir)?;
        if sessions.is_empty() {
            continue;
        }
        sessions.sort_by_key(|s| std::cmp::Reverse(s.mtime));
        let newest = &sessions[0];
        out.push(Project {
            cwd: read_cwd(&newest.path),
            latest: newest.mtime,
            n_sessions: sessions.len(),
            dir,
        });
    }
    Ok(out)
}

// ── session 掃描 ───────────────────────────────────────────────

struct Session {
    path: PathBuf,
    mtime: SystemTime,
}

/// 列出一個專案資料夾裡全部的 session jsonl 和各自的 mtime
fn sessions_in(dir: &Path) -> Result<Vec<Session>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)
        .with_context(|| format!("讀不到 {}", dir.display()))?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        out.push(Session { path, mtime });
    }
    Ok(out)
}

/// jsonl 每行事件幾乎都帶 cwd 欄位；掃前幾行拿到就收工
#[derive(Deserialize)]
struct CwdLine {
    cwd: Option<String>,
}

fn read_cwd(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(std::io::Read::take(file, 64 * 1024));
    for line in reader.lines().take(10).map_while(Result::ok) {
        if let Ok(l) = serde_json::from_str::<CwdLine>(&line) {
            if let Some(c) = l.cwd {
                return Some(c);
            }
        }
    }
    None
}

/// 抓 session 裡第一句「真的是使用者手打」的 prompt 當預覽，
/// 沿用 main 的 extract_user_text 過濾規則。
/// 只讀每個檔案開頭 64KB、最多 80 行：jsonl 的單行可以到幾百 KB
/// （assistant 回覆、tool 結果都在裡面），全部 serde 掃一遍會讓選單開很慢。
/// 先用字串比對挑出可能是 user 的行，其他行連 parse 都不用。
fn first_prompt_snippet(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(std::io::Read::take(file, 64 * 1024));
    for line in reader.lines().take(80).map_while(Result::ok) {
        if !line.contains(r#""type":"user""#) && !line.contains(r#""type":"queue-operation""#) {
            continue;
        }
        if let Some(text) = crate::extract_user_text(&line) {
            let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
            return Some(truncate_chars(&one_line, 48));
        }
    }
    None
}

/// 以「字元數」截斷字串，太長補 …（選單顯示用，粗略即可）
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// 檔名（session uuid）開頭 8 碼，夠用來對照又不佔版面
fn short_id(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .take(8)
        .collect()
}

/// 把時間點轉成「剛剛 / N 分鐘前」這種相對描述
fn ago(t: SystemTime) -> String {
    let secs = SystemTime::now().duration_since(t).unwrap_or_default().as_secs();
    match secs {
        0..=59 => "剛剛".to_string(),
        60..=3599 => format!("{} 分鐘前", secs / 60),
        3600..=86399 => format!("{} 小時前", secs / 3600),
        _ => format!("{} 天前", secs / 86400),
    }
}

// ── 等新 session ───────────────────────────────────────────────

/// 每 300ms 掃一次專案資料夾，出現「原本沒有的 .jsonl」就回傳它。
/// 用輪詢不用 notify：一次性的等待，輪詢最簡單也最可靠（notify 在某些
/// 平台上會漏 create 事件）。這裡還在啟動階段、沒有其他非同步工作，
/// 直接 thread::sleep 沒關係。
fn wait_new_session(project: &Project) -> Result<PathBuf> {
    let dir = &project.dir;
    let known: BTreeSet<PathBuf> = sessions_in(dir)?.into_iter().map(|s| s.path).collect();
    println!(
        "{} {}",
        "等待新 session…".yellow().bold(),
        format!(
            "現在到 {} 開新的 Claude Code（Ctrl-C 取消）",
            project.display_name()
        )
        .yellow()
    );
    loop {
        std::thread::sleep(Duration::from_millis(300));
        for s in sessions_in(dir)? {
            if !known.contains(&s.path) {
                println!(
                    "{} {}",
                    "接上新 session：".green().bold(),
                    crate::paths::display(&s.path).dimmed()
                );
                return Ok(s.path);
            }
        }
    }
}
