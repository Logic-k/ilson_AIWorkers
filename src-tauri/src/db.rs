use crate::models::{status, EventRow, Gate, Settings, StepRow, Task};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;
use std::path::Path;

pub fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

pub fn open(dir: &Path) -> Result<Connection> {
    std::fs::create_dir_all(dir).context("데이터 폴더를 만들 수 없습니다")?;
    let conn = Connection::open(dir.join("ilson.db"))?;
    let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys=ON;
         CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            employee_id TEXT NOT NULL,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            input_json TEXT NOT NULL,
            progress INTEGER NOT NULL DEFAULT 0,
            current_step TEXT,
            pending_gate_json TEXT,
            approved_gates_json TEXT NOT NULL DEFAULT '[]',
            cost_usd REAL NOT NULL DEFAULT 0,
            result_json TEXT,
            error TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS task_steps (
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            idx INTEGER NOT NULL,
            key TEXT NOT NULL,
            label TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            output_json TEXT,
            note TEXT,
            started_at TEXT,
            finished_at TEXT,
            PRIMARY KEY (task_id, idx)
         );
         CREATE TABLE IF NOT EXISTS task_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            at TEXT NOT NULL,
            level TEXT NOT NULL,
            message TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value_json TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS recent_inputs (
            employee_id TEXT PRIMARY KEY,
            input_json TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS guards (
            employee_id TEXT PRIMARY KEY,
            error TEXT NOT NULL,
            at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS usage_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            step_key TEXT NOT NULL,
            actor TEXT NOT NULL,
            engine TEXT NOT NULL,
            model TEXT,
            model_source TEXT NOT NULL,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_write_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            reasoning_tokens INTEGER NOT NULL DEFAULT 0,
            reported_cost_usd REAL,
            est_cost_usd REAL NOT NULL DEFAULT 0,
            price_version TEXT,
            usd_krw REAL,
            prompt_chars INTEGER,
            est_input_tokens INTEGER,
            duration_ms INTEGER,
            mode TEXT NOT NULL,
            at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS usage_log_at ON usage_log(at);
         CREATE INDEX IF NOT EXISTS usage_log_engine ON usage_log(engine, at);",
    )?;
    // 0.2: 단계를 실제로 하는 직원(사무실 화면용) + 결과 열람 시각(inbox 미열람 배지용)
    let _ = conn.execute("ALTER TABLE tasks ADD COLUMN current_actor TEXT", []);
    let _ = conn.execute("ALTER TABLE tasks ADD COLUMN seen_at TEXT", []);
    Ok(())
}

/// 같은 직원의 업무가 같은 이유로 두 번 연달아 실패하면, 원인을 고칠 때까지 새 업무를 막는다.
pub fn check_guard(conn: &Connection, employee_id: &str) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT status, error FROM tasks WHERE employee_id=?1 AND status IN (?2, ?3) ORDER BY updated_at DESC LIMIT 2",
    )?;
    let rows: Vec<(String, Option<String>)> = stmt
        .query_map(params![employee_id, status::FAILED, status::COMPLETED], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    if rows.len() == 2 && rows.iter().all(|(s, _)| s == status::FAILED) && rows[0].1.is_some() && rows[0].1 == rows[1].1 {
        conn.execute(
            "INSERT INTO guards (employee_id, error, at) VALUES (?1, ?2, ?3)
             ON CONFLICT(employee_id) DO UPDATE SET error=excluded.error, at=excluded.at",
            params![employee_id, rows[0].1, now()],
        )?;
    }
    Ok(())
}

pub fn guard(conn: &Connection, employee_id: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT error FROM guards WHERE employee_id=?1", params![employee_id], |r| r.get(0))
        .optional()?)
}

pub fn clear_guard(conn: &Connection, employee_id: &str) -> Result<()> {
    conn.execute("DELETE FROM guards WHERE employee_id=?1", params![employee_id])?;
    Ok(())
}

/// 아직 끝나지 않은 단계의 확인 기록을 지운다. 실패 후 이어하기 때 비용을 다시 묻기 위해서다.
pub fn reset_pending_gates(conn: &Connection, id: &str, done_gates: &[String]) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET approved_gates_json=?2, pending_gate_json=NULL, error=NULL, updated_at=?3 WHERE id=?1",
        params![id, serde_json::to_string(done_gates)?, now()],
    )?;
    Ok(())
}

