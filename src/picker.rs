// picker.rs
//
// 互動式選單（純 UI 層）：清單怎麼列、分頁與按鍵怎麼走。
// 目錄佈局與掃描邏輯在 sources.rs；這裡只把 Project／SessionFile 排成選單。
//
// 兩層選單：先選專案（偵測到多家 agent 時有分頁列，Tab 切換）、再選 session
// （或「等待下一個新 Session」，session 的 jsonl 要等第一句才建檔，
// 先附著既有檔案永遠會漏掉第一句，這個模式反過來等新檔出現從頭讀）。

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::paths::tildify;
use crate::select::{select, Outcome};
use crate::sources::{self, Kind, Project, Scope};

/// 選單的結果：要監看的檔案、是否從頭讀（等到的新檔一定從頭，否則漏第一句）、
/// 以及之後 follow 模式的掃描範圍。
pub struct Picked {
    pub path: PathBuf,
    pub from_start: bool,
    pub scope: Scope,
}

/// 互動式選單的進入點：選專案 → 選 session（或等新 session）。
/// 在 session 那層按 ← 或 Esc 會退回專案那層重選；在專案那層按才是離開。
pub fn pick() -> Result<Picked> {
    let mut projects = sources::discover()?;
    if projects.is_empty() {
        anyhow::bail!("找不到任何 session：~/.claude/projects 和 ~/.codex/sessions 都是空的");
    }
    // 最近有動靜的排前面，直接按 Enter 就是最新的專案
    projects.sort_by_key(|p| std::cmp::Reverse(p.latest));

    // 分頁 = 偵測到的 agent 種類；預設停在最近有活動的那家
    let kinds: Vec<Kind> = {
        let mut ks = Vec::new();
        for p in &projects {
            if !ks.contains(&p.kind) {
                ks.push(p.kind);
            }
        }
        ks
    };
    let tab_labels: Vec<String> = kinds.iter().map(|k| k.label().to_string()).collect();
    let mut tab = 0; // projects 已按最近排序，第一個的 kind 排在 kinds[0]

    loop {
        let visible: Vec<usize> = projects
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind == kinds[tab])
            .map(|(i, _)| i)
            .collect();
        let entries: Vec<(String, usize)> =
            visible.iter().map(|&i| (project_label(&projects[i]), i)).collect();
        let tabs = (kinds.len() > 1).then_some((tab_labels.as_slice(), tab));
        let help = if kinds.len() > 1 {
            "↑↓ 移動，Tab 換工具，Enter/→ 確認，打字過濾，Esc 離開"
        } else {
            "↑↓ 移動，Enter/→ 確認，打字過濾，Esc 離開"
        };

        let picked = match select("選擇專案：", help, entries, false, tabs)? {
            Outcome::Chosen(i) => i,
            Outcome::NextTab => {
                tab = (tab + 1) % kinds.len();
                continue;
            }
            Outcome::Back => anyhow::bail!("已取消"),
        };

        match select_session(&projects[picked])? {
            Some(picked) => return Ok(picked),
            None => continue, // ←/Esc：回上一層重選專案
        }
    }
}

fn project_label(p: &Project) -> String {
    let name = match &p.cwd {
        Some(c) => tildify(c),
        None => p.fallback_label.clone(),
    };
    format!("{}（{} 個 session，最近 {}）", name, p.n_sessions, ago(p.latest))
}

/// 列出一個專案的 session 讓使用者選。回傳 None 代表要回上一層。
fn select_session(project: &Project) -> Result<Option<Picked>> {
    let mut sessions = project.scope.sessions()?;
    sessions.sort_by_key(|s| std::cmp::Reverse(s.mtime));

    // 第一個選項固定是「等新 session」：這是唯一能收到第一句 prompt 的方式
    let mut entries: Vec<(String, Option<PathBuf>)> = vec![(
        format!(
            "⏳ 等待下一個新 Session (先選這個、再開 {}，才不會漏掉第一句話)",
            project.kind.label()
        ),
        None,
    )];
    entries.extend(sessions.into_iter().map(|s| {
        let snippet = sources::first_prompt_snippet(project.kind, &s.path)
            .map(|t| truncate_chars(&t, 48))
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
        None,
    )? {
        Outcome::Back | Outcome::NextTab => Ok(None),
        Outcome::Chosen(Some(path)) => Ok(Some(Picked {
            path,
            from_start: false,
            scope: project.scope.clone(),
        })),
        Outcome::Chosen(None) => {
            let path = wait_new_session(project)?;
            Ok(Some(Picked { path, from_start: true, scope: project.scope.clone() }))
        }
    }
}

/// 每 300ms 掃一次專案範圍，出現「原本沒有的 jsonl」就回傳它。
/// 用輪詢不用 notify：一次性的等待，輪詢最簡單也最可靠。
fn wait_new_session(project: &Project) -> Result<PathBuf> {
    let mut known: BTreeSet<PathBuf> =
        project.scope.sessions()?.into_iter().map(|s| s.path).collect();
    let place = project.cwd.as_deref().map(tildify).unwrap_or_else(|| project.fallback_label.clone());
    println!(
        "{} {}",
        "等待新 session…".yellow().bold(),
        format!("現在到 {} 開新的 {}（Ctrl-C 取消）", place, project.kind.label()).yellow()
    );
    loop {
        std::thread::sleep(Duration::from_millis(300));
        if let Some(path) = project.scope.newest_unknown(&mut known)? {
            println!(
                "{} {}",
                "接上新 session：".green().bold(),
                crate::paths::display(&path).dimmed()
            );
            return Ok(path);
        }
    }
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

/// 檔名的 8 碼識別，夠用來對照又不佔版面。
/// Claude 的檔名就是 uuid，取開頭；Codex 全都是 rollout- 開頭，
/// 取開頭會人人相同，改取尾巴（uuid 的結尾，一樣唯一）。
fn short_id(path: &std::path::Path) -> String {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    if stem.starts_with("rollout-") {
        let chars: Vec<char> = stem.chars().collect();
        chars[chars.len().saturating_sub(8)..].iter().collect()
    } else {
        stem.chars().take(8).collect()
    }
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
