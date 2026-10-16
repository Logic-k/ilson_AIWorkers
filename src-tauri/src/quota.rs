//! 구독 한도 조회 (§B-2). 사장님이 "지금 확인" 버튼을 눌렀을 때만 CLI(`/usage`)를 호출하고 10분간 캐시한다.

use crate::engine::{self, Kind};
use crate::models::Settings;
use crate::AppState;
use anyhow::Result;
use chrono::{Duration, Local};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Stdio;
use std::time::Instant;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub kind: String, // "5h" | "7d" | "30d"
    pub label: String,
    pub percent: Option<f64>,
    pub used_label: String,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineQuota {
    pub engine: String,
    pub label: String,
    pub plan: String,
    pub source: String, // "cli" | "estimated"
    pub windows: Vec<QuotaWindow>,
    pub message: Option<String>,
    pub checked_at: String,
}

const CACHE_TTL_SECS: u64 = 600;

pub async fn check_all(state: &AppState, refresh: bool) -> Result<Vec<EngineQuota>> {
    if !refresh {
        let cache = state.quota.lock().unwrap();
        if let Some((at, val)) = cache.get("all") {
            if at.elapsed().as_secs() < CACHE_TTL_SECS {
                if let Ok(parsed) = serde_json::from_value::<Vec<EngineQuota>>(val.clone()) {
                    return Ok(parsed);
                }
            }
        }
    }

    let settings = {
        let conn = state.db.lock().unwrap();
        crate::db::load_settings(&conn)?
    };

    // refresh == false이고 캐시가 없으면 CLI는 실행하지 않고 앱 추정치만 즉시 돌려준다.
    let (agy_cli, claude_cli) = if refresh {
        tokio::join!(probe_agy(), probe_claude())
    } else {
        (None, None)
    };

    let conn = state.db.lock().unwrap();
    let mut out = Vec::new();
    for kind in [Kind::Agy, Kind::Claude, Kind::Codex, Kind::Opencode] {
        let eq = match kind {
            Kind::Agy => {
                if let Some(ref res) = agy_cli {
                    res.clone()
                } else {
                    let mut est = app_estimated(&conn, &settings, kind)?;
                    if !refresh {
                        est.message = Some("‘지금 확인’을 누르면 Antigravity CLI 한도를 조회합니다.".into());
                    }
                    est
                }
            }
            Kind::Claude => {
                if let Some(ref res) = claude_cli {
                    res.clone()
                } else {
                    let mut est = app_estimated(&conn, &settings, kind)?;
                    if !refresh {
                        est.message = Some("‘지금 확인’을 누르면 Claude Code CLI 한도를 조회합니다.".into());
                    }
                    est
                }
            }
            _ => app_estimated(&conn, &settings, kind)?,
        };
        out.push(eq);
    }
    drop(conn);

    if let Ok(v) = serde_json::to_value(&out) {
        state.quota.lock().unwrap().insert("all".into(), (Instant::now(), v));
    }
    Ok(out)
}

fn app_estimated(conn: &Connection, settings: &Settings, kind: Kind) -> Result<EngineQuota> {
    let now = Local::now();
    let iso_5h = (now - Duration::hours(5)).to_rfc3339();
    let iso_7d = (now - Duration::days(7)).to_rfc3339();
    let iso_30d = (now - Duration::days(30)).to_rfc3339();

    let query_win = |since: &str| -> (u64, f64) {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens + cache_read_tokens + cache_write_tokens + output_tokens), 0),
                    COALESCE(SUM(est_cost_usd), 0.0)
             FROM usage_log WHERE engine = ?1 AND at >= ?2",
            params![kind.id(), since],
            |r| Ok((r.get::<_, i64>(0)?.max(0) as u64, r.get::<_, f64>(1)?)),
        )
        .unwrap_or((0, 0.0))
    };

    let (tok_5h, usd_5h) = query_win(&iso_5h);
    let (tok_7d, usd_7d) = query_win(&iso_7d);
    let (tok_30d, usd_30d) = query_win(&iso_30d);

    let plan_code = settings
        .plans
        .get(kind.id())
        .map(String::as_str)
        .unwrap_or(if kind == Kind::Opencode { "free" } else { "" });

    let (cap_5h, cap_7d, cap_30d, plan_label, note) = match (kind, plan_code) {
        (Kind::Opencode, "go") => (
            Some(16.0),
            Some(40.0),
            Some(80.0),
            "OpenCode Go ($10/월)".to_string(),
            Some("Go 약관 기준 한도(5시간 $16 · 주 $40 · 월 $80)로 추정한 비율입니다.".to_string()),
        ),
        (Kind::Opencode, "go_plus") => (
            Some(32.0),
            Some(80.0),
            Some(160.0),
            "OpenCode Go Plus ($40/월)".to_string(),
            Some("Go Plus 약관 기준 한도(5시간 $32 · 주 $80 · 월 $160)로 추정한 비율입니다.".to_string()),
        ),
        (Kind::Opencode, _) => (
            None,
            None,
            None,
            "OpenCode 무료 모델".to_string(),
            Some("무료 모델 한도는 OpenCode 서버가 수요에 따라 동적으로 정합니다(공개 한도 없음).".to_string()),
        ),
        (_, p) => (
            None,
            None,
            None,
            if p.is_empty() { "요금제 미설정".into() } else { p.to_uppercase() },
            Some("공개 한도 조회 API가 없어 앱이 기록한 사용량만 표시합니다.".to_string()),
        ),
    };

    let make_win = |k: &str, label: &str, tok: u64, usd: f64, cap: Option<f64>| -> QuotaWindow {
        let pct = cap.map(|c| ((usd / c) * 100.0).clamp(0.0, 999.0));
        let used_label = if let Some(c) = cap {
            format!("${usd:.2} / ${c:.0} ({tok} 토큰)")
        } else {
            format!("{tok} 토큰 (환산 ${usd:.2})")
        };
        QuotaWindow {
            kind: k.to_string(),
            label: label.to_string(),
            percent: pct,
            used_label,
            resets_at: None,
        }
    };

    Ok(EngineQuota {
        engine: kind.id().to_string(),
        label: kind.label().to_string(),
        plan: plan_label,
        source: "estimated".to_string(),
        windows: vec![
            make_win("5h", "최근 5시간", tok_5h, usd_5h, cap_5h),
            make_win("7d", "최근 7일", tok_7d, usd_7d, cap_7d),
            make_win("30d", "최근 30일", tok_30d, usd_30d, cap_30d),
        ],
        message: note,
        checked_at: crate::db::now(),
    })
}