/// 앱이 꺼진 사이 실행 중이던 업무는 '중단됨'으로 돌려 이어하기를 기다린다.
pub fn recover_interrupted(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM tasks WHERE status IN (?1, ?2)")?;
    let ids: Vec<String> = stmt
        .query_map(params![status::RUNNING, status::QUEUED], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for id in &ids {
        conn.execute(
            "UPDATE task_steps SET status='pending', started_at=NULL WHERE task_id=?1 AND status='running'",
            params![id],
        )?;
        set_status(conn, id, status::PAUSED)?;
        add_event(conn, id, "warn", "앱이 종료되어 업무가 멈췄습니다. 마지막으로 끝낸 단계부터 이어서 할 수 있습니다.")?;
    }
    Ok(ids)
}

fn task_from_row(r: &Row) -> rusqlite::Result<Task> {
    let input: String = r.get("input_json")?;
    let gate: Option<String> = r.get("pending_gate_json")?;
    let approved: String = r.get("approved_gates_json")?;
    let result: Option<String> = r.get("result_json")?;
    Ok(Task {
        id: r.get("id")?,
        employee_id: r.get("employee_id")?,
        title: r.get("title")?,
        status: r.get("status")?,
        input: serde_json::from_str(&input).unwrap_or(Value::Null),
        progress: r.get("progress")?,
        current_step: r.get("current_step")?,
        current_actor: r.get("current_actor")?,
        pending_gate: gate.and_then(|g| serde_json::from_str::<Gate>(&g).ok()),
        approved_gates: serde_json::from_str(&approved).unwrap_or_default(),
        cost_usd: r.get("cost_usd")?,
        result: result.and_then(|s| serde_json::from_str(&s).ok()),
        error: r.get("error")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        seen_at: r.get("seen_at").unwrap_or(None),
        steps: vec![],
        events: vec![],
    })
}

pub fn insert_task(conn: &Connection, task: &Task, steps: &[(String, String)]) -> Result<()> {
    conn.execute(
        "INSERT INTO tasks (id, employee_id, title, status, input_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![
            task.id,
            task.employee_id,
            task.title,
            task.status,
            task.input.to_string(),
            task.created_at
        ],
    )?;
    for (i, (key, label)) in steps.iter().enumerate() {
        conn.execute(
            "INSERT INTO task_steps (task_id, idx, key, label) VALUES (?1, ?2, ?3, ?4)",
            params![task.id, i as i64, key, label],
        )?;
    }
    Ok(())
}

pub fn list_tasks(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare("SELECT * FROM tasks ORDER BY created_at DESC")?;
    let rows = stmt.query_map([], task_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get_task(conn: &Connection, id: &str) -> Result<Task> {
    let mut task = conn
        .query_row("SELECT * FROM tasks WHERE id=?1", params![id], task_from_row)
        .optional()?
        .context("업무를 찾을 수 없습니다")?;
    let mut stmt = conn.prepare(
        "SELECT idx, key, label, status, note, started_at, finished_at FROM task_steps WHERE task_id=?1 ORDER BY idx",
    )?;
    task.steps = stmt
        .query_map(params![id], |r| {
            Ok(StepRow {
                idx: r.get(0)?,
                key: r.get(1)?,
                label: r.get(2)?,
                status: r.get(3)?,
                note: r.get(4)?,
                started_at: r.get(5)?,
                finished_at: r.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut stmt = conn.prepare("SELECT at, level, message FROM task_events WHERE task_id=?1 ORDER BY id")?;
    task.events = stmt
        .query_map(params![id], |r| {
            Ok(EventRow { at: r.get(0)?, level: r.get(1)?, message: r.get(2)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(task)
}

pub fn mark_task_seen(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET seen_at=?2 WHERE id=?1 AND seen_at IS NULL AND status=?3",
        params![id, now(), status::COMPLETED],
    )?;
    Ok(())
}

pub fn delete_task(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM tasks WHERE id=?1", params![id])?;
    Ok(())
}

pub fn set_status(conn: &Connection, id: &str, st: &str) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET status=?2, updated_at=?3 WHERE id=?1",
        params![id, st, now()],
    )?;
    Ok(())
}

pub fn set_progress(conn: &Connection, id: &str, progress: i64, step_label: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET progress=?2, current_step=?3, updated_at=?4 WHERE id=?1",
        params![id, progress, step_label, now()],
    )?;
    Ok(())
}

/// 지금 단계를 하는 직원. None이면 비운다.
pub fn set_actor(conn: &Connection, id: &str, actor: Option<&str>) -> Result<()> {
    conn.execute("UPDATE tasks SET current_actor=?2 WHERE id=?1", params![id, actor])?;
    Ok(())
}

pub fn set_gate(conn: &Connection, id: &str, gate: Option<&Gate>) -> Result<()> {
    let json = gate.map(|g| serde_json::to_string(g).unwrap_or_default());
    conn.execute(
        "UPDATE tasks SET pending_gate_json=?2, updated_at=?3 WHERE id=?1",
        params![id, json, now()],
    )?;
    Ok(())
}

pub fn approve_gate(conn: &Connection, id: &str, kind: &str) -> Result<()> {
    let task = get_task(conn, id)?;
    let mut gates = task.approved_gates;
    if !gates.iter().any(|g| g == kind) {
        gates.push(kind.to_string());
    }
    conn.execute(
        "UPDATE tasks SET approved_gates_json=?2, pending_gate_json=NULL, updated_at=?3 WHERE id=?1",
        params![id, serde_json::to_string(&gates)?, now()],
    )?;
    Ok(())
}

pub fn add_cost(conn: &Connection, id: &str, usd: f64) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET cost_usd = cost_usd + ?2, updated_at=?3 WHERE id=?1",
        params![id, usd, now()],
    )?;
    Ok(())
}

pub fn finish(conn: &Connection, id: &str, result: &Value) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET status=?2, result_json=?3, progress=100, current_step=NULL, current_actor=NULL, error=NULL, seen_at=NULL, updated_at=?4 WHERE id=?1",
        params![id, status::COMPLETED, result.to_string(), now()],
    )?;
    Ok(())
}

pub fn fail(conn: &Connection, id: &str, error: &str) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET status=?2, error=?3, updated_at=?4 WHERE id=?1",
        params![id, status::FAILED, error, now()],
    )?;
    Ok(())
}

