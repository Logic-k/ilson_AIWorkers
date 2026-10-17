use crate::models::{status, CustomEmployee, Settings, Task};
use crate::{db, engine, gitops, roster, runner, workers, AppState};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// 직원별 지금 상태: ready(바로 사용) / setup_required(설정 필요) / busy(업무 중) / guarded(점검 필요)
type Engines = HashMap<String, engine::EngineStatus>;

fn availability(conn: &rusqlite::Connection, employee: &Value, settings: &Settings, engines: &Engines, tasks: &[Task]) -> Value {
    let id = employee["id"].as_str().unwrap_or("");
    let kind = settings.engine_for(id);
    // 아직 확인하지 않았으면 막지 않는다 (실행할 때 다시 확인된다).
    let engine_ready = engines.get(kind.id()).is_none_or(|e| e.signed_in);
    let engine_need = format!("{} 로그인", kind.label());
    let mut missing: Vec<String> = vec![];
    for req in roster::requirements(id) {
        match *req {
            "opencode" if !engine_ready => missing.push(engine_need.clone()),
            "business" if settings.business_name.trim().is_empty() => missing.push("사업장 상호".into()),
            _ => {}
        }
    }
    if let Ok(Some(error)) = db::guard(conn, id) {
        return json!({ "status": "guarded", "missing": missing, "guard": error });
    }
    if !missing.is_empty() {
        return json!({ "status": "setup_required", "missing": missing });
    }
    let busy = tasks.iter().any(|t| t.employee_id == id && t.status == status::RUNNING);
    json!({ "status": if busy { "busy" } else { "ready" }, "missing": [] })
}

fn find_employee(conn: &rusqlite::Connection, id: &str, safe: bool) -> Option<Value> {
    if let Some(e) = roster::find(id) {
        return Some(e);
    }
    let custom = db::load_custom_employees(conn).unwrap_or_default();
    custom.iter().find(|c| c.id == id).map(|c| roster::custom_employee_json(c, safe))
}

