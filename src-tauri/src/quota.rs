//! 구독 한도 조회. OpenCode 사용량과 추정치를 계산해 돌려준다.

use crate::engine::{self, Kind};
use crate::models::Settings;
use crate::AppState;
use anyhow::Result;
use chrono::{Duration, Local};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::time::Instant;

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

    let conn = state.db.lock().unwrap();
    let mut out = Vec::new();
    for kind in engine::ALL {
        let eq = app_estimated(&conn, &settings, kind)?;
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
        .unwrap_or("free");

    let (cap_5h, cap_7d, cap_30d, plan_label, note) = match plan_code {
        "go" => (
            Some(16.0),
            Some(40.0),
            Some(80.0),
            "OpenCode Go ($10/월)".to_string(),
            Some("Go 약관 기준 한도(5시간 $16 · 주 $40 · 월 $80)로 추정한 비율입니다.".to_string()),
        ),
        "go_plus" => (
            Some(32.0),
            Some(80.0),
            Some(160.0),
            "OpenCode Go Plus ($40/월)".to_string(),
            Some("Go Plus 약관 기준 한도(5시간 $32 · 주 $80 · 월 $160)로 추정한 비율입니다.".to_string()),
        ),
        _ => (
            None,
            None,
            None,
            "OpenCode 무료 모델".to_string(),
            Some("무료 모델 한도는 OpenCode 서버가 수요에 따라 동적으로 정합니다(공개 한도 없음).".to_string()),
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