async fn probe_claude() -> Option<EngineQuota> {
    let bin = engine::locate(Kind::Claude)?;
    let mut cmd = engine::command_for(Kind::Claude, &bin);
    cmd.args(["-p", "/usage", "--output-format", "stream-json", "--verbose"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"/usage\n").await;
        drop(stdin);
    }
    let out = tokio::time::timeout(std::time::Duration::from_secs(12), child.wait_with_output())
        .await
        .ok()?
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut windows = Vec::new();
    for line in text.lines() {
        let Ok(ev) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(limits) = ev["usage_report"]["rate_limits"]["limits"]
            .as_array()
            .or_else(|| ev["message"]["usage_report"]["rate_limits"]["limits"].as_array())
        {
            for item in limits {
                let k = item["kind"].as_str().unwrap_or("5h").to_string();
                let pct = item["percent"].as_f64();
                let resets = item["resets_at"].as_str().map(String::from);
                windows.push(QuotaWindow {
                    label: if k == "5h" { "5시간 창".into() } else { format!("{k} 창") },
                    kind: k,
                    percent: pct,
                    used_label: pct.map(|p| format!("{p:.0}% 사용")).unwrap_or_else(|| "확인됨".into()),
                    resets_at: resets,
                });
            }
        }
    }
    if windows.is_empty() {
        Some(EngineQuota {
            engine: "claude".into(),
            label: Kind::Claude.label().into(),
            plan: "Claude 구독".into(),
            source: "estimated".into(),
            windows: vec![],
            message: Some("이 버전은 사용량 보고를 지원하지 않습니다.".into()),
            checked_at: crate::db::now(),
        })
    } else {
        Some(EngineQuota {
            engine: "claude".into(),
            label: Kind::Claude.label().into(),
            plan: "Claude 구독".into(),
            source: "cli".into(),
            windows,
            message: None,
            checked_at: crate::db::now(),
        })
    }
}

async fn probe_agy() -> Option<EngineQuota> {
    let bin = engine::locate(Kind::Agy)?;
    let mut cmd = engine::command_for(Kind::Agy, &bin);
    cmd.args(["-p", "/usage", "--output-format", "json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = tokio::time::timeout(std::time::Duration::from_secs(12), cmd.output())
        .await
        .ok()?
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let ev: Value = serde_json::from_str(text.trim()).ok()?;
    let mut windows = Vec::new();
    if let Some(groups) = ev["command"]["data"]["groups"].as_array() {
        for g in groups {
            if let Some(buckets) = g["buckets"].as_array() {
                for b in buckets {
                    let name = b["name"].as_str().unwrap_or("한도");
                    let win = b["window"].as_str().unwrap_or("5h");
                    let rem = b["remaining_fraction"].as_f64();
                    let used_pct = rem.map(|r| ((1.0 - r).clamp(0.0, 1.0)) * 100.0);
                    let resets = b["reset_time"].as_str().map(String::from);
                    windows.push(QuotaWindow {
                        kind: win.to_string(),
                        label: format!("{name} ({win})"),
                        percent: used_pct,
                        used_label: used_pct
                            .map(|p| format!("{p:.0}% 사용"))
                            .unwrap_or_else(|| "확인됨".into()),
                        resets_at: resets,
                    });
                }
            }
        }
    }
    if windows.is_empty() {
        return None;
    }
    Some(EngineQuota {
        engine: "agy".into(),
        label: Kind::Agy.label().into(),
        plan: "Antigravity 구독".into(),
        source: "cli".into(),
        windows,
        message: None,
        checked_at: crate::db::now(),
    })
}