#[tauri::command]
pub fn bootstrap(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Value> {
    let conn = state.db.lock().unwrap();
    let settings = db::load_settings(&conn).map_err(err)?;
    let custom_employees = db::load_custom_employees(&conn).unwrap_or_default();
    let tasks = db::list_tasks(&conn).map_err(err)?;
    let engines = state.engines.lock().unwrap().clone();
    let safe = !settings.auto_mode();
    let mut raw_employees = roster::all(safe);
    for c in &custom_employees {
        raw_employees.push(roster::custom_employee_json(c, safe));
    }
    let employees: Vec<Value> = raw_employees
        .into_iter()
        .map(|mut e| {
            let id = e["id"].as_str().unwrap_or("").to_string();
            e["availability"] = availability(&conn, &e, &settings, &engines, &tasks);
            if let Some(extra_skill) = settings.custom_skills.get(&id) {
                if !extra_skill.trim().is_empty() {
                    e["equippedSkill"] = json!(extra_skill);
                }
            }
            if roster::requirements(&id).contains(&"opencode") {
                let kind = settings.engine_for(&id);
                let model = settings.model_for(kind).unwrap_or_else(|| crate::pricing::default_model_for(kind.id()).to_string());
                let free = crate::pricing::resolve(kind.id(), Some(&model)).map(|p| p.list_cost_zero).unwrap_or(false);
                e["engine"] = json!({
                    "id": kind.id(),
                    "label": kind.label(),
                    "auto": settings.auto_ok(kind) && settings.engine_mode == "auto",
                    "model": model,
                    "free": free,
                });
            }
            e
        })
        .collect();
    let data_dir = app.path().app_data_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    Ok(json!({
        "employees": employees,
        "settings": settings,
        "engines": engine::ALL.iter().map(|k| engines.get(k.id()).cloned().map(|s| json!(s)).unwrap_or_else(|| json!({ "id": k.id(), "label": k.label(), "plan": k.plan(), "canAuto": k.can_auto(), "installed": false, "signedIn": false, "models": [], "message": "확인 중" }))).collect::<Vec<_>>(),
        "levels": engine::ALL.iter().map(|k| json!({
            "engine": k.id(),
            "readOnly": engine::level_display(*k, engine::Level::ReadOnly),
            "edit": engine::level_display(*k, engine::Level::Edit),
            "sandbox": engine::level_display(*k, engine::Level::Sandbox),
            "yolo": engine::level_display(*k, engine::Level::Yolo),
        })).collect::<Vec<_>>(),
        "dataDir": data_dir,
        "version": app.package_info().version.to_string(),
    }))
}

#[tauri::command]
pub fn list_tasks(state: State<'_, AppState>) -> CmdResult<Vec<Task>> {
    let conn = state.db.lock().unwrap();
    db::list_tasks(&conn).map_err(err)
}

#[tauri::command]
pub fn get_task(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<Task> {
    let conn = state.db.lock().unwrap();
    let mut task = db::get_task(&conn, &id).map_err(err)?;
    if task.status == status::COMPLETED && task.seen_at.is_none() {
        let _ = db::mark_task_seen(&conn, &id);
        task.seen_at = Some(db::now());
        drop(conn);
        runner::emit(&app, &id);
    }
    Ok(task)
}

#[tauri::command]
pub fn mark_task_seen(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let conn = state.db.lock().unwrap();
    db::mark_task_seen(&conn, &id).map_err(err)?;
    drop(conn);
    runner::emit(&app, &id);
    Ok(())
}

#[tauri::command]
pub fn create_task(app: AppHandle, state: State<'_, AppState>, employee_id: String, title: String, input: Value) -> CmdResult<Task> {
    let task = {
        let conn = state.db.lock().unwrap();
        let settings = db::load_settings(&conn).map_err(err)?;
        let safe = !settings.auto_mode();
        let employee = find_employee(&conn, &employee_id, safe).ok_or("알 수 없는 직원입니다.")?;
        let problems = roster::validate(&employee, &input);
        if !problems.is_empty() {
            return Err(problems.join("\n"));
        }
        let tasks = db::list_tasks(&conn).map_err(err)?;
        let engines = state.engines.lock().unwrap().clone();
        let avail = availability(&conn, &employee, &settings, &engines, &tasks);
        match avail["status"].as_str() {
            Some("guarded") => {
                return Err(format!(
                    "같은 문제로 업무가 두 번 실패해 잠시 멈춰 두었습니다. 원인을 확인한 뒤 ‘조치했어요’를 눌러 주세요.\n최근 오류: {}",
                    avail["guard"].as_str().unwrap_or("")
                ))
            }
            Some("setup_required") => {
                let missing: Vec<&str> = avail["missing"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                return Err(format!("먼저 준비해 주세요: {}. 연결 및 설정에서 입력할 수 있습니다.", missing.join(", ")));
            }
            _ => {}
        }
        let title = if title.trim().is_empty() { roster::default_title(&employee, &input) } else { title.trim().to_string() };
        let now = db::now();
        let task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            employee_id: employee_id.clone(),
            title,
            status: status::QUEUED.into(),
            input: {
                let mut i = input.clone();
                i["_engine"] = json!(if settings.auto_mode() { "auto" } else { "safe" });
                i
            },
            progress: 0,
            current_step: None,
            current_actor: None,
            pending_gate: None,
            approved_gates: vec![],
            cost_usd: 0.0,
            result: None,
            error: None,
            created_at: now.clone(),
            updated_at: now,
            seen_at: None,
            steps: vec![],
            events: vec![],
        };
        let steps: Vec<(String, String)> = roster::steps(&employee_id, safe)
            .iter()
            .map(|s| (s.key.to_string(), s.label.to_string()))
            .collect();
        db::insert_task(&conn, &task, &steps).map_err(err)?;
        db::add_event(&conn, &task.id, "info", &format!("{}에게 업무를 맡겼습니다.", employee["name"].as_str().unwrap_or(""))).map_err(err)?;
        db::save_recent_input(&conn, &employee_id, &input).map_err(err)?;
        task
    };
    runner::emit(&app, &task.id);
    runner::spawn(app, task.id.clone());
    Ok(task)
}

#[tauri::command]
pub fn approve_task(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();
        let task = db::get_task(&conn, &id).map_err(err)?;
        let gate = task.pending_gate.filter(|_| task.status == status::AWAITING_APPROVAL).ok_or("확인을 기다리는 업무가 아닙니다.")?;
        let key = if gate.key.is_empty() { gate.kind.clone() } else { gate.key.clone() };
        db::approve_gate(&conn, &id, &key).map_err(err)?;
        db::set_status(&conn, &id, status::QUEUED).map_err(err)?;
        db::add_event(&conn, &id, "info", &format!("사장님이 확인했습니다. ({})", gate.summary)).map_err(err)?;
    }
    runner::spawn(app, id);
    Ok(())
}

#[tauri::command]
pub fn decline_task(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let conn = state.db.lock().unwrap();
    let task = db::get_task(&conn, &id).map_err(err)?;
    if task.status != status::AWAITING_APPROVAL {
        return Err("확인을 기다리는 업무가 아닙니다.".into());
    }
    db::set_gate(&conn, &id, None).map_err(err)?;
    db::set_status(&conn, &id, status::CANCELLED).map_err(err)?;
    db::add_event(&conn, &id, "warn", "사장님이 이번 단계를 진행하지 않기로 했습니다. 코딩 업무라면 작업 브랜치는 남아 있어 직접 살펴보거나 ‘작업 공간 지우기’로 정리할 수 있습니다.").map_err(err)?;
    drop(conn);
    runner::emit(&app, &id);
    Ok(())
}

#[tauri::command]
pub fn pause_task(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    if let Some(flag) = state.running.lock().unwrap().get(&id) {
        flag.store(true, Ordering::SeqCst);
        return Ok(());
    }
    let conn = state.db.lock().unwrap();
    let task = db::get_task(&conn, &id).map_err(err)?;
    if task.status == status::QUEUED || task.status == status::AWAITING_APPROVAL {
        db::set_gate(&conn, &id, None).map_err(err)?;
        db::set_status(&conn, &id, status::PAUSED).map_err(err)?;
        db::add_event(&conn, &id, "warn", "사장님 요청으로 멈췄습니다.").map_err(err)?;
    }
    drop(conn);
    runner::emit(&app, &id);
    Ok(())
}

#[tauri::command]
pub fn resume_task(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();
        let task = db::get_task(&conn, &id).map_err(err)?;
        if ![status::PAUSED, status::FAILED, status::CANCELLED].contains(&task.status.as_str()) {
            return Err("멈춘 업무만 이어서 할 수 있습니다.".into());
        }
        // 끝난 단계의 확인만 남긴다. 다시 할 유료 단계는 비용을 새로 확인받는다.
        let done: Vec<String> = task.steps.iter().filter(|s| s.status == "done").map(|s| s.key.clone()).collect();
        let keep: Vec<String> = task
            .approved_gates
            .iter()
            .filter(|g| g.split_once(':').is_some_and(|(_, step)| done.iter().any(|d| d == step)))
            .cloned()
            .collect();
        db::reset_pending_gates(&conn, &id, &keep).map_err(err)?;
        db::set_status(&conn, &id, status::QUEUED).map_err(err)?;
        db::add_event(&conn, &id, "info", "이어하기를 요청했습니다.").map_err(err)?;
    }
    runner::spawn(app, id);
    Ok(())
}

