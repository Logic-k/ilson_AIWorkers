//! 토큰·환산 비용 기록, 집계, 사전 추정, 일일 예산 상한 검사.

use crate::engine::{Kind, RunResult};
use crate::models::Settings;
use crate::pricing;
use crate::roster;
use crate::workers::{self, AiJob, Ctx};
use anyhow::Result;
use chrono::{Duration, Local};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
    pub reasoning: u64,
    pub reported_cost_usd: Option<f64>,
    pub duration_ms: u64,
}

impl From<&RunResult> for Usage {
    fn from(r: &RunResult) -> Self {
        Self {
            input: r.input_tokens,
            cache_read: r.cache_read_tokens,
            cache_write: r.cache_write_tokens,
            output: r.output_tokens,
            reasoning: r.reasoning_tokens,
            reported_cost_usd: r.reported_cost_usd,
            duration_ms: r.duration_ms,
        }
    }
}

impl From<RunResult> for Usage {
    fn from(r: RunResult) -> Self {
        Self::from(&r)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRow {
    pub id: i64,
    pub task_id: String,
    pub step_key: String,
    pub actor: String,
    pub engine: String,
    pub model: Option<String>,
    pub model_source: String,
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub reported_cost_usd: Option<f64>,
    pub est_cost_usd: f64,
    pub est_krw: i64,
    pub price_version: Option<String>,
    pub usd_krw: f64,
    pub prompt_chars: Option<u64>,
    pub est_input_tokens: Option<u64>,
    pub duration_ms: Option<u64>,
    pub mode: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineUsage {
    pub engine: String,
    pub runs: u64,
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub est_usd: f64,
    pub est_krw: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub engine: String,
    pub model: String,
    pub runs: u64,
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub est_usd: f64,
    pub est_krw: i64,
    pub free: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorUsage {
    pub actor: String,
    pub runs: u64,
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub est_usd: f64,
    pub est_krw: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayUsage {
    pub day: String,
    pub runs: u64,
    pub total_tokens: u64,
    pub est_usd: f64,
    pub est_krw: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetAlert {
    pub scope: String,
    pub label: String,
    pub metric: String,
    pub used: u64,
    pub cap: u64,
    pub percent: f64,
    pub status: String, // "ok" | "warn" | "over"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub range: String,
    pub runs: u64,
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub est_usd: f64,
    pub est_krw: i64,
    pub reported_usd: f64,
    pub today_tokens: u64,
    pub today_usd: f64,
    pub today_krw: i64,
    pub week_tokens: u64,
    pub week_usd: f64,
    pub week_krw: i64,
    pub month_tokens: u64,
    pub month_usd: f64,
    pub month_krw: i64,
    pub yesterday_tasks: u64,
    pub yesterday_tokens: u64,
    pub by_engine: Vec<EngineUsage>,
    pub by_model: Vec<ModelUsage>,
    pub by_actor: Vec<ActorUsage>,
    pub by_day: Vec<DayUsage>,
    pub budgets: Vec<BudgetAlert>,
    pub price_version: String,
    pub price_source: String,
    pub usd_krw: f64,
    pub fx_updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    pub engine: String,
    pub model: String,
    pub free: bool,
    pub prompt_chars: u64,
    pub est_input: u64,
    pub est_output: u64,
    pub est_total: u64,
    pub est_usd: f64,
    pub est_krw: i64,
    pub basis: String, // "heuristic" | "history"
    pub samples: u64,
}

/// 단계 실행 후 `usage_log`에 한 줄을 남기고 환산 비용(USD)을 돌려준다.
#[allow(clippy::too_many_arguments)]
pub fn record(
    conn: &Connection,
    ctx: &Ctx,
    step_key: &str,
    job: &AiJob,
    usage: &Usage,
    model: Option<&str>,
    model_source: &str,
    est_input: Option<u64>,
    prompt_chars: Option<u64>,
    mode: &str,
) -> Result<f64> {
    let eng = job.engine.id();
    let price = pricing::resolve(eng, model);
    let est_cost_usd = if mode == "safe" {
        0.0
    } else if let Some(ref p) = price {
        pricing::cost(p, usage.input, usage.cache_read, usage.cache_write, usage.output)
    } else {
        0.0
    };
    let resolved_model = model
        .map(str::to_string)
        .or_else(|| price.as_ref().map(|p| p.model_key.clone()));
    let fx = ctx.settings.effective_usd_krw();
    conn.execute(
        "INSERT INTO usage_log (
            task_id, step_key, actor, engine, model, model_source,
            input_tokens, cache_read_tokens, cache_write_tokens, output_tokens, reasoning_tokens,
            reported_cost_usd, est_cost_usd, price_version, usd_krw,
            prompt_chars, est_input_tokens, duration_ms, mode, at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            ctx.task_id,
            step_key,
            job.actor,
            eng,
            resolved_model,
            model_source,
            usage.input as i64,
            usage.cache_read as i64,
            usage.cache_write as i64,
            usage.output as i64,
            usage.reasoning as i64,
            usage.reported_cost_usd,
            est_cost_usd,
            pricing::generated_at(),
            fx,
            prompt_chars.map(|n| n as i64),
            est_input.map(|n| n as i64),
            Some(usage.duration_ms as i64),
            mode,
            crate::db::now(),
        ],
    )?;
    Ok(est_cost_usd)
}

pub fn for_task(conn: &Connection, task_id: &str, fallback_fx: f64) -> Result<Vec<UsageRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, step_key, actor, engine, model, model_source,
                input_tokens, cache_read_tokens, cache_write_tokens, output_tokens, reasoning_tokens,
                reported_cost_usd, est_cost_usd, price_version, usd_krw,
                prompt_chars, est_input_tokens, duration_ms, mode, at
         FROM usage_log WHERE task_id = ?1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![task_id], |r| {
        let est_cost_usd: f64 = r.get(13)?;
        let usd_krw: f64 = r.get::<_, Option<f64>>(15)?.unwrap_or(fallback_fx);
        Ok(UsageRow {
            id: r.get(0)?,
            task_id: r.get(1)?,
            step_key: r.get(2)?,
            actor: r.get(3)?,
            engine: r.get(4)?,
            model: r.get(5)?,
            model_source: r.get(6)?,
            input_tokens: r.get::<_, i64>(7)?.max(0) as u64,
            cache_read_tokens: r.get::<_, i64>(8)?.max(0) as u64,
            cache_write_tokens: r.get::<_, i64>(9)?.max(0) as u64,
            output_tokens: r.get::<_, i64>(10)?.max(0) as u64,
            reasoning_tokens: r.get::<_, i64>(11)?.max(0) as u64,
            reported_cost_usd: r.get(12)?,
            est_cost_usd,
            est_krw: (est_cost_usd * usd_krw).round() as i64,
            price_version: r.get(14)?,
            usd_krw,
            prompt_chars: r.get::<_, Option<i64>>(16)?.map(|n| n.max(0) as u64),
            est_input_tokens: r.get::<_, Option<i64>>(17)?.map(|n| n.max(0) as u64),
            duration_ms: r.get::<_, Option<i64>>(18)?.map(|n| n.max(0) as u64),
            mode: r.get(19)?,
            at: r.get(20)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn cutoff_for_range(range: &str) -> Option<String> {
    let now = Local::now();
    match range {
        "today" => Some(format!("{}T00:00:00", now.format("%Y-%m-%d"))),
        "7d" => Some((now - Duration::days(7)).to_rfc3339()),
        "30d" => Some((now - Duration::days(30)).to_rfc3339()),
        _ => None,
    }
}

fn period_rollup(conn: &Connection, since_prefix_or_iso: &str, is_date_prefix: bool, fx: f64) -> Result<(u64, f64, i64)> {
    let (tok, usd): (i64, f64) = if is_date_prefix {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens + cache_read_tokens + cache_write_tokens + output_tokens), 0),
                    COALESCE(SUM(est_cost_usd), 0.0)
             FROM usage_log WHERE substr(at, 1, 10) = ?1",
            params![since_prefix_or_iso],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?
    } else {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens + cache_read_tokens + cache_write_tokens + output_tokens), 0),
                    COALESCE(SUM(est_cost_usd), 0.0)
             FROM usage_log WHERE at >= ?1",
            params![since_prefix_or_iso],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?
    };
    let tokens = tok.max(0) as u64;
    Ok((tokens, usd, (usd * fx).round() as i64))
}

pub fn summary(conn: &Connection, settings: &Settings, range: &str) -> Result<UsageSummary> {
    let fx = settings.effective_usd_krw();
    let cutoff = cutoff_for_range(range).unwrap_or_else(|| "0000-01-01".to_string());

    let (runs, inp, cr, cw, out, reas, est_usd, rep_usd): (i64, i64, i64, i64, i64, i64, f64, f64) =
        conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cache_read_tokens), 0),
                    COALESCE(SUM(cache_write_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_tokens), 0),
                    COALESCE(SUM(est_cost_usd), 0.0),
                    COALESCE(SUM(reported_cost_usd), 0.0)
             FROM usage_log WHERE at >= ?1",
            params![cutoff],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            },
        )?;

