// sources.rs
//
// 「session 檔從哪來」的抽象層。各家 coding agent 的目錄佈局知識關在這裡，
// 選單（picker）只管 UI，監看端（main）的 follow 模式只透過 Scope 掃描：
//   Claude Code：~/.claude/projects/<專案路徑編碼>/<uuid>.jsonl，一資料夾一專案
//   Codex：~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl，按日期分、cwd 在第一行
// 之後要支援新的 agent：加一個 Kind、一個 discover_*、Scope 加一個 variant。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::paths;

/// Codex 的專案發現往回掃這麼多天的日期資料夾
const CODEX_SCAN_DAYS: i64 = 30;

/// 掃 jsonl 開頭時每檔最多讀的位元組數：單行可以到幾百 KB，
/// 不設上限會讓選單開很慢。Codex 的 session_meta（內嵌整份 system prompt）
/// 特別肥，第一句 prompt 常在 30-40KB 之後，所以上限放寬一倍。
const CLAUDE_HEAD_BYTES: u64 = 64 * 1024;
const CODEX_HEAD_BYTES: u64 = 128 * 1024;

/// 哪一家 coding agent（選單分頁的單位）
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Claude,
    Codex,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Claude => "Claude Code",
            Kind::Codex => "Codex",
        }
    }
}

/// 一個 session jsonl 檔
pub struct SessionFile {
    pub path: PathBuf,
    pub mtime: SystemTime,
}

/// 選單上的一個「專案」：同一個工作目錄底下的一群 session
pub struct Project {
    pub kind: Kind,
    /// 真實工作目錄（顯示用；Codex 同時拿它篩選檔案）
    pub cwd: Option<String>,
    /// cwd 讀不到時顯示的退路（Claude 用資料夾名）
    pub fallback_label: String,
    pub latest: SystemTime,
    pub n_sessions: usize,
    /// 選定之後，session 清單、等待新檔、follow 都用這個範圍掃
    pub scope: Scope,
}

/// 監看範圍：定義「哪些 session 檔屬於這裡」
#[derive(Clone)]
pub enum Scope {
    /// Claude Code 的專案資料夾：底下所有 *.jsonl
    ClaudeDir { dir: PathBuf },
    /// Codex：sessions 根目錄＋目標 cwd。新檔要讀第一行確認 cwd 相符才算數
    CodexCwd { root: PathBuf, cwd: String },
}

impl Scope {
    /// 這個範圍目前全部的 session 檔（選單的 session 清單用）
    pub fn sessions(&self) -> Result<Vec<SessionFile>> {
        match self {
            Scope::ClaudeDir { dir } => list_jsonl(dir),
            Scope::CodexCwd { root, cwd } => {
                let mut out = Vec::new();
                for day in 0..CODEX_SCAN_DAYS {
                    for f in list_jsonl(&codex_day_dir(root, day)).unwrap_or_default() {
                        if let FirstLine::Cwd(Some(c)) = codex_first_line_cwd(&f.path) {
                            if c == *cwd {
                                out.push(f);
                            }
                        }
                    }
                }
                Ok(out)
            }
        }
    }

    /// 「啟動後才出現」的新 session；看過的都記進 known，同檔不回報兩次。
    /// follow 模式和等待新 session 都靠它，每個輪詢 tick 呼叫一次。
    pub fn newest_unknown(&self, known: &mut BTreeSet<PathBuf>) -> Result<Option<PathBuf>> {
        match self {
            Scope::ClaudeDir { dir } => {
                let mut fresh: Vec<SessionFile> = list_jsonl(dir)?
                    .into_iter()
                    .filter(|s| !known.contains(&s.path))
                    .collect();
                if fresh.is_empty() {
                    return Ok(None);
                }
                fresh.sort_by_key(|s| s.mtime);
                for s in &fresh {
                    known.insert(s.path.clone());
                }
                Ok(fresh.pop().map(|s| s.path))
            }
            Scope::CodexCwd { root, cwd } => {
                // 新檔只會出現在今天的日期資料夾；多掃昨天一天是為了
                // 跨午夜的競態（23:59 建檔、00:00 才輪詢到）。
                // 「今天」每次重算，跨午夜自動換資料夾。
                for day in 0..2 {
                    let mut fresh: Vec<SessionFile> = list_jsonl(&codex_day_dir(root, day))
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|s| !known.contains(&s.path))
                        .collect();
                    fresh.sort_by_key(|s| std::cmp::Reverse(s.mtime));
                    for s in fresh {
                        match codex_first_line_cwd(&s.path) {
                            // 第一行還沒寫完：先不記 known，下個 tick 再看
                            FirstLine::Missing => continue,
                            FirstLine::Cwd(Some(c)) if c == *cwd => {
                                known.insert(s.path.clone());
                                return Ok(Some(s.path));
                            }
                            // 別的專案的 session：記住並永遠忽略
                            FirstLine::Cwd(_) => {
                                known.insert(s.path.clone());
                            }
                        }
                    }
                }
                Ok(None)
            }
        }
    }

    /// 使用者直接指定 jsonl 路徑時推斷監看範圍：Codex 的 rollout 走 cwd 篩選
    /// （才能跨日期資料夾 follow），認不出來（含第一行讀不到 cwd）一律退回
    /// 「同資料夾」語意，跟過去的行為一致。
    pub fn for_watched_file(path: &Path) -> Scope {
        if let Some(root) = codex_root_of(path) {
            if let FirstLine::Cwd(Some(cwd)) = codex_first_line_cwd(path) {
                return Scope::CodexCwd { root, cwd };
            }
        }
        let dir = path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
        Scope::ClaudeDir { dir }
    }

    /// notify 要掛在哪裡（事件只是提示，真正保證的是輪詢）
    pub fn watch_target(&self) -> (&Path, bool) {
        match self {
            Scope::ClaudeDir { dir } => (dir, false),
            // 日期資料夾天天換，直接遞迴監看整個 sessions 根目錄
            Scope::CodexCwd { root, .. } => (root, true),
        }
    }
}