pub fn set_result(conn: &Connection, id: &str, result: &Value) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET result_json=?2, updated_at=?3 WHERE id=?1",
        params![id, result.to_string(), now()],
    )?;
    Ok(())
}

pub fn add_event(conn: &Connection, id: &str, level: &str, message: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO task_events (task_id, at, level, message) VALUES (?1, ?2, ?3, ?4)",
        params![id, now(), level, message],
    )?;
    Ok(())
}

/// 완료된 단계의 출력 (체크포인트). 이어하기 때 이 값을 다시 읽는다.
pub fn step_outputs(conn: &Connection, id: &str) -> Result<Vec<(String, String, Option<Value>)>> {
    let mut stmt = conn.prepare("SELECT key, status, output_json FROM task_steps WHERE task_id=?1 ORDER BY idx")?;
    let rows = stmt.query_map(params![id], |r| {
        let out: Option<String> = r.get(2)?;
        Ok((r.get(0)?, r.get(1)?, out.and_then(|s| serde_json::from_str(&s).ok())))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn step_started(conn: &Connection, id: &str, key: &str) -> Result<()> {
    conn.execute(
        "UPDATE task_steps SET status='running', started_at=?3, note=NULL WHERE task_id=?1 AND key=?2",
        params![id, key, now()],
    )?;
    Ok(())
}

pub fn step_done(conn: &Connection, id: &str, key: &str, output: &Value, note: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE task_steps SET status='done', output_json=?3, note=?4, finished_at=?5 WHERE task_id=?1 AND key=?2",
        params![id, key, output.to_string(), note, now()],
    )?;
    Ok(())
}

pub fn step_reset(conn: &Connection, id: &str, key: &str, st: &str, note: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE task_steps SET status=?3, note=?4, started_at=NULL WHERE task_id=?1 AND key=?2",
        params![id, key, st, note],
    )?;
    Ok(())
}

pub fn load_settings(conn: &Connection) -> Result<Settings> {
    let json: Option<String> = conn
        .query_row("SELECT value_json FROM settings WHERE key='app'", [], |r| r.get(0))
        .optional()?;
    Ok(json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

pub fn save_settings(conn: &Connection, s: &Settings) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value_json) VALUES ('app', ?1)
         ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json",
        params![serde_json::to_string(s)?],
    )?;
    Ok(())
}

pub fn recent_input(conn: &Connection, employee_id: &str) -> Result<Option<Value>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT input_json FROM recent_inputs WHERE employee_id=?1",
            params![employee_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(json.and_then(|s| serde_json::from_str(&s).ok()))
}

pub fn save_recent_input(conn: &Connection, employee_id: &str, input: &Value) -> Result<()> {
    conn.execute(
        "INSERT INTO recent_inputs (employee_id, input_json) VALUES (?1, ?2)
         ON CONFLICT(employee_id) DO UPDATE SET input_json=excluded.input_json",
        params![employee_id, input.to_string()],
    )?;
    Ok(())
}