#[tauri::command]
pub fn delete_task(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    if state.running.lock().unwrap().contains_key(&id) {
        return Err("진행 중인 업무는 먼저 멈춰 주세요.".into());
    }
    let conn = state.db.lock().unwrap();
    db::delete_task(&conn, &id).map_err(err)?;
    drop(conn);
    runner::emit(&app, &id);
    Ok(())
}

#[tauri::command]
pub fn undo_organize(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<String> {
    let conn = state.db.lock().unwrap();
    let task = db::get_task(&conn, &id).map_err(err)?;
    let mut result = task.result.filter(|r| r["kind"] == "organize").ok_or("되돌릴 정리 기록이 없습니다.")?;
    let (back, skipped) = workers::undo_organize(&mut result).map_err(err)?;
    db::set_result(&conn, &id, &result).map_err(err)?;
    let msg = if skipped == 0 {
        format!("파일 {back}개를 원래 자리로 되돌렸습니다.")
    } else {
        format!("파일 {back}개를 되돌리고, 그사이 바뀐 {skipped}개는 그대로 두었습니다.")
    };
    db::add_event(&conn, &id, "info", &msg).map_err(err)?;
    drop(conn);
    runner::emit(&app, &id);
    Ok(msg)
}

#[tauri::command]
pub fn get_recent_input(state: State<'_, AppState>, employee_id: String) -> CmdResult<Option<Value>> {
    let conn = state.db.lock().unwrap();
    db::recent_input(&conn, &employee_id).map_err(err)
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> CmdResult<()> {
    let conn = state.db.lock().unwrap();
    db::save_settings(&conn, &settings).map_err(err)
}

/// 모든 구독 엔진의 설치·로그인 상태를 함께 확인한다 (사용량을 쓰지 않는다).
pub async fn check_engines() -> Engines {
    let mut map = HashMap::new();
    for k in engine::ALL {
        let st = engine::status(k).await;
        map.insert(st.id.clone(), st);
    }
    map
}

#[tauri::command]
pub async fn engine_status(app: AppHandle) -> CmdResult<Vec<engine::EngineStatus>> {
    let all = check_engines().await;
    *app.state::<AppState>().engines.lock().unwrap() = all.clone();
    Ok(engine::ALL.iter().filter_map(|k| all.get(k.id()).cloned()).collect())
}

#[tauri::command]
pub fn open_login_terminal(engine: String) -> CmdResult<()> {
    engine::open_login_terminal(engine::Kind::from_id(&engine)).map_err(err)
}

/// 안전 모드: 작업 폴더에서 새 터미널을 연다.
#[tauri::command]
pub fn open_terminal(folder: String) -> CmdResult<()> {
    engine::open_terminal(&folder).map_err(err)
}

/// 지금 직원들이 하고 있는 일 (사무실 화면 첫 그림용).
#[tauri::command]
pub fn activities(state: State<'_, AppState>) -> Vec<Value> {
    state.activities.lock().unwrap().values().cloned().collect()
}

fn task_workspace(conn: &rusqlite::Connection, id: &str) -> Result<gitops::Workspace, String> {
    let setup = db::step_outputs(conn, id)
        .map_err(err)?
        .into_iter()
        .find(|(k, _, _)| k == "setup")
        .and_then(|(_, _, out)| out)
        .ok_or("작업 공간이 없는 업무입니다.")?;
    serde_json::from_value(setup["workspace"].clone()).map_err(|_| "작업 공간 정보를 읽지 못했습니다.".to_string())
}

/// 병합 전 변경 내용(diff).
#[tauri::command]
pub fn task_diff(state: State<'_, AppState>, id: String) -> CmdResult<gitops::DiffSummary> {
    let conn = state.db.lock().unwrap();
    let ws = task_workspace(&conn, &id)?;
    drop(conn);
    if !std::path::Path::new(&ws.path).exists() {
        return Err("작업 공간이 이미 정리되었습니다.".into());
    }
    gitops::diff(&ws).map_err(err)
}

/// 병합하지 않은 작업 공간과 작업 브랜치를 지운다.
#[tauri::command]
pub fn discard_task(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    if state.running.lock().unwrap().contains_key(&id) {
        return Err("진행 중인 업무는 먼저 멈춰 주세요.".into());
    }
    let conn = state.db.lock().unwrap();
    let ws = task_workspace(&conn, &id)?;
    gitops::cleanup(&ws, true);
    db::set_gate(&conn, &id, None).map_err(err)?;
    db::set_status(&conn, &id, status::CANCELLED).map_err(err)?;
    db::add_event(&conn, &id, "warn", &format!("작업 공간과 브랜치 {}를 지웠습니다. 원래 저장소는 바뀌지 않았습니다.", ws.branch)).map_err(err)?;
    drop(conn);
    runner::emit(&app, &id);
    Ok(())
}


/// 안전 모드: 보고서 파일이 없어도 이 단계를 끝난 것으로 처리한다.
#[tauri::command]
pub fn finish_external(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    if !state.running.lock().unwrap().contains_key(&id) {
        return Err("지켜보는 중인 업무가 아닙니다.".into());
    }
    state.external_done.lock().unwrap().insert(id);
    Ok(())
}

/// 작업 공간 폴더 경로 (탐색기·IDE로 열기용). 병합 후 worktree가 정리되었으면 실제 프로젝트 폴더(repo)를 돌려준다.
#[tauri::command]
pub fn task_workspace_path(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    let conn = state.db.lock().unwrap();
    let ws = task_workspace(&conn, &id)?;
    if std::path::Path::new(&ws.path).exists() {
        Ok(ws.path)
    } else {
        Ok(ws.repo)
    }
}

/// 한줄 자동업무: 한 줄 지시를 분석해 가장 알맞은 직원·팀 파이프라인으로 자동 착수한다.
#[tauri::command]
pub fn create_auto_task(
    app: AppHandle,
    state: State<'_, AppState>,
    prompt: String,
    folder: Option<String>,
    force_program: Option<bool>,
) -> CmdResult<Task> {
    if prompt.trim().is_empty() {
        return Err("만들고 싶은 프로그램이나 시키실 업무를 한 줄로 입력해 주세요.".into());
    }
    let (employee_id, title, input) = roster::route_one_line(
        prompt.trim(),
        folder.as_deref(),
        force_program.unwrap_or(false),
    );
    {
        let conn = state.db.lock().unwrap();
        if let Ok(mut settings) = db::load_settings(&conn) {
            let mut changed = false;
            if settings.engine_mode != "auto" {
                settings.engine_mode = "auto".into();
                changed = true;
            }
            if !settings.auto_acks.get("opencode").is_some_and(|s| !s.trim().is_empty()) {
                settings.auto_acks.insert("opencode".into(), "자동 실행 동의".into());
                changed = true;
            }
            if changed {
                let _ = db::save_settings(&conn, &settings);
            }
        }
    }
    create_task(app, state, employee_id, title, input)
}

/// 완성된 프로그램(index.html 등)이나 프로젝트 폴더를 즉시 실행한다.
#[tauri::command]
pub fn launch_project(path: String) -> CmdResult<()> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(format!("실행할 파일이나 폴더를 찾을 수 없습니다: {path}"));
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path])
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|e| format!("프로그램을 실행하지 못했습니다: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("프로그램을 실행하지 못했습니다: {e}"))?;
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("프로그램을 실행하지 못했습니다: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn acknowledge_guard(state: State<'_, AppState>, employee_id: String) -> CmdResult<()> {
    let conn = state.db.lock().unwrap();
    db::clear_guard(&conn, &employee_id).map_err(err)
}

#[tauri::command]
pub fn save_text_file(path: String, contents: String) -> CmdResult<()> {
    std::fs::write(&path, contents).map_err(|e| format!("파일을 저장하지 못했습니다: {e}"))
}

#[tauri::command]
pub fn usage_summary(state: State<'_, AppState>, range: Option<String>) -> CmdResult<crate::usage::UsageSummary> {
    let conn = state.db.lock().unwrap();
    let settings = db::load_settings(&conn).map_err(err)?;
    crate::usage::summary(&conn, &settings, range.as_deref().unwrap_or("30d")).map_err(err)
}

#[tauri::command]
pub fn usage_for_task(state: State<'_, AppState>, id: String) -> CmdResult<Vec<crate::usage::UsageRow>> {
    let conn = state.db.lock().unwrap();
    let settings = db::load_settings(&conn).map_err(err)?;
    crate::usage::for_task(&conn, &id, settings.effective_usd_krw()).map_err(err)
}

#[tauri::command]
pub fn estimate_task(state: State<'_, AppState>, employee_id: String, input: Value) -> CmdResult<crate::usage::Estimate> {
    let conn = state.db.lock().unwrap();
    let settings = db::load_settings(&conn).map_err(err)?;
    crate::usage::estimate(&conn, &settings, &employee_id, &input).map_err(err)
}

#[tauri::command]
pub async fn quota_status(state: State<'_, AppState>, refresh: Option<bool>) -> CmdResult<Vec<crate::quota::EngineQuota>> {
    crate::quota::check_all(&state, refresh.unwrap_or(false)).await.map_err(err)
}

#[tauri::command]
pub async fn refresh_fx(state: State<'_, AppState>) -> CmdResult<Settings> {
    let mut settings = {
        let conn = state.db.lock().unwrap();
        db::load_settings(&conn).map_err(err)?
    };
    if let Ok(resp) = state
        .http
        .get("https://open.er-api.com/v6/latest/USD")
        .send()
        .await
    {
        if let Ok(json) = resp.json::<Value>().await {
            if let Some(krw) = json["rates"]["KRW"].as_f64().filter(|&v| v > 500.0 && v < 5000.0) {
                settings.usd_krw = (krw * 100.0).round() / 100.0;
                settings.fx_updated_at = Some(db::now());
                let conn = state.db.lock().unwrap();
                let _ = db::save_settings(&conn, &settings);
            }
        }
    }
    Ok(settings)
}

/// 진행 중이거나 완료된 업무의 단계별 산출물(리서치·기획서·개발보고 등)을 가져온다.
#[tauri::command]
pub fn task_step_outputs(state: State<'_, AppState>, id: String) -> CmdResult<HashMap<String, Value>> {
    let conn = state.db.lock().unwrap();
    let rows = db::step_outputs(&conn, &id).map_err(err)?;
    let mut map = HashMap::new();
    for (k, st, out) in rows {
        if st == "done" {
            if let Some(v) = out {
                map.insert(k, v);
            }
        }
    }
    Ok(map)
}

/// 완료·중단된 업무 세션에서 바로 추가 업무 지시를 내려 같은 폴더·맥락에서 이어서 작업한다.
#[tauri::command]
pub fn followup_task(app: AppHandle, state: State<'_, AppState>, id: String, prompt: String) -> CmdResult<Task> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Err("추가로 지시할 업무 내용을 입력해 주세요.".into());
    }
    if state.running.lock().unwrap().contains_key(&id) {
        return Err("현재 실행 중인 업무입니다. 끝나거나 멈춘 뒤에 추가 지시를 내려 주세요.".into());
    }
    let updated_task = {
        let conn = state.db.lock().unwrap();
        let task = db::get_task(&conn, &id).map_err(err)?;
        let mut input = task.input.clone();
        let prev_count = input["_followupCount"].as_u64().unwrap_or(0);
        let next_count = prev_count + 1;

        // 원래 제목을 보존하고 차수 + 요약 제목을 자동으로 붙인다.
        let base_title = input["_baseTitle"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(String::from)
            .unwrap_or_else(|| task.title.clone());
        input["_baseTitle"] = json!(base_title);
        input["_followupCount"] = json!(next_count);
        input["_followupLatest"] = json!(trimmed);

        let mut history = input["_followupHistory"].as_array().cloned().unwrap_or_default();
        history.push(json!({
            "round": next_count,
            "prompt": trimmed,
            "at": db::now(),
        }));
        input["_followupHistory"] = json!(history);

        // 코딩 업무는 이전에 병합된 실제 프로젝트 폴더(repo)를 그대로 이어받아 작업한다.
        if roster::is_dev_id(&task.employee_id) {
            let prev_repo = task
                .result
                .as_ref()
                .and_then(|r| r["workspace"]["repo"].as_str())
                .map(String::from)
                .or_else(|| {
                    db::step_outputs(&conn, &id)
                        .ok()?
                        .into_iter()
                        .find(|(k, _, _)| k == "setup")
                        .and_then(|(_, _, out)| out?["workspace"]["repo"].as_str().map(String::from))
                });
            if let Some(repo) = prev_repo {
                if !repo.trim().is_empty() {
                    input["folder"] = json!(repo);
                    input["autoMerge"] = json!(true);
                }
            }
            let prev_req = input["request"].as_str().unwrap_or("").trim().to_string();
            input["request"] = json!(if prev_req.is_empty() {
                trimmed.to_string()
            } else {
                format!("{prev_req}\n\n[{next_count}차 추가 지시] {trimmed}")
            });
        } else {
            for field in ["facts", "focus", "context", "reference", "extra"] {
                if input.get(field).is_some() {
                    let prev = input[field].as_str().unwrap_or("").trim().to_string();
                    input[field] = json!(if prev.is_empty() {
                        format!("[추가 지시] {trimmed}")
                    } else {
                        format!("{prev}\n[추가 지시] {trimmed}")
                    });
                    break;
                }
            }
        }

        let safe = input["_engine"] != "auto";
        let (short_title, reset_keys, skip_keys, assigned_summary) =
            roster::route_followup(&task.employee_id, trimmed, safe);
        let new_title = format!("{base_title} · {}차 ({short_title})", next_count + 1);

        db::prepare_followup(&conn, &id, &new_title, &input, &reset_keys, &skip_keys).map_err(err)?;
        db::add_event(
            &conn,
            &id,
            "info",
            &format!(
                "⚡ [추가 업무 지시 · {}차] “{trimmed}” → OpenCode 자동 배정: {assigned_summary}",
                next_count + 1
            ),
        )
        .map_err(err)?;
        db::get_task(&conn, &id).map_err(err)?
    };
    runner::emit(&app, &id);
    runner::spawn(app, id);
    Ok(updated_task)
}

#[tauri::command]
pub fn list_skill_catalog() -> CmdResult<Vec<Value>> {
    Ok(roster::builtin_skill_catalog())
}

fn to_raw_github_url(url: &str) -> String {
    let trimmed = url.trim();
    if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 5 && parts[2] == "blob" {
            let owner = parts[0];
            let repo = parts[1];
            let branch = parts[3];
            let path = parts[4..].join("/");
            return format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}");
        }
    }
    trimmed.to_string()
}