/// 兩家一起找出所有專案（哪家不存在就自然缺席）
pub fn discover() -> Result<Vec<Project>> {
    let mut out = Vec::new();
    if let Some(root) = claude_root() {
        if root.is_dir() {
            out.extend(discover_claude(&root)?);
        }
    }
    if let Some(root) = codex_root() {
        if root.is_dir() {
            out.extend(discover_codex(&root)?);
        }
    }
    Ok(out)
}

fn claude_root() -> Option<PathBuf> {
    paths::home().map(|h| PathBuf::from(h).join(".claude").join("projects"))
}

fn codex_root() -> Option<PathBuf> {
    paths::home().map(|h| PathBuf::from(h).join(".codex").join("sessions"))
}

/// Claude Code：一個子資料夾就是一個專案
fn discover_claude(root: &Path) -> Result<Vec<Project>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(root)
        .with_context(|| format!("讀不到 {}", root.display()))?
        .flatten()
    {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let mut sessions = list_jsonl(&dir)?;
        if sessions.is_empty() {
            continue;
        }
        sessions.sort_by_key(|s| std::cmp::Reverse(s.mtime));
        out.push(Project {
            kind: Kind::Claude,
            cwd: claude_read_cwd(&sessions[0].path),
            fallback_label: dir.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            latest: sessions[0].mtime,
            n_sessions: sessions.len(),
            scope: Scope::ClaudeDir { dir },
        });
    }
    Ok(out)
}

/// Codex：掃最近 CODEX_SCAN_DAYS 天的日期資料夾，讀每檔第一行的 cwd 分組。
/// 只讀第一行（session_meta 固定在那），肥大的內容不會拖慢掃描。
/// cwd 讀不到的檔案（極少數，剛建檔或格式變動）直接略過。
fn discover_codex(root: &Path) -> Result<Vec<Project>> {
    let mut groups: BTreeMap<String, Vec<SessionFile>> = BTreeMap::new();
    for day in 0..CODEX_SCAN_DAYS {
        for f in list_jsonl(&codex_day_dir(root, day)).unwrap_or_default() {
            if let FirstLine::Cwd(Some(cwd)) = codex_first_line_cwd(&f.path) {
                groups.entry(cwd).or_default().push(f);
            }
        }
    }
    Ok(groups
        .into_iter()
        .map(|(cwd, sessions)| {
            let latest = sessions.iter().map(|s| s.mtime).max().unwrap_or(SystemTime::UNIX_EPOCH);
            Project {
                kind: Kind::Codex,
                fallback_label: cwd.clone(),
                cwd: Some(cwd.clone()),
                latest,
                n_sessions: sessions.len(),
                scope: Scope::CodexCwd { root: root.to_path_buf(), cwd },
            }
        })
        .collect())
}

