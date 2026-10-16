//! 업무 실행기. 모든 직원이 같은 계약을 따른다:
//! 단계마다 결과를 저장(체크포인트)하고, 확인이 필요한 단계 앞에서는 멈춰 사장님을 기다린다.

use crate::models::status;
use crate::workers;
use crate::{db, roster, AppState};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub fn emit(app: &AppHandle, task_id: &str) {
    let _ = app.emit("task-updated", task_id);
}

pub fn spawn(app: AppHandle, task_id: String) {
    tauri::async_runtime::spawn(async move { run(app, task_id).await });
}

async fn run(app: AppHandle, task_id: String) {
    let state = app.state::<AppState>();
    let flag = {
        let mut running = state.running.lock().unwrap();
        if running.contains_key(&task_id) {
            return;
        }
        let flag = Arc::new(AtomicBool::new(false));
        running.insert(task_id.clone(), flag.clone());
        flag
    };
    let outcome = run_inner(&app, &task_id, &flag).await;
    state.running.lock().unwrap().remove(&task_id);
    state.activities.lock().unwrap().retain(|_, v| v["taskId"] != task_id.as_str());
    if let Err(e) = outcome {
        let msg = e.to_string();
        let conn = state.db.lock().unwrap();
        let _ = db::fail(&conn, &task_id, &msg);
        let _ = db::add_event(&conn, &task_id, "error", &msg);
        if let Ok(task) = db::get_task(&conn, &task_id) {
            let _ = db::check_guard(&conn, &task.employee_id);
        }
    }
    emit(&app, &task_id);
}

fn pause(app: &AppHandle, task_id: &str, message: &str) -> Result<()> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    db::set_status(&conn, task_id, status::PAUSED)?;
    db::add_event(&conn, task_id, "warn", message)?;
    Ok(())
}