fn parse_skill_markdown(md: &str, url: &str) -> Value {
    let mut name = String::new();
    let mut description = String::new();
    let mut body = md.trim().to_string();

    if let Some(after_open) = md.trim_start().strip_prefix("---") {
        if let Some(end_idx) = after_open.find("\n---") {
            let fm = &after_open[..end_idx];
            body = after_open[end_idx + 4..].trim().to_string();
            for line in fm.lines() {
                let l = line.trim();
                if let Some(v) = l.strip_prefix("name:") {
                    name = v.trim().trim_matches(['"', '\'']).to_string();
                } else if let Some(v) = l.strip_prefix("description:") {
                    description = v.trim().trim_matches(['"', '\'']).to_string();
                }
            }
        }
    }

    if name.is_empty() {
        if let Some(h1) = body.lines().find(|l| l.starts_with("# ")) {
            name = h1[2..].trim().to_string();
        }
    }
    if description.is_empty() {
        if let Some(p) = body
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('-') && !l.starts_with('>'))
        {
            description = p.chars().take(120).collect();
        }
    }
    let lower = format!("{name} {description} {body}").to_lowercase();
    let is_dev = ["code", "react", "typescript", "rust", "python", "api", "frontend", "backend", "ui", "ux", "security", "test", "개발", "코드", "서버", "화면"]
        .iter()
        .any(|k| lower.contains(k));

    let role = if !name.is_empty() {
        name.chars().take(24).collect()
    } else {
        "전문 스킬 담당".to_string()
    };
    let summary = if !description.is_empty() {
        description
    } else {
        "외부 스킬 문서(SKILL.md)를 기반으로 전문 업무를 수행합니다.".to_string()
    };

    json!({
        "role": role,
        "summary": summary,
        "floor": if is_dev { "dev" } else { "office" },
        "team": if is_dev { "개발팀" } else { "전문지원팀" },
        "skillMarkdown": if body.is_empty() { md.trim().to_string() } else { body },
        "sourceUrl": url.trim(),
    })
}

