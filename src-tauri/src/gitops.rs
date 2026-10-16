//! 코딩팀의 작업 공간. 사장님 저장소는 그대로 두고, 업무마다 별도 git worktree에서 일한 뒤
//! 사장님이 변경을 보고 병합하거나 버린다.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const IDENTITY: [&str; 4] = ["-c", "user.name=일손", "-c", "user.email=ilson@localhost"];
/// 회사 관리용 파일: 커밋·변경 목록에서 뺀다.
const MANAGED: [&str; 6] = ["--", ".", ":(exclude).ilson", ":(exclude).agents/agents/ilson-*", ":(exclude).claude/agents/ilson-*", ":(exclude).opencode/agents/ilson-*"];
/// 화면에 보여 줄 diff 최대 크기.
const MAX_DIFF: usize = 200_000;

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(IDENTITY).args(args);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let out = cmd.output().map_err(|e| anyhow!("git을 실행하지 못했습니다. Git이 설치되어 있는지 확인해 주세요. ({e})"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("git {} 실패: {}", args.first().unwrap_or(&""), err.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub repo: String,
    pub base_branch: String,
    pub base_commit: String,
    pub branch: String,
    pub path: String,
    pub initialized: bool,
    pub dirty_main: bool,
}

/// 저장소를 확인하고(필요하면 새로 만들고) 업무용 worktree를 만든다.
pub fn prepare(folder: &str, worktrees_root: &Path, short_id: &str, allow_init: bool) -> Result<Workspace> {
    let dir = Path::new(folder);
    if !dir.is_dir() {
        std::fs::create_dir_all(dir).map_err(|e| anyhow!("작업 폴더를 만들 수 없습니다: {e}"))?;
    }
    let mut initialized = false;
    let repo = match git(dir, &["rev-parse", "--show-toplevel"]) {
        Ok(top) => PathBuf::from(top),
        Err(_) if allow_init => {
            git(dir, &["init"])?;
            initialized = true;
            dir.to_path_buf()
        }
        Err(_) => bail!("git 저장소가 아닙니다. ‘git이 없으면 새로 시작’을 켜거나 저장소 폴더를 골라 주세요."),
    };
    if git(&repo, &["rev-parse", "--verify", "HEAD"]).is_err() {
        // 커밋이 하나도 없으면 지금 상태를 첫 기록으로 남긴다.
        git(&repo, &["add", "-A"])?;
        git(&repo, &["commit", "--allow-empty", "-m", "일손: 시작 시점 기록"])?;
        initialized = true;
    }
    let base_branch = git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let base_commit = git(&repo, &["rev-parse", "HEAD"])?;
    let dirty_main = !git(&repo, &["status", "--porcelain"])?.is_empty();
    let branch = format!("ilson/{short_id}");
    let path = worktrees_root.join(short_id);
    std::fs::create_dir_all(worktrees_root)?;
    if !path.exists() {
        git(&repo, &["worktree", "add", "-b", &branch, &path.to_string_lossy(), "HEAD"])?;
    }
    Ok(Workspace {
        repo: repo.to_string_lossy().into(),
        base_branch,
        base_commit,
        branch,
        path: path.to_string_lossy().into(),
        initialized,
        dirty_main,
    })
}

/// 작업 공간의 변경을 커밋한다. 바뀐 것이 없으면 None.
pub fn commit_all(ws: &Workspace, message: &str) -> Result<Option<String>> {
    let wt = Path::new(&ws.path);
    git(wt, &[&["add", "-A"][..], &MANAGED[..]].concat())?;
    if git(wt, &[&["diff", "--cached", "--name-only"][..], &MANAGED[..]].concat())?.is_empty() {
        return Ok(None);
    }
    git(wt, &["commit", "-m", message])?;
    Ok(Some(git(wt, &["rev-parse", "--short", "HEAD"])?))
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiffSummary {
    pub files: Vec<FileChange>,
    pub added: u64,
    pub removed: u64,
    pub patch: String,
    pub truncated: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct FileChange {
    pub path: String,
    pub added: u64,
    pub removed: u64,
}

pub fn diff(ws: &Workspace) -> Result<DiffSummary> {
    let wt = Path::new(&ws.path);
    let range = format!("{}..HEAD", ws.base_commit);
    let mut summary = DiffSummary::default();
    for line in git(wt, &["diff", "--numstat", &range])?.lines() {
        let mut parts = line.splitn(3, '\t');
        let (a, r, p) = (parts.next().unwrap_or("0"), parts.next().unwrap_or("0"), parts.next().unwrap_or(""));
        let (a, r) = (a.parse().unwrap_or(0), r.parse().unwrap_or(0));
        summary.added += a;
        summary.removed += r;
        summary.files.push(FileChange { path: p.to_string(), added: a, removed: r });
    }
    let patch = git(wt, &["diff", "--no-color", &range])?;
    summary.truncated = patch.len() > MAX_DIFF;
    summary.patch = if summary.truncated {
        let mut cut = MAX_DIFF;
        while !patch.is_char_boundary(cut) {
            cut -= 1;
        }
        patch[..cut].to_string()
    } else {
        patch
    };
    Ok(summary)
}

/// 아직 커밋하지 않은 변경 파일 (안전 모드 관찰용).
pub fn changed_files(dir: &Path) -> Result<Vec<String>> {
    Ok(git(dir, &[&["status", "--porcelain", "-uall"][..], &MANAGED[..]].concat())?
        .lines()
        .filter_map(|l| l.get(3..).map(|p| p.trim_matches('"').to_string()))
        .collect())
}

pub fn log(ws: &Workspace) -> Result<Vec<String>> {
    let range = format!("{}..HEAD", ws.base_commit);
    Ok(git(Path::new(&ws.path), &["log", "--format=%h %s", &range])?.lines().map(String::from).collect())
}

/// 사장님 저장소의 원래 브랜치에 작업 브랜치를 병합하고 작업 공간을 정리한다.
pub fn merge(ws: &Workspace, title: &str) -> Result<String> {
    let repo = Path::new(&ws.repo);
    let current = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if current != ws.base_branch {
        bail!(
            "저장소가 지금 ‘{current}’ 브랜치에 있습니다. 시작할 때의 ‘{}’ 브랜치로 바꾼 뒤 다시 병합해 주세요.",
            ws.base_branch
        );
    }
    let msg = format!("일손: {title}");
    if let Err(e) = git(repo, &["merge", "--no-ff", "-m", &msg, &ws.branch]) {
        let _ = git(repo, &["merge", "--abort"]);
        bail!("병합 중 충돌이 나서 되돌렸습니다. 작업 브랜치 ‘{}’는 그대로 남아 있습니다. ({e})", ws.branch);
    }
    let merged = git(repo, &["rev-parse", "--short", "HEAD"])?;
    cleanup(ws, false);
    Ok(merged)
}

/// 작업 공간과(원하면) 작업 브랜치를 지운다. 실패해도 업무 진행에는 영향이 없다.
pub fn cleanup(ws: &Workspace, delete_branch: bool) {
    let repo = Path::new(&ws.repo);
    let _ = git(repo, &["worktree", "remove", "--force", &ws.path]);
    let _ = git(repo, &["worktree", "prune"]);
    let _ = git(repo, &["branch", if delete_branch { "-D" } else { "-d" }, &ws.branch]);
}

/// 리뷰 전용 업무용: 저장소의 현재 변경 또는 최근 커밋 diff.
pub fn review_target(folder: &str, scope: &str) -> Result<(String, String)> {
    let dir = Path::new(folder);
    let repo = git(dir, &["rev-parse", "--show-toplevel"]).map_err(|_| anyhow!("git 저장소 폴더를 골라 주세요."))?;
    let repo_path = Path::new(&repo);
    let (label, patch) = match scope {
        "last3" => ("최근 커밋 3개", git(repo_path, &["log", "-p", "-3", "--no-color", "--format=commit %h %s"])?),
        "last1" => ("최근 커밋 1개", git(repo_path, &["log", "-p", "-1", "--no-color", "--format=commit %h %s"])?),
        _ => ("커밋하지 않은 변경", git(repo_path, &["diff", "HEAD", "--no-color"])?),
    };
    if patch.trim().is_empty() {
        bail!("{label}이 없습니다. 다른 검토 범위를 골라 주세요.");
    }
    Ok((repo, patch.chars().take(120_000).collect()))
}
