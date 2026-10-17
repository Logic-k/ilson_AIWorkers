mod commands;
mod db;
mod engine;
mod gitops;
mod models;
mod pricing;
mod quota;
mod roster;
mod runner;
mod usage;
mod web;
mod workers;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{Emitter, Manager};

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub data_dir: PathBuf,
    pub http: reqwest::Client,
    /// 실행 중인 업무와 '멈추기' 플래그.
    pub running: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// 마지막으로 확인한 구독 엔진 상태 (엔진 ID → 상태).
    pub engines: Mutex<HashMap<String, engine::EngineStatus>>,
    /// 직원별 지금 하는 일 (사무실 화면).
    pub activities: Mutex<HashMap<String, serde_json::Value>>,
    /// 자동 모드: 한 번에 한 업무만 OpenCode를 실행한다.
    pub engine_lock: tokio::sync::Mutex<()>,
    /// 안전 모드: 사장님이 ‘완료 처리’를 누른 업무.
    pub external_done: Mutex<HashSet<String>>,
    /// 구독 한도 조회 캐시 (10분).
    pub quota: Mutex<HashMap<String, (Instant, serde_json::Value)>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let conn = db::open(&dir)?;
            db::recover_interrupted(&conn)?;
            app.manage(AppState {
                db: Mutex::new(conn),
                data_dir: dir,
                http: reqwest::Client::builder().timeout(std::time::Duration::from_secs(60)).build()?,
                running: Mutex::new(HashMap::new()),
                engines: Mutex::new(HashMap::new()),
                activities: Mutex::new(HashMap::new()),
                engine_lock: tokio::sync::Mutex::new(()),
                external_done: Mutex::new(HashSet::new()),
                quota: Mutex::new(HashMap::new()),
            });
            // 시작할 때 구독 엔진들의 설치·로그인 상태를 한 번 확인해 둔다 (사용량을 쓰지 않는다).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let all = commands::check_engines().await;
                *handle.state::<AppState>().engines.lock().unwrap() = all;
                let _ = handle.emit("engine-updated", ());
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::list_tasks,
            commands::get_task,
            commands::mark_task_seen,
            commands::create_task,
            commands::create_auto_task,
            commands::launch_project,
            commands::approve_task,
            commands::decline_task,
            commands::pause_task,
            commands::resume_task,
            commands::delete_task,
            commands::undo_organize,
            commands::get_recent_input,
            commands::save_settings,
            commands::engine_status,
            commands::open_login_terminal,
            commands::activities,
            commands::task_diff,
            commands::discard_task,
            commands::task_workspace_path,
            commands::open_terminal,
            commands::finish_external,
            commands::acknowledge_guard,
            commands::save_text_file,
            commands::usage_summary,
            commands::usage_for_task,
            commands::estimate_task,
            commands::quota_status,
            commands::refresh_fx,
            commands::task_step_outputs,
            commands::followup_task,
            commands::list_skill_catalog,
            commands::import_skill_from_url,
            commands::hire_employee,
            commands::dismiss_employee,
            commands::save_employee_skill,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