/// 列出一個資料夾裡全部的 jsonl 和各自的 mtime（資料夾不存在＝空清單由呼叫端決定）
pub fn list_jsonl(dir: &Path) -> Result<Vec<SessionFile>> {
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
        out.push(SessionFile {
            path,
            mtime: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }
    Ok(out)
}

/// root/YYYY/MM/DD，days_ago=0 是今天。每次呼叫重算，跨午夜自動生效。
fn codex_day_dir(root: &Path, days_ago: i64) -> PathBuf {
    let day = chrono::Local::now() - chrono::Duration::days(days_ago);
    root.join(day.format("%Y/%m/%d").to_string())
}

/// path 長得像 …/sessions/YYYY/MM/DD/x.jsonl 就回 sessions 根目錄
fn codex_root_of(path: &Path) -> Option<PathBuf> {
    let dd = path.parent()?;
    let mm = dd.parent()?;
    let yy = mm.parent()?;
    let numeric = |p: &Path| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    };
    if numeric(dd) && numeric(mm) && numeric(yy) {
        yy.parent().map(|r| r.to_path_buf())
    } else {
        None
    }
}

/// Codex rollout 第一行的讀取結果
enum FirstLine {
    /// 還沒有完整的第一行（檔案剛建立、寫到一半）
    Missing,
    /// 有完整第一行；cwd 可能讀得到也可能沒有
    Cwd(Option<String>),
}

#[derive(Deserialize)]
struct CodexMetaLine {
    payload: Option<CodexMetaPayload>,
}

#[derive(Deserialize)]
struct CodexMetaPayload {
    cwd: Option<String>,
}

/// 讀 rollout 第一行的 session_meta 拿 cwd。只讀開頭 64KB。
fn codex_first_line_cwd(path: &Path) -> FirstLine {
    let Ok(file) = fs::File::open(path) else {
        return FirstLine::Missing;
    };
    let mut buf = Vec::new();
    let _ = file.take(CLAUDE_HEAD_BYTES).read_to_end(&mut buf);
    let Some(nl) = buf.iter().position(|&b| b == b'\n') else {
        return FirstLine::Missing;
    };
    let line = String::from_utf8_lossy(&buf[..nl]);
    let cwd = serde_json::from_str::<CodexMetaLine>(&line)
        .ok()
        .and_then(|l| l.payload)
        .and_then(|p| p.cwd);
    FirstLine::Cwd(cwd)
}

/// jsonl 每行事件幾乎都帶 cwd 欄位；掃前幾行拿到就收工（Claude Code 用）
#[derive(Deserialize)]
struct ClaudeCwdLine {
    cwd: Option<String>,
}

fn claude_read_cwd(path: &Path) -> Option<String> {
    for line in read_head(path, CLAUDE_HEAD_BYTES)?.take(10) {
        if let Ok(l) = serde_json::from_str::<ClaudeCwdLine>(&line) {
            if let Some(c) = l.cwd {
                return Some(c);
            }
        }
    }
    None
}

/// 開檔讀前 cap 位元組，逐行回傳（讀壞的行直接停）
fn read_head(path: &Path, cap: u64) -> Option<impl Iterator<Item = String>> {
    let file = fs::File::open(path).ok()?;
    Some(BufReader::new(file.take(cap)).lines().map_while(Result::ok))
}