/// GitHub URL(SKILL.md 또는 마크다운 주소)에서 스킬 내용을 읽어 직원 채용 초안으로 파싱한다.
#[tauri::command]
pub async fn import_skill_from_url(state: State<'_, AppState>, url: String) -> CmdResult<Value> {
    let raw_url = to_raw_github_url(&url);
    if !raw_url.starts_with("http://") && !raw_url.starts_with("https://") {
        return Err("올바른 http:// 또는 https:// 주소를 입력해 주세요.".into());
    }
    let resp = state
        .http
        .get(&raw_url)
        .header("User-Agent", "Ilson-Desktop/0.1")
        .send()
        .await
        .map_err(|e| format!("주소에서 스킬 문서를 가져오지 못했습니다: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("주소 응답 오류 (HTTP {})", resp.status()));
    }
    let text = resp.text().await.map_err(|e| format!("본문을 읽지 못했습니다: {e}"))?;
    if text.trim().is_empty() {
        return Err("가져온 문서 내용이 비어 있습니다.".into());
    }
    let clipped: String = text.chars().take(12_000).collect();
    Ok(parse_skill_markdown(&clipped, &url))
}

/// 새로운 AI 직원을 추가 고용한다.
#[tauri::command]
pub fn hire_employee(app: AppHandle, state: State<'_, AppState>, mut employee: CustomEmployee) -> CmdResult<Value> {
    if employee.name.trim().is_empty() {
        return Err("직원 이름을 입력해 주세요.".into());
    }
    if employee.role.trim().is_empty() {
        return Err("담당 역할(직무)을 입력해 주세요.".into());
    }
    let prefix = if employee.floor == "dev" { "custom_dev_" } else { "custom_office_" };
    if !employee.id.starts_with(prefix) {
        let short = &uuid::Uuid::new_v4().simple().to_string()[..6];
        employee.id = format!("{prefix}{short}");
    }
    if employee.team.trim().is_empty() {
        employee.team = if employee.floor == "dev" { "개발팀".into() } else { "전문지원팀".into() };
    }
    if employee.summary.trim().is_empty() {
        employee.summary = format!("{} 전문 스킬을 바탕으로 업무를 수행합니다.", employee.role);
    }
    if employee.motto.trim().is_empty() {
        employee.motto = "장착된 전문 스킬로 빈틈없이 완수합니다.".into();
    }

    let emp_json = {
        let conn = state.db.lock().unwrap();
        let settings = db::load_settings(&conn).map_err(err)?;
        let mut list = db::load_custom_employees(&conn).unwrap_or_default();
        if let Some(existing) = list.iter_mut().find(|e| e.id == employee.id) {
            *existing = employee.clone();
        } else {
            list.push(employee.clone());
        }
        db::save_custom_employees(&conn, &list).map_err(err)?;
        roster::custom_employee_json(&employee, !settings.auto_mode())
    };
    let _ = app.emit("engine-updated", ());
    Ok(emp_json)
}

/// 추가 고용한 AI 직원을 해고(목록에서 제거)한다.
#[tauri::command]
pub fn dismiss_employee(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    if !id.starts_with("custom_") {
        return Err("기본 소속 직원은 해고할 수 없습니다.".into());
    }
    {
        let conn = state.db.lock().unwrap();
        let mut list = db::load_custom_employees(&conn).unwrap_or_default();
        list.retain(|e| e.id != id);
        db::save_custom_employees(&conn, &list).map_err(err)?;
    }
    let _ = app.emit("engine-updated", ());
    Ok(())
}

/// 기존 직원 또는 특채 직원에게 추가 전문 스킬(SKILL.md)을 장착하거나 해제한다.
#[tauri::command]
pub fn save_employee_skill(
    app: AppHandle,
    state: State<'_, AppState>,
    employee_id: String,
    skill_markdown: String,
) -> CmdResult<Settings> {
    let settings = {
        let conn = state.db.lock().unwrap();
        let mut s = db::load_settings(&conn).map_err(err)?;
        let trimmed = skill_markdown.trim().to_string();
        if trimmed.is_empty() {
            s.custom_skills.remove(&employee_id);
        } else {
            s.custom_skills.insert(employee_id.clone(), trimmed.clone());
        }
        db::save_settings(&conn, &s).map_err(err)?;
        if employee_id.starts_with("custom_") {
            let mut list = db::load_custom_employees(&conn).unwrap_or_default();
            if let Some(c) = list.iter_mut().find(|e| e.id == employee_id) {
                if !trimmed.is_empty() {
                    c.skill_markdown = trimmed;
                    let _ = db::save_custom_employees(&conn, &list);
                }
            }
        }
        s
    };
    let _ = app.emit("engine-updated", ());
    Ok(settings)
}