    let input_tokens = inp.max(0) as u64;
    let cache_read_tokens = cr.max(0) as u64;
    let cache_write_tokens = cw.max(0) as u64;
    let output_tokens = out.max(0) as u64;
    let reasoning_tokens = reas.max(0) as u64;
    let total_tokens = input_tokens + cache_read_tokens + cache_write_tokens + output_tokens;

    let now = Local::now();
    let today_str = now.format("%Y-%m-%d").to_string();
    let yesterday_str = (now - Duration::days(1)).format("%Y-%m-%d").to_string();
    let week_iso = (now - Duration::days(7)).to_rfc3339();
    let month_iso = (now - Duration::days(30)).to_rfc3339();

    let (today_tokens, today_usd, today_krw) = period_rollup(conn, &today_str, true, fx)?;
    let (week_tokens, week_usd, week_krw) = period_rollup(conn, &week_iso, false, fx)?;
    let (month_tokens, month_usd, month_krw) = period_rollup(conn, &month_iso, false, fx)?;
    let (yesterday_tokens, _, _) = period_rollup(conn, &yesterday_str, true, fx)?;
    let yesterday_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'completed' AND substr(updated_at, 1, 10) = ?1",
            params![yesterday_str],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // 엔진별 집계
    let mut stmt = conn.prepare(
        "SELECT engine,
                COUNT(*),
                COALESCE(SUM(input_tokens), 0),
                COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(cache_write_tokens), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(reasoning_tokens), 0),
                COALESCE(SUM(est_cost_usd), 0.0)
         FROM usage_log WHERE at >= ?1
         GROUP BY engine ORDER BY SUM(est_cost_usd) DESC, COUNT(*) DESC",
    )?;
    let by_engine = stmt
        .query_map(params![cutoff], |r| {
            let i = r.get::<_, i64>(2)?.max(0) as u64;
            let cr = r.get::<_, i64>(3)?.max(0) as u64;
            let cw = r.get::<_, i64>(4)?.max(0) as u64;
            let o = r.get::<_, i64>(5)?.max(0) as u64;
            let re = r.get::<_, i64>(6)?.max(0) as u64;
            let u: f64 = r.get(7)?;
            Ok(EngineUsage {
                engine: r.get(0)?,
                runs: r.get::<_, i64>(1)?.max(0) as u64,
                input_tokens: i,
                cache_read_tokens: cr,
                cache_write_tokens: cw,
                output_tokens: o,
                reasoning_tokens: re,
                total_tokens: i + cr + cw + o,
                est_usd: u,
                est_krw: (u * fx).round() as i64,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // 모델별 집계
    let mut stmt = conn.prepare(
        "SELECT engine,
                COALESCE(model, '(기본 모델)'),
                COUNT(*),
                COALESCE(SUM(input_tokens), 0),
                COALESCE(SUM(cache_read_tokens + cache_write_tokens), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(est_cost_usd), 0.0)
         FROM usage_log WHERE at >= ?1
         GROUP BY engine, COALESCE(model, '(기본 모델)')
         ORDER BY SUM(est_cost_usd) DESC, COUNT(*) DESC",
    )?;
    let by_model = stmt
        .query_map(params![cutoff], |r| {
            let eng: String = r.get(0)?;
            let mdl: String = r.get(1)?;
            let i = r.get::<_, i64>(3)?.max(0) as u64;
            let c = r.get::<_, i64>(4)?.max(0) as u64;
            let o = r.get::<_, i64>(5)?.max(0) as u64;
            let u: f64 = r.get(6)?;
            let free = pricing::resolve(&eng, Some(&mdl))
                .map(|p| p.list_cost_zero)
                .unwrap_or(false);
            Ok(ModelUsage {
                engine: eng,
                model: mdl,
                runs: r.get::<_, i64>(2)?.max(0) as u64,
                input_tokens: i,
                cache_read_tokens: c,
                output_tokens: o,
                total_tokens: i + c + o,
                est_usd: u,
                est_krw: (u * fx).round() as i64,
                free,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // 직원별 집계
    let mut stmt = conn.prepare(
        "SELECT actor,
                COUNT(*),
                COALESCE(SUM(input_tokens), 0),
                COALESCE(SUM(cache_read_tokens + cache_write_tokens), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(est_cost_usd), 0.0)
         FROM usage_log WHERE at >= ?1
         GROUP BY actor
         ORDER BY SUM(est_cost_usd) DESC, COUNT(*) DESC",
    )?;
    let by_actor = stmt
        .query_map(params![cutoff], |r| {
            let i = r.get::<_, i64>(2)?.max(0) as u64;
            let c = r.get::<_, i64>(3)?.max(0) as u64;
            let o = r.get::<_, i64>(4)?.max(0) as u64;
            let u: f64 = r.get(5)?;
            Ok(ActorUsage {
                actor: r.get(0)?,
                runs: r.get::<_, i64>(1)?.max(0) as u64,
                input_tokens: i,
                cache_read_tokens: c,
                output_tokens: o,
                total_tokens: i + c + o,
                est_usd: u,
                est_krw: (u * fx).round() as i64,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // 일별 집계 (최근 30일)
    let mut stmt = conn.prepare(
        "SELECT substr(at, 1, 10) AS day,
                COUNT(*),
                COALESCE(SUM(input_tokens + cache_read_tokens + cache_write_tokens + output_tokens), 0),
                COALESCE(SUM(est_cost_usd), 0.0)
         FROM usage_log WHERE at >= ?1
         GROUP BY day
         ORDER BY day DESC LIMIT 30",
    )?;
    let by_day = stmt
        .query_map(params![month_iso], |r| {
            let u: f64 = r.get(3)?;
            Ok(DayUsage {
                day: r.get(0)?,
                runs: r.get::<_, i64>(1)?.max(0) as u64,
                total_tokens: r.get::<_, i64>(2)?.max(0) as u64,
                est_usd: u,
                est_krw: (u * fx).round() as i64,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let budgets = compute_budget_alerts(conn, settings)?;

    Ok(UsageSummary {
        range: range.to_string(),
        runs: runs.max(0) as u64,
        input_tokens,
        cache_read_tokens,
        cache_write_tokens,
        output_tokens,
        reasoning_tokens,
        total_tokens,
        est_usd,
        est_krw: (est_usd * fx).round() as i64,
        reported_usd: rep_usd,
        today_tokens,
        today_usd,
        today_krw,
        week_tokens,
        week_usd,
        week_krw,
        month_tokens,
        month_usd,
        month_krw,
        yesterday_tasks: yesterday_tasks.max(0) as u64,
        yesterday_tokens,
        by_engine,
        by_model,
        by_actor,
        by_day,
        budgets,
        price_version: pricing::generated_at().to_string(),
        price_source: pricing::source().to_string(),
        usd_krw: fx,
        fx_updated_at: settings.fx_updated_at.clone(),
    })
}

// ───────────────────────── 글자 분류 및 토큰 추정 (§B-2) ─────────────────────────
//
// 글자당 토큰 계수 초기값 (보정 대상 — 실측 이력 3건 이상 쌓이면 이력 중앙값으로 자동 보정):
// | 분류 | agy(Gemini) | claude | codex(o200k) | opencode(DeepSeek) |
// | 한글 음절·자모 | 0.58 | 1.05 | 0.65 | 0.90 |
// | 한자·가나 | 0.60 | 1.00 | 0.70 | 0.60 |
// | 라틴·숫자·공백·구두점 | 0.22 | 0.36 | 0.22 | 0.30 |
// | 코드 블록 안(``` 사이) | 0.30 | 0.48 | 0.30 | 0.33 |

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CharBuckets {
    pub hangul: usize,
    pub cjk_kana: usize,
    pub latin_other: usize,
    pub code: usize,
}

pub fn classify_chars(text: &str) -> CharBuckets {
    let mut b = CharBuckets::default();
    let mut in_code = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            b.code += line.chars().count();
            continue;
        }
        if in_code {
            b.code += line.chars().count() + 1;
            continue;
        }
        for c in line.chars() {
            let u = c as u32;
            if (0xAC00..=0xD7A3).contains(&u)
                || (0x1100..=0x11FF).contains(&u)
                || (0x3130..=0x318F).contains(&u)
            {
                b.hangul += 1;
            } else if (0x4E00..=0x9FFF).contains(&u)
                || (0x3040..=0x309F).contains(&u)
                || (0x30A0..=0x30FF).contains(&u)
            {
                b.cjk_kana += 1;
            } else {
                b.latin_other += 1;
            }
        }
        b.latin_other += 1; // 줄바꿈
    }
    b
}

pub fn engine_overhead(engine: Kind) -> u64 {
    match engine {
        Kind::Opencode => 6_000,
    }
}

/// 프롬프트 문자열만으로 휴리스틱 입력 토큰(오버헤드 포함)을 추정한다.
pub fn estimate_prompt_tokens(engine: Kind, prompt: &str) -> u64 {
    let b = classify_chars(prompt);
    let (k_ko, k_cjk, k_lat, k_code) = match engine {
        Kind::Opencode => (0.90, 0.60, 0.30, 0.33),
    };
    let body = b.hangul as f64 * k_ko
        + b.cjk_kana as f64 * k_cjk
        + b.latin_other as f64 * k_lat
        + b.code as f64 * k_code;
    engine_overhead(engine) + body.round() as u64
}

fn median_f64(mut vals: Vec<f64>) -> f64 {
    if vals.is_empty() {
        return 1.0;
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = vals.len() / 2;
    if vals.len() % 2 == 1 {
        vals[mid]
    } else {
        (vals[mid - 1] + vals[mid]) / 2.0
    }
}

fn median_u64(mut vals: Vec<u64>) -> u64 {
    if vals.is_empty() {
        return 0;
    }
    vals.sort_unstable();
    let mid = vals.len() / 2;
    if vals.len() % 2 == 1 {
        vals[mid]
    } else {
        (vals[mid - 1] + vals[mid]) / 2
    }
}

/// 업무 맡기기 전(Preflight) 또는 단계 시작 전 토큰·환산 비용을 추정한다.
pub fn estimate(
    conn: &Connection,
    settings: &Settings,
    employee_id: &str,
    input: &Value,
) -> Result<Estimate> {
    let engine = settings.engine_for(employee_id);
    let model_opt = settings.model_for(engine);
    let price = pricing::resolve(engine.id(), model_opt.as_deref());
    let model = model_opt
        .or_else(|| price.as_ref().map(|p| p.model_key.clone()))
        .unwrap_or_else(|| pricing::default_model_for(engine.id()).to_string());
    let free = price.as_ref().map(|p| p.list_cost_zero).unwrap_or(false);

    // 로컬 전용 직원(수진·도현)은 AI 토큰을 쓰지 않는다.
    if matches!(employee_id, "sujin" | "dohyun") {
        return Ok(Estimate {
            engine: engine.id().to_string(),
            model,
            free: true,
            prompt_chars: 0,
            est_input: 0,
            est_output: 0,
            est_total: 0,
            est_usd: 0.0,
            est_krw: 0,
            basis: "heuristic".into(),
            samples: 0,
        });
    }

    let (prompt, typical_output, step_key) = if roster::DEV_TEAM.contains(&employee_id) {
        let req = input["request"].as_str().unwrap_or("").trim();
        let focus = input["focus"].as_str().unwrap_or("").trim();
        let body = if !req.is_empty() { req } else { focus };
        let approx = format!(
            "{}\n\n{}\n\n{}",
            workers::persona_text(employee_id),
            workers::rules_preview(input["allowRun"].as_bool().unwrap_or(false), employee_id),
            body
        );
        let (sk, typ) = match employee_id {
            "taeo" => ("plan", 4500u32),
            "minhyuk" => ("review", 2500u32),
            "sohee" => ("test", 3500u32),
            "daeun" => ("docs", 3000u32),
            _ => ("build", 4000u32),
        };
        (approx, typ, sk)
    } else {
        let empty_outputs = HashMap::new();
        let (p, _searches, typ) =
            workers::ai_request_from_input(settings, employee_id, input, &empty_outputs)?;
        let sk = match employee_id {
            "harin" | "jiho" => "write",
            "seojin" | "yena" => "analyze",
            "minjae" => "research",
            _ => "write",
        };
        (p, typ, sk)
    };

    let prompt_chars = prompt.chars().count() as u64;
    let raw_est_input = estimate_prompt_tokens(engine, &prompt);

    // 같은 (engine, actor, step_key)의 최근 10건 이력 보정
    let mut stmt = conn.prepare(
        "SELECT input_tokens + cache_read_tokens, est_input_tokens, output_tokens
         FROM usage_log
         WHERE engine = ?1 AND actor = ?2 AND step_key = ?3
           AND mode = 'auto'
           AND (input_tokens + cache_read_tokens) > 0
           AND est_input_tokens IS NOT NULL AND est_input_tokens > 0
         ORDER BY id DESC LIMIT 10",
    )?;
    let rows: Vec<(u64, u64, u64)> = stmt
        .query_map(params![engine.id(), employee_id, step_key], |r| {
            Ok((
                r.get::<_, i64>(0)?.max(0) as u64,
                r.get::<_, i64>(1)?.max(1) as u64,
                r.get::<_, i64>(2)?.max(0) as u64,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let samples = rows.len() as u64;
    let (est_input, est_output, basis) = if samples >= 3 {
        let ratios: Vec<f64> = rows
            .iter()
            .map(|(actual, est, _)| (*actual as f64 / *est as f64).clamp(0.2, 5.0))
            .collect();
        let outs: Vec<u64> = rows.iter().map(|(_, _, o)| *o).collect();
        let ratio = median_f64(ratios);
        let cal_in = ((raw_est_input as f64) * ratio).round() as u64;
        let cal_out = median_u64(outs).max(100);
        (cal_in, cal_out, "history".to_string())
    } else {
        (raw_est_input, typical_output as u64, "heuristic".to_string())
    };

    let est_total = est_input + est_output;
    let est_usd = price
        .as_ref()
        .map(|p| pricing::cost(p, est_input, 0, 0, est_output))
        .unwrap_or(0.0);
    let est_krw = (est_usd * settings.effective_usd_krw()).round() as i64;

    Ok(Estimate {
        engine: engine.id().to_string(),
        model,
        free,
        prompt_chars,
        est_input,
        est_output,
        est_total,
        est_usd,
        est_krw,
        basis,
        samples,
    })
}

// ───────────────────────── 예산 상한 (§B-5) ─────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetDecision {
    Ok,
    Warn(String),
    Pause(String),
}

fn today_usage_for(conn: &Connection, engine: Option<&str>, fx: f64) -> Result<(u64, i64)> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let (tok, usd): (i64, f64) = if let Some(eng) = engine {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens + cache_read_tokens + cache_write_tokens + output_tokens), 0),
                    COALESCE(SUM(est_cost_usd), 0.0)
             FROM usage_log WHERE substr(at, 1, 10) = ?1 AND engine = ?2",
            params![today, eng],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?
    } else {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens + cache_read_tokens + cache_write_tokens + output_tokens), 0),
                    COALESCE(SUM(est_cost_usd), 0.0)
             FROM usage_log WHERE substr(at, 1, 10) = ?1",
            params![today],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?
    };
    Ok((tok.max(0) as u64, (usd * fx).round() as i64))
}

pub fn compute_budget_alerts(conn: &Connection, settings: &Settings) -> Result<Vec<BudgetAlert>> {
    let fx = settings.effective_usd_krw();
    let mut alerts = Vec::new();

    let push_cap = |alerts: &mut Vec<BudgetAlert>,
                    scope: &str,
                    label: &str,
                    used_tok: u64,
                    used_krw: i64,
                    cap_tok: Option<u64>,
                    cap_krw: Option<i64>| {
        if let Some(ct) = cap_tok.filter(|&c| c > 0) {
            let pct = (used_tok as f64 / ct as f64) * 100.0;
            let status = if pct >= 100.0 {
                "over"
            } else if pct >= 80.0 {
                "warn"
            } else {
                "ok"
            };
            alerts.push(BudgetAlert {
                scope: scope.to_string(),
                label: label.to_string(),
                metric: "tokens".to_string(),
                used: used_tok,
                cap: ct,
                percent: (pct * 10.0).round() / 10.0,
                status: status.to_string(),
            });
        }
        if let Some(ck) = cap_krw.filter(|&c| c > 0) {
            let used = used_krw.max(0) as u64;
            let cap = ck as u64;
            let pct = (used as f64 / cap as f64) * 100.0;
            let status = if pct >= 100.0 {
                "over"
            } else if pct >= 80.0 {
                "warn"
            } else {
                "ok"
            };
            alerts.push(BudgetAlert {
                scope: scope.to_string(),
                label: label.to_string(),
                metric: "krw".to_string(),
                used,
                cap,
                percent: (pct * 10.0).round() / 10.0,
                status: status.to_string(),
            });
        }
    };

    let (all_tok, all_krw) = today_usage_for(conn, None, fx)?;
    push_cap(
        &mut alerts,
        "all",
        "전체 일일 예산",
        all_tok,
        all_krw,
        settings.budgets.daily_tokens,
        settings.budgets.daily_krw,
    );

    for k in crate::engine::ALL {
        if let Some(cap) = settings.budgets.per_engine.get(k.id()) {
            let (e_tok, e_krw) = today_usage_for(conn, Some(k.id()), fx)?;
            push_cap(
                &mut alerts,
                k.id(),
                k.label(),
                e_tok,
                e_krw,
                cap.daily_tokens,
                cap.daily_krw,
            );
        }
    }

    Ok(alerts)
}

/// 자동 모드 AI 단계 시작 전에 오늘 사용량 + 이번 단계 예상치를 합쳐 예산 상한(80%/100%)을 판정한다.
pub fn check_budget_before_step(
    conn: &Connection,
    settings: &Settings,
    engine: Kind,
    est_tokens: u64,
    est_krw: i64,
) -> Result<BudgetDecision> {
    let fx = settings.effective_usd_krw();
    let (all_tok, all_krw) = today_usage_for(conn, None, fx)?;
    let (eng_tok, eng_krw) = today_usage_for(conn, Some(engine.id()), fx)?;

    let mut max_pct: f64 = 0.0;
    let mut max_label = String::from("전체");

    let mut check = |label: &str, next_val: u64, cap: Option<u64>| {
        if let Some(c) = cap.filter(|&c| c > 0) {
            let pct = (next_val as f64 / c as f64) * 100.0;
            if pct > max_pct {
                max_pct = pct;
                max_label = label.to_string();
            }
        }
    };

    check(
        "전체",
        all_tok.saturating_add(est_tokens),
        settings.budgets.daily_tokens,
    );
    check(
        "전체",
        (all_krw.saturating_add(est_krw.max(0))).max(0) as u64,
        settings.budgets.daily_krw.filter(|&c| c > 0).map(|c| c as u64),
    );

    if let Some(ec) = settings.budgets.per_engine.get(engine.id()) {
        check(
            engine.label(),
            eng_tok.saturating_add(est_tokens),
            ec.daily_tokens,
        );
        check(
            engine.label(),
            (eng_krw.saturating_add(est_krw.max(0))).max(0) as u64,
            ec.daily_krw.filter(|&c| c > 0).map(|c| c as u64),
        );
    }

    if max_pct >= 100.0 {
        Ok(BudgetDecision::Pause(
            "예산 상한에 닿아 멈췄습니다. 내일 자정에 다시 열리거나, 연결 및 설정에서 상한을 올린 뒤 이어하기".to_string(),
        ))
    } else if max_pct >= 80.0 {
        Ok(BudgetDecision::Warn(format!(
            "오늘 {} 예산 {}% 사용",
            max_label,
            max_pct.round() as u64
        )))
    } else {
        Ok(BudgetDecision::Ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ status, BudgetCap, Task };
    use serde_json::json;

    #[test]
    fn classify_chars_separates_korean_cjk_latin_and_code() {
        let sample = "안녕하세요 Hello 漢字\n```rust\nlet x = 42;\n```\n끝";
        let b = classify_chars(sample);
        assert_eq!(b.hangul, 6); // "안녕하세요" (5) + "끝" (1)
        assert_eq!(b.cjk_kana, 2); // "漢字"
        assert!(b.code > 10);
        assert!(b.latin_other >= 6);
    }

    #[test]
    fn summary_and_budget_caps_work_with_in_memory_sqlite() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn).unwrap();

        let now = crate::db::now();
        let task = Task {
            id: "t1".into(),
            employee_id: "harin".into(),
            title: "테스트 업무".into(),
            status: status::RUNNING.into(),
            input: json!({"keyword": "성수동 카페", "length": 1500}),
            progress: 0,
            current_step: None,
            current_actor: None,
            pending_gate: None,
            approved_gates: vec![],
            cost_usd: 0.0,
            result: None,
            error: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            seen_at: None,
            steps: vec![],
            events: vec![],
        };
        crate::db::insert_task(&conn, &task, &[("write".into(), "초안 작성".into())]).unwrap();

        let mut settings = Settings::default();
        settings.assignments.insert("harin".into(), "opencode".into());

        // 3건 기록을 넣어 이력 보정(history)과 집계를 함께 검증한다.
        for i in 0..3 {
            conn.execute(
                "INSERT INTO usage_log (
                    task_id, step_key, actor, engine, model, model_source,
                    input_tokens, cache_read_tokens, cache_write_tokens, output_tokens, reasoning_tokens,
                    reported_cost_usd, est_cost_usd, price_version, usd_krw,
                    prompt_chars, est_input_tokens, duration_ms, mode, at
                ) VALUES ('t1', 'write', 'harin', 'opencode', 'opencode/deepseek-v4.1-flash-free', 'default',
                          10000, 2000, 0, 3000, 500, 0.0, 0.0066, '2026-10-16', 1343.0,
                          1200, 8000, 1500, 'auto', ?1)",
                params![format!("{now}_{i}")],
            )
            .unwrap();
        }

        let sum = summary(&conn, &settings, "today").unwrap();
        assert_eq!(sum.runs, 3);
        assert_eq!(sum.input_tokens, 30_000);
        assert_eq!(sum.cache_read_tokens, 6_000);
        assert_eq!(sum.output_tokens, 9_000);
        assert_eq!(sum.total_tokens, 45_000);
        assert_eq!(sum.by_engine.len(), 1);
        assert_eq!(sum.by_engine[0].engine, "opencode");

        // 3건 이상 쌓였으므로 estimate의 basis가 "history"여야 한다.
        let est = estimate(&conn, &settings, "harin", &task.input).unwrap();
        assert_eq!(est.basis, "history");
        assert_eq!(est.samples, 3);
        assert!(est.free);

        // 예산 상한 검사: 현재 오늘 45,000 토큰 사용 중
        // 상한 55,000 토큰 + 추가 1,000 토큰 => 46,000 / 55,000 = 83.6% => Warn
        settings.budgets.per_engine.insert(
            "opencode".into(),
            BudgetCap {
                daily_tokens: Some(55_000),
                daily_krw: None,
            },
        );
        let d_warn = check_budget_before_step(&conn, &settings, Kind::Opencode, 1_000, 10).unwrap();
        assert!(matches!(d_warn, BudgetDecision::Warn(_)), "got {d_warn:?}");

        // 상한 50,000 토큰 + 추가 6,000 토큰 => 51,000 / 50,000 = 102% => Pause
        settings.budgets.per_engine.insert(
            "opencode".into(),
            BudgetCap {
                daily_tokens: Some(50_000),
                daily_krw: None,
            },
        );
        let d_pause = check_budget_before_step(&conn, &settings, Kind::Opencode, 6_000, 10).unwrap();
        assert!(matches!(d_pause, BudgetDecision::Pause(_)), "got {d_pause:?}");
    }
}