/// 抓 session 裡第一句「真的是使用者手打」的 prompt 當預覽（未截斷、已併成一行）。
/// 先用各家的字串特徵挑出可能的行，其他行連 parse 都不用。
pub fn first_prompt_snippet(kind: Kind, path: &Path) -> Option<String> {
    let (cap, hints): (u64, &[&str]) = match kind {
        Kind::Claude => (CLAUDE_HEAD_BYTES, &[r#""type":"user""#, r#""type":"queue-operation""#]),
        Kind::Codex => (CODEX_HEAD_BYTES, &[r#""input_text""#]),
    };
    for line in read_head(path, cap)?.take(120) {
        if !hints.iter().any(|h| line.contains(h)) {
            continue;
        }
        if let Some(text) = crate::transcript::extract_user_text(&line) {
            return Some(text.split_whitespace().collect::<Vec<_>>().join(" "));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gw-sources-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_rollout(dir: &Path, name: &str, cwd: &str, pad: usize) {
        fs::create_dir_all(dir).unwrap();
        let mut f = fs::File::create(dir.join(name)).unwrap();
        // 模擬肥大的 session_meta：cwd 之外塞一大段 base_instructions
        writeln!(
            f,
            r#"{{"timestamp":"t","ordinal":0,"type":"session_meta","payload":{{"cwd":"{}","base_instructions":{{"text":"{}"}}}}}}"#,
            cwd,
            "x".repeat(pad)
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"hello from {}"}}]}}}}"#,
            cwd
        )
        .unwrap();
    }

    #[test]
    fn codex_projects_grouped_by_cwd() {
        let root = tmpdir("group");
        let today = codex_day_dir(&root, 0);
        let yesterday = codex_day_dir(&root, 1);
        write_rollout(&today, "rollout-a.jsonl", "/proj/alpha", 100);
        write_rollout(&today, "rollout-b.jsonl", "/proj/beta", 100);
        write_rollout(&yesterday, "rollout-c.jsonl", "/proj/alpha", 100);

        let mut projects = discover_codex(&root).unwrap();
        projects.sort_by(|a, b| a.cwd.cmp(&b.cwd));
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].cwd.as_deref(), Some("/proj/alpha"));
        assert_eq!(projects[0].n_sessions, 2);
        assert_eq!(projects[1].cwd.as_deref(), Some("/proj/beta"));
        assert_eq!(projects[1].n_sessions, 1);
    }

    #[test]
    fn codex_scope_sessions_filter_by_cwd() {
        let root = tmpdir("scope");
        let today = codex_day_dir(&root, 0);
        write_rollout(&today, "rollout-a.jsonl", "/proj/alpha", 100);
        write_rollout(&today, "rollout-b.jsonl", "/proj/beta", 100);

        let scope = Scope::CodexCwd { root: root.clone(), cwd: "/proj/alpha".into() };
        let sessions = scope.sessions().unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn codex_newest_unknown_respects_cwd_and_incomplete_lines() {
        let root = tmpdir("fresh");
        let today = codex_day_dir(&root, 0);
        fs::create_dir_all(&today).unwrap();
        let scope = Scope::CodexCwd { root: root.clone(), cwd: "/proj/alpha".into() };
        let mut known = BTreeSet::new();

        // 半行（沒換行）：不算新 session、也不能記進 known
        fs::write(today.join("rollout-partial.jsonl"), r#"{"type":"session_me"#).unwrap();
        assert!(scope.newest_unknown(&mut known).unwrap().is_none());
        assert!(known.is_empty());

        // 別的 cwd：忽略並記住
        write_rollout(&today, "rollout-other.jsonl", "/proj/beta", 10);
        assert!(scope.newest_unknown(&mut known).unwrap().is_none());
        assert_eq!(known.len(), 1);

        // 目標 cwd：回報
        write_rollout(&today, "rollout-mine.jsonl", "/proj/alpha", 10);
        let found = scope.newest_unknown(&mut known).unwrap();
        assert!(found.is_some_and(|p| p.file_name().unwrap() == "rollout-mine.jsonl"));
    }

    #[test]
    fn watched_file_scope_inferred_from_path_shape() {
        let root = tmpdir("infer");
        let today = codex_day_dir(&root, 0);
        write_rollout(&today, "rollout-x.jsonl", "/proj/alpha", 10);
        match Scope::for_watched_file(&today.join("rollout-x.jsonl")) {
            Scope::CodexCwd { cwd, .. } => assert_eq!(cwd, "/proj/alpha"),
            _ => panic!("應判定為 Codex 範圍"),
        }
        // 非日期佈局 → 退回同資料夾語意
        let plain = tmpdir("plain");
        fs::write(plain.join("a.jsonl"), "{}\n").unwrap();
        match Scope::for_watched_file(&plain.join("a.jsonl")) {
            Scope::ClaudeDir { dir } => assert_eq!(dir, plain),
            _ => panic!("應退回資料夾範圍"),
        }
    }

    #[test]
    fn codex_discovery_of_1000_fat_rollouts_is_fast() {
        // 效能驗收：30 天 × 34 檔、每檔第一行約 15KB，發現流程要在門檻內
        let root = tmpdir("bench");
        let cwds = ["/p/a", "/p/b", "/p/c", "/p/d", "/p/e"];
        let mut n = 0;
        'outer: for day in 0..CODEX_SCAN_DAYS {
            let dir = codex_day_dir(&root, day);
            for i in 0..34 {
                write_rollout(&dir, &format!("rollout-{day}-{i}.jsonl"), cwds[n % cwds.len()], 15_000);
                n += 1;
                if n >= 1000 {
                    break 'outer;
                }
            }
        }
        let t = std::time::Instant::now();
        let projects = discover_codex(&root).unwrap();
        let elapsed = t.elapsed();
        println!("discover_codex：{n} 檔 → {} 專案，耗時 {elapsed:?}", projects.len());
        assert_eq!(projects.len(), cwds.len());
        // 驗收門檻 200ms 的十倍當作 CI 容忍（debug 模式較慢、機器差異大）
        assert!(elapsed < std::time::Duration::from_secs(2), "掃描太慢：{elapsed:?}");
    }
}