async fn run_inner(app: &AppHandle, task_id: &str, flag: &Arc<AtomicBool>) -> Result<()> {
    let state = app.state::<AppState>();
    let (task, settings, saved) = {
        let conn = state.db.lock().unwrap();
        let task = db::get_task(&conn, task_id)?;
        let settings = db::load_settings(&conn)?;
        let saved = db::step_outputs(&conn, task_id)?;
        let resumed = saved.iter().any(|(_, st, _)| st == "done");
        db::set_status(&conn, task_id, status::RUNNING)?;
        db::add_event(
            &conn,
            task_id,
            "info",
            if resumed { "마지막으로 끝낸 단계 다음부터 이어서 진행합니다." } else { "업무를 시작합니다." },
        )?;
        (task, settings, saved)
    };
    emit(app, task_id);

    let mut ctx = workers::Ctx {
        app: app.clone(),
        task_id: task_id.to_string(),
        data_dir: app.path().app_data_dir()?,
        employee_id: task.employee_id.clone(),
        title: task.title.clone(),
        input: task.input.clone(),
        outputs: HashMap::new(),
        settings: settings.clone(),
        http: state.http.clone(),
    };
    for (key, st, out) in saved {
        if st == "done" {
            ctx.outputs.insert(key, out.unwrap_or(Value::Null));
        }
    }

    let safe = task.input["_engine"] != "auto";
    let steps = roster::steps(&task.employee_id, safe);
    let total = steps.len().max(1) as i64;
    for (i, step) in steps.iter().enumerate() {
        let i = i as i64;
        if ctx.outputs.contains_key(step.key) {
            continue;
        }
        if flag.load(Ordering::SeqCst) {
            pause(app, task_id, "사장님 요청으로 멈췄습니다. 이어하기를 누르면 이 단계부터 다시 합니다.")?;
            return Ok(());
        }

        if let Some(gate_key) = workers::gate_kind(&ctx, step.key, step.gate)? {
            if !task.approved_gates.iter().any(|g| g == &gate_key) {
                if let Some(gate) = workers::gate(&ctx, step.key, &gate_key)? {
                    let conn = state.db.lock().unwrap();
                    db::set_gate(&conn, task_id, Some(&gate))?;
                    db::set_status(&conn, task_id, status::AWAITING_APPROVAL)?;
                    db::set_actor(&conn, task_id, Some(&task.employee_id))?;
                    db::set_progress(&conn, task_id, i * 100 / total, Some("사장님 차례"))?;
                    db::add_event(&conn, task_id, "warn", &format!("{} {}", gate.title, gate.summary))?;
                    drop(conn);
                    emit(app, task_id);
                    return Ok(());
                }
            }
        }

        if let Ok(workers::Plan::Ai(job)) = workers::plan(&ctx, step.key) {
            if !workers::handoff_for(&ctx, &job.actor) {
                let decision = {
                    let conn = state.db.lock().unwrap();
                    let est = crate::usage::estimate(&conn, &ctx.settings, &job.actor, &ctx.input).ok();
                    let (est_tok, est_krw) = est
                        .map(|e| (e.est_total, e.est_krw))
                        .unwrap_or_else(|| {
                            let inp = crate::usage::estimate_prompt_tokens(job.engine, &job.prompt);
                            (inp + 3000, 0)
                        });
                    crate::usage::check_budget_before_step(&conn, &ctx.settings, job.engine, est_tok, est_krw)?
                };
                match decision {
                    crate::usage::BudgetDecision::Pause(msg) => {
                        pause(app, task_id, &msg)?;
                        return Ok(());
                    }
                    crate::usage::BudgetDecision::Warn(msg) => {
                        let conn = state.db.lock().unwrap();
                        let _ = db::add_event(&conn, task_id, "warn", &msg);
                    }
                    crate::usage::BudgetDecision::Ok => {}
                }
            }
        }

        {
            let conn = state.db.lock().unwrap();
            db::step_started(&conn, task_id, step.key)?;
            db::set_actor(&conn, task_id, Some(&roster::actor_of(&task.employee_id, step.key, safe)))?;
            db::set_progress(&conn, task_id, (i * 100 / total).max(3), Some(step.label))?;
        }
        emit(app, task_id);

        // 단계 도중에도 '멈추기'를 받을 수 있게 플래그를 함께 살핀다.
        let outcome = {
            let exec = workers::execute(&ctx, step.key);
            tokio::pin!(exec);
            loop {
                tokio::select! {
                    r = &mut exec => break Some(r),
                    _ = tokio::time::sleep(Duration::from_millis(400)) => {
                        if flag.load(Ordering::SeqCst) { break None; }
                    }
                }
            }
        };
        let Some(outcome) = outcome else {
            {
                let conn = state.db.lock().unwrap();
                db::step_reset(&conn, task_id, step.key, "pending", Some("중간에 멈춤"))?;
            }
            pause(
                app,
                task_id,
                "사장님 요청으로 멈췄습니다. 진행 중이던 AI 요청이 있었다면 비용이 청구될 수 있습니다. 이어하기를 누르면 이 단계부터 다시 합니다.",
            )?;
            return Ok(());
        };

        match outcome {
            Ok(r) => {
                let conn = state.db.lock().unwrap();
                db::step_done(&conn, task_id, step.key, &r.output, r.note.as_deref())?;
                if r.cost_usd > 0.0 {
                    db::add_cost(&conn, task_id, r.cost_usd)?;
                }
                if let Some(note) = &r.note {
                    db::add_event(&conn, task_id, "info", &format!("{} · {}", step.label, note))?;
                }
                db::set_progress(&conn, task_id, (i + 1) * 100 / total, Some(step.label))?;
                ctx.outputs.insert(step.key.to_string(), r.output);
            }
            Err(e) => {
                let conn = state.db.lock().unwrap();
                db::step_reset(&conn, task_id, step.key, "failed", Some(&e.to_string()))?;
                return Err(e);
            }
        }
        emit(app, task_id);
    }

    let result = ctx.outputs.get("deliver").cloned().unwrap_or(Value::Null);
    let conn = state.db.lock().unwrap();
    db::finish(&conn, task_id, &result)?;
    db::add_event(&conn, task_id, "success", "업무를 마쳤습니다. 결과를 확인해 주세요.")?;
    db::clear_guard(&conn, &task.employee_id)?;
    Ok(())
}
