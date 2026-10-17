//! AI 엔진: 사용자 PC에 설치된 오픈소스 OpenCode CLI를 실행한다. API 키는 절대 쓰지 않는다.
//!
//! - OpenCode (`opencode`) — DeepSeek 무료 모델 (기본값) · OpenCode Console/Go
//!
//! 자식 프로세스 환경에서 API 키·클라우드 과금 관련 변수를 지우고 --pure 격리 실행을 통해
//! 안전하고 깨끗한 환경에서 자동화를 수행한다.

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub const OPENCODE_DEFAULT_MODEL: &str = "opencode/deepseek-v4.1-flash-free";

const STRIPPED_ENV: &[&str] = &[
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GOOGLE_GENAI_API_KEY",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_CLOUD_PROJECT",
    "GOOGLE_CLOUD_LOCATION",
    "GOOGLE_CLOUD_QUOTA_PROJECT",
    "GOOGLE_GENAI_USE_VERTEXAI",
    "GOOGLE_GEMINI_BASE_URL",
    "GOOGLE_VERTEX_BASE_URL",
    "AGY_ADC_AUTH",
    "AGY_ACCOUNT",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "CODEX_API_KEY",
    "AZURE_OPENAI_API_KEY",
    "OPENCODE_API_KEY",
    "OPENCODE_AUTH_CONTENT",
    "OPENCODE_CONFIG_CONTENT",
    "OPENCODE_CONFIG",
    "DEEPSEEK_API_KEY",
    "OPENROUTER_API_KEY",
    "GROQ_API_KEY",
    "NVIDIA_API_KEY",
    "XAI_API_KEY",
    "MISTRAL_API_KEY",
    "ZHIPU_API_KEY",
    "MOONSHOT_API_KEY",
    "KIMI_API_KEY",
    "MINIMAX_API_KEY",
    "GOOGLE_GENERATIVE_AI_API_KEY",
    "TOGETHER_API_KEY",
    "FIREWORKS_API_KEY",
    "CEREBRAS_API_KEY",
    "HF_TOKEN",
    "AI_GATEWAY_API_KEY",
    "REQUESTY_API_KEY",
    "DEEPINFRA_API_KEY",
    "DASHSCOPE_API_KEY",
    "PERPLEXITY_API_KEY",
    "COHERE_API_KEY",
    "OLLAMA_API_KEY",
    "LMSTUDIO_API_KEY",
    "AZURE_API_KEY",
    "AWS_BEARER_TOKEN_BEDROCK",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "CLOUDFLARE_API_TOKEN",
    "CLOUDFLARE_API_KEY",
    "DIGITALOCEAN_ACCESS_TOKEN",
    "DATABRICKS_TOKEN",
];

#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// OpenCode 자식 프로세스에서 제거할 공급자 API 키 환경 변수 패턴 검사.
pub fn is_opencode_secret_env(name: &str) -> bool {
    if name == "GITHUB_TOKEN" || name == "GITLAB_TOKEN" {
        return true;
    }
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_') {
        return false;
    }
    const SUFFIXES: &[&str] = &[
        "_SECRET_ACCESS_KEY",
        "_SERVICE_KEY",
        "_ACCESS_KEY",
        "_API_TOKEN",
        "_API_KEY",
        "_APIKEY",
        "_PAT",
    ];
    SUFFIXES.iter().any(|s| name.len() > s.len() && name.ends_with(s))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Opencode,
}

pub const ALL: [Kind; 1] = [Kind::Opencode];

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Opencode => "opencode",
        }
    }
    pub fn from_id(_s: &str) -> Kind {
        Kind::Opencode
    }
    pub fn label(self) -> &'static str {
        match self {
            Kind::Opencode => "OpenCode",
        }
    }
    pub fn plan(self) -> &'static str {
        match self {
            Kind::Opencode => "OpenCode (DeepSeek 무료 · Console/Go)",
        }
    }
    pub fn can_auto(self) -> bool {
        true
    }
    fn exe_names(self) -> &'static [&'static str] {
        if cfg!(windows) {
            &["opencode.exe", "opencode.cmd"]
        } else {
            &["opencode"]
        }
    }
    fn fallback_dirs(self) -> Vec<PathBuf> {
        let roaming = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).unwrap_or_default();
        vec![
            roaming.join("npm").join("node_modules").join("opencode-ai").join("bin"),
            roaming.join("npm"),
            home.join(".opencode").join("bin"),
            home.join("scoop").join("shims"),
            home.join(".local").join("bin"),
        ]
    }
}

/// 실행 파일 위치: OpenCode는 네이티브 바이너리(`node_modules/opencode-ai/bin/opencode.exe`)를 .cmd 래퍼보다 먼저 찾는다.
pub fn locate(kind: Kind) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if kind == Kind::Opencode {
        if let Some(roaming) = std::env::var_os("APPDATA").map(PathBuf::from) {
            dirs.push(roaming.join("npm").join("node_modules").join("opencode-ai").join("bin"));
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path_var));
    }
    dirs.extend(kind.fallback_dirs());
    for dir in dirs {
        for name in kind.exe_names() {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn ensure_opencode_isolated_config() -> Option<(PathBuf, PathBuf)> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir)
        .join("kr.ilson.desktop")
        .join("opencode-config");
    let oc_dir = base.join("opencode");
    std::fs::create_dir_all(&oc_dir).ok()?;
    let cfg_file = oc_dir.join("opencode.json");
    if !cfg_file.is_file() {
        let minimal = r#"{"$schema":"https://opencode.ai/config.json","autoupdate":false,"model":"opencode/deepseek-v4.1-flash-free"}"#;
        let _ = std::fs::write(&cfg_file, minimal);
    }
    Some((base, oc_dir))
}

pub(crate) fn command_for(kind: Kind, exe: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(exe);
    for key in STRIPPED_ENV {
        cmd.env_remove(key);
    }
    if kind == Kind::Opencode {
        for (k, _) in std::env::vars() {
            if is_opencode_secret_env(&k) {
                cmd.env_remove(&k);
            }
        }
        cmd.env("OPENCODE_DISABLE_AUTOUPDATE", "1");
        if let Some((xdg_root, oc_dir)) = ensure_opencode_isolated_config() {
            cmd.env("XDG_CONFIG_HOME", xdg_root);
            cmd.env("OPENCODE_CONFIG_DIR", oc_dir);
        }
        cmd.arg("--pure");
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub id: String,
    pub label: String,
    pub plan: String,
    pub can_auto: bool,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub signed_in: bool,
    pub login: Option<String>,
    pub models: Vec<ModelInfo>,
    pub message: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
}

async fn capture_for(kind: Kind, exe: &Path, args: &[&str], secs: u64) -> Result<(bool, String, String)> {
    let child = command_for(kind, exe).stdin(Stdio::null()).args(args).spawn()?;
    let out = tokio::time::timeout(Duration::from_secs(secs), child.wait_with_output())
        .await
        .map_err(|_| anyhow!("응답이 없어 중단했습니다"))??;
    Ok((
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    ))
}

fn model(id: &str, label: &str) -> ModelInfo {
    ModelInfo { id: id.into(), label: label.into() }
}

pub fn opencode_static_models() -> Vec<ModelInfo> {
    vec![
        model("opencode/deepseek-v4.1-flash-free", "DeepSeek V4.1 Flash (무료)"),
        model("opencode/glm-5.3-flash-free", "GLM 5.3 Flash (무료)"),
        model("opencode/nemotron-3.5-super-vl-free", "Nemotron 3.5 Super VL (무료)"),
        model("opencode/big-pickle", "Big Pickle (무료)"),
        model("opencode/deepseek-v4.1-flash", "DeepSeek V4.1 Flash (Console)"),
        model("opencode-go/deepseek-v4.1-flash", "DeepSeek V4.1 Flash (Go)"),
    ]
}

fn format_opencode_model_label(id: &str) -> String {
    match id {
        "opencode/deepseek-v4.1-flash-free" => "DeepSeek V4.1 Flash (무료)".into(),
        "opencode/deepseek-v4.1-flash" => "DeepSeek V4.1 Flash (Console)".into(),
        "opencode-go/deepseek-v4.1-flash" => "DeepSeek V4.1 Flash (Go)".into(),
        "opencode/glm-5.3-flash-free" => "GLM 5.3 Flash (무료)".into(),
        "opencode/nemotron-3.5-super-vl-free" => "Nemotron 3.5 Super VL (무료)".into(),
        "opencode/nemotron-3.5-lightning-free" => "Nemotron 3.5 Lightning (무료)".into(),
        "opencode/nemotron-3-ultra-free" => "Nemotron 3 Ultra (무료)".into(),
        "opencode/big-pickle" => "Big Pickle (무료)".into(),
        _ => {
            let is_go = id.starts_with("opencode-go/");
            let slug = id.split_once('/').map(|(_, s)| s).unwrap_or(id);
            let (base, free) = match slug.strip_suffix("-free") {
                Some(b) => (b, true),
                None => (slug, false),
            };
            let words: Vec<String> = base
                .split('-')
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                })
                .collect();
            let title = words.join(" ");
            if free {
                format!("{title} (무료)")
            } else if is_go {
                format!("{title} (Go)")
            } else {
                title
            }
        }
    }
}

/// 설치·로그인 상태를 확인한다. 사용량을 쓰지 않는다.
pub async fn status(kind: Kind) -> EngineStatus {
    let mut st = EngineStatus {
        id: kind.id().into(),
        label: kind.label().into(),
        plan: kind.plan().into(),
        can_auto: kind.can_auto(),
        ..Default::default()
    };
    let Some(exe) = locate(kind) else {
        st.message = Some("OpenCode를 찾지 못했습니다. `npm i -g opencode-ai@latest`로 설치할 수 있습니다.".into());
        return st;
    };
    st.installed = true;
    st.path = Some(exe.to_string_lossy().into());

    let mut msgs: Vec<String> = Vec::new();
    if let Ok((_, out, _)) = capture_for(kind, &exe, &["--version"], 20).await {
        let ver = out.split_whitespace().last().map(String::from);
        if let Some(ref v) = ver {
            let major = v.trim_start_matches('v').split('.').next().and_then(|m| m.parse::<u32>().ok()).unwrap_or(1);
            if major >= 2 {
                st.can_auto = false;
                msgs.push("OpenCode 2.x는 권한 제어 방식이 달라 안전 모드로만 연결합니다. 1.x(`npm i -g opencode-ai@latest`)를 권장합니다.".into());
            }
        }
        st.version = ver;
    }
    st.signed_in = true;
    st.login = Some("자격 없음 · 무료 모델".into());
    if let Ok((_, out, err)) = capture_for(kind, &exe, &["auth", "list"], 20).await {
        let text = format!("{out}\n{err}");
        let creds_section = text.split("Environment").next().unwrap_or(&text);
        let mut has_opencode = false;
        let mut has_other = false;
        for line in creds_section.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('•') || trimmed.starts_with('*') || trimmed.starts_with('-') {
                let lower = trimmed.to_lowercase();
                if lower.contains("opencode") {
                    has_opencode = true;
                } else {
                    has_other = true;
                }
            }
        }
        if has_opencode {
            st.login = Some("OpenCode Console/Go 키 (CLI 보관)".into());
        }
        if has_other {
            msgs.push("다른 공급자 키는 쓰지 않습니다(모델을 opencode/로 고정)".into());
        }
    }
    match capture_for(kind, &exe, &["models", "opencode"], 40).await {
        Ok((true, out, _)) => {
            let mut parsed: Vec<ModelInfo> = out
                .lines()
                .map(str::trim)
                .filter(|l| l.starts_with("opencode/") || l.starts_with("opencode-go/"))
                .map(|id| model(id, &format_opencode_model_label(id)))
                .collect();
            if parsed.is_empty() {
                st.models = opencode_static_models();
                msgs.push("모델 목록을 가져오지 못해 기본 목록을 씁니다".into());
            } else {
                parsed.sort_by_key(|m| if m.id == OPENCODE_DEFAULT_MODEL { 0 } else { 1 });
                for extra in ["opencode/deepseek-v4.1-flash", "opencode-go/deepseek-v4.1-flash"] {
                    if !parsed.iter().any(|m| m.id == extra) {
                        parsed.push(model(extra, &format_opencode_model_label(extra)));
                    }
                }
                st.models = parsed;
            }
        }
        _ => {
            st.models = opencode_static_models();
            msgs.push("모델 목록을 가져오지 못해 기본 목록을 씁니다".into());
        }
    }
    if !msgs.is_empty() {
        st.message = Some(msgs.join(" · "));
    }
    st
}

/// 사람이 알아볼 수 있는 실패 이유로 바꾼다.
pub fn friendly(text: &str) -> Option<String> {
    let t = text.to_lowercase();
    let msg = if t.contains("free usage exceeded") || t.contains("add credits") {
        "OpenCode 무료 한도에 닿았습니다. 잠시 뒤 이어하기를 누르거나 연결 및 설정에서 Go 모델을 선택해 주세요."
    } else if t.contains("missingsessionid") {
        "OpenCode 세션 오류가 발생했습니다. 잠시 뒤 다시 시도하거나 버전을 확인해 주세요."
    } else if t.contains("authentication expired")
        || t.contains("log back in")
        || t.contains("not logged in")
        || t.contains("please log in")
        || t.contains("401")
        || t.contains("unauthorized")
        || t.contains("invalid api key")
    {
        "인증이 필요하거나 만료되었습니다. 연결 및 설정에서 다시 확인해 주세요."
    } else if t.contains("ai credits balance")
        || t.contains("quota")
        || t.contains("resource_exhausted")
        || t.contains("rate limit")
        || t.contains("usage limit")
        || t.contains("limit reached")
    {
        "사용 한도에 닿았습니다. 한도가 다시 채워진 뒤 ‘이어하기’를 눌러 주세요."
    } else if t.contains("suspended") || t.contains("verify your account") || t.contains("terms of service") {
        "계정이 정지되었거나 확인이 필요하다는 응답을 받았습니다. 해당 서비스에 직접 로그인해 안내를 확인해 주세요."
    } else {
        return None;
    };
    Some(msg.into())
}

/// 권한 단계. 각 CLI의 실제 옵션으로 바뀐다.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Level {
    /// 읽기와 답변만 (파일 수정·명령 실행 거부)
    ReadOnly,
    /// 파일 수정 자동 승인, 명령은 거부
    Edit,
    /// 파일 수정 + 샌드박스 안 명령 자동 승인
    Sandbox,
    /// 모든 확인 생략 (YOLO)
    Yolo,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::ReadOnly => "읽기",
            Level::Edit => "편집",
            Level::Sandbox => "샌드박스 실행",
            Level::Yolo => "YOLO",
        }
    }
}

/// 엔진·권한 단계별 CLI 옵션 (화면의 ‘YOLO 키워드’ 표와 같다).
pub fn level_args(kind: Kind, level: Level) -> Vec<&'static str> {
    match (kind, level) {
        (Kind::Opencode, Level::Yolo) => vec!["--auto"],
        (Kind::Opencode, _) => vec![],
    }
}

/// OpenCode 권한 단계별 환경 변수(`OPENCODE_PERMISSION` 인라인 JSON).
pub fn level_env(kind: Kind, level: Level) -> Vec<(&'static str, String)> {
    if kind != Kind::Opencode {
        return vec![];
    }
    let json = match level {
        Level::ReadOnly => r#"{"edit":"deny","bash":"deny","webfetch":"deny","websearch":"deny","external_directory":"deny","doom_loop":"deny"}"#,
        Level::Edit => r#"{"edit":"allow","bash":"deny","webfetch":"deny","websearch":"deny","external_directory":"deny","doom_loop":"deny"}"#,
        Level::Sandbox => r#"{"edit":"allow","bash":"allow","webfetch":"allow","websearch":"allow","external_directory":"deny","doom_loop":"deny"}"#,
        Level::Yolo => r#"{"*":"allow"}"#,
    };
    vec![("OPENCODE_PERMISSION", json.into())]
}

/// 화면의 권한 단계 표에 보여 줄 문자열.
pub fn level_display(kind: Kind, level: Level) -> String {
    let args = level_args(kind, level).join(" ");
    let envs: Vec<String> = level_env(kind, level).into_iter().map(|(k, v)| format!("{k}={v}")).collect();
    match (args.is_empty(), envs.is_empty()) {
        (true, true) => String::new(),
        (false, true) => args,
        (true, false) => envs.join(" "),
        (false, false) => format!("{args} ({})", envs.join(" ")),
    }
}

pub struct RunOptions<'a> {
    pub kind: Kind,
    pub cwd: &'a Path,
    pub prompt: String,
    pub model: Option<String>,
    pub level: Level,
    pub title: Option<String>,
}

/// 진행 중 화면에 보여 줄 활동.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    /// typing | reading | running | thinking | talking
    pub kind: String,
    pub label: String,
    /// 기록에 남길 만한 단계인지 (파일 수정·명령 실행)
    pub notable: bool,
}

impl Activity {
    pub fn new(kind: &str, label: impl Into<String>, notable: bool) -> Self {
        Self { kind: kind.into(), label: label.into(), notable }
    }
}

#[derive(Default, Debug, Clone)]
pub struct RunResult {
    pub response: String,
    pub conversation_id: Option<String>,
    /// 신규 입력 토큰 (캐시 제외)
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub reported_cost_usd: Option<f64>,
    pub duration_ms: u64,
    pub model: Option<String>,
    pub denied: Vec<String>,
    pub tools_used: u32,
}

fn short(path: &str) -> String {
    let p = path.replace('\\', "/");
    let parts: Vec<&str> = p.rsplitn(3, '/').collect();
    match parts.as_slice() {
        [file, dir, _] | [file, dir] => format!("{dir}/{file}"),
        _ => p,
    }
}

fn param_hint(params: &Value) -> Option<String> {
    let obj = params.as_object()?;
    let pick = |pred: &dyn Fn(&str) -> bool| obj.iter().find(|(k, v)| pred(&k.to_lowercase()) && v.is_string()).map(|(_, v)| v.as_str().unwrap_or("").to_string());
    if let Some(c) = pick(&|k| k.contains("command")) {
        return Some(c.chars().take(60).collect());
    }
    if let Some(p) = pick(&|k| k.contains("file") || k.contains("path") || k.contains("dir")) {
        return Some(short(&p));
    }
    pick(&|k| k.contains("query") || k.contains("url") || k.contains("pattern")).map(|q| q.chars().take(50).collect())
}

/// 도구 이름을 사무실 화면의 동작으로 바꾼다.
fn tool_activity(name: &str, params: &Value, done: bool) -> Activity {
    let hint = param_hint(params).unwrap_or_default();
    let n = name.to_lowercase();
    let (kind, verb) = if ["write", "edit", "replace", "sed_file", "notebook", "apply_patch", "file_change", "fs_write"].iter().any(|k| n.contains(k)) {
        ("typing", "고치는 중")
    } else if ["bash", "run_command", "command", "shell", "exec", "execute_bash"].iter().any(|k| n.contains(k)) {
        ("running", "명령 실행")
    } else if n.contains("search_web") || n.contains("websearch") || n.contains("web_search") {
        ("reading", "웹 검색")
    } else if n.contains("url") || n.contains("fetch") || n.contains("browser") {
        ("reading", "웹 페이지 읽기")
    } else if ["subagent", "task", "agent", "send_message"].iter().any(|k| n.contains(k)) {
        ("talking", "동료와 상의")
    } else {
        ("reading", "살펴보는 중")
    };
    let notable = done && matches!(kind, "typing" | "running");
    Activity::new(kind, if hint.is_empty() { verb.to_string() } else { format!("{verb} · {hint}") }, notable)
}

/// OpenCode 스트림 출력을 읽어 진행 상황과 결과를 모은다.
#[derive(Default)]
struct Collector {
    out: RunResult,
    status: Option<Result<(), String>>,
    step_buf: String,
    all_text: String,
    seen_step_ids: HashSet<String>,
    cost_accum: f64,
    has_cost: bool,
}

impl Collector {
    fn line(&mut self, _kind: Kind, ev: &Value, on: &mut dyn FnMut(Activity)) {
        self.opencode(ev, on);
    }

    fn raw_line(&mut self, _kind: Kind, line: &str) {
        let lower = line.to_lowercase();
        if lower.contains("auto-rejecting") || lower.contains("permission denied") {
            let tool = if let Some(idx) = lower.find("permission requested:") {
                let after = line[idx + "permission requested:".len()..].trim();
                after
                    .split(|c: char| c.is_whitespace() || c == '(' || c == ';' || c == ':' || c == ',')
                    .find(|s| !s.is_empty())
                    .unwrap_or("권한 거부")
                    .to_string()
            } else {
                "권한 거부".to_string()
            };
            if !self.out.denied.contains(&tool) {
                self.out.denied.push(tool);
            }
        }
    }

    fn finish(&mut self, _kind: Kind, exit_ok: bool) {
        self.out.response = if !self.step_buf.trim().is_empty() {
            self.step_buf.clone()
        } else {
            self.all_text.clone()
        };
        if self.has_cost {
            self.out.reported_cost_usd = Some(self.cost_accum);
        }
        if self.status.is_none() && exit_ok && !self.out.response.trim().is_empty() {
            self.status = Some(Ok(()));
        }
    }

    fn opencode(&mut self, ev: &Value, on: &mut dyn FnMut(Activity)) {
        match ev["type"].as_str() {
            Some("step_start") => {
                self.step_buf.clear();
                if self.out.conversation_id.is_none() {
                    self.out.conversation_id = ev["sessionID"].as_str().map(String::from);
                    on(Activity::new("thinking", "요청을 읽는 중", false));
                }
            }
            Some("text") => {
                if let Some(t) = ev["part"]["text"].as_str() {
                    self.step_buf.push_str(t);
                    self.all_text.push_str(t);
                }
                on(Activity::new("thinking", "정리해서 적는 중", false));
            }
            Some("reasoning") => {
                on(Activity::new("thinking", "생각하는 중", false));
            }
            Some("tool_use") => {
                self.out.tools_used += 1;
                let tool = ev["part"]["tool"].as_str().unwrap_or("");
                let state = &ev["part"]["state"];
                let mut act = tool_activity(tool, &state["input"], true);
                if state["status"] == "error" {
                    act.label = format!("{} (실패)", act.label);
                }
                on(act);
            }
            Some("step_finish") => {
                let part = &ev["part"];
                let pid = part["id"].as_str().unwrap_or("").to_string();
                if pid.is_empty() || self.seen_step_ids.insert(pid) {
                    let tok = &part["tokens"];
                    self.out.input_tokens += tok["input"].as_u64().unwrap_or(0);
                    self.out.cache_read_tokens += tok["cache"]["read"].as_u64().unwrap_or(0);
                    self.out.cache_write_tokens += tok["cache"]["write"].as_u64().unwrap_or(0);
                    self.out.output_tokens += tok["output"].as_u64().unwrap_or(0);
                    self.out.reasoning_tokens += tok["reasoning"].as_u64().unwrap_or(0);
                    if let Some(c) = part["cost"].as_f64() {
                        self.cost_accum += c;
                        self.has_cost = true;
                    }
                }
                if part["reason"] == "stop" {
                    self.status = Some(Ok(()));
                }
            }
            Some("error") => {
                let msg = ev["error"]["data"]["message"]
                    .as_str()
                    .or_else(|| ev["error"]["message"].as_str())
                    .or_else(|| ev["message"].as_str())
                    .unwrap_or("OpenCode 오류");
                self.status = Some(Err(msg.to_string()));
            }
            _ => {}
        }
    }
}

/// OpenCode CLI를 헤드리스로 한 번 실행한다. 반환된 future를 버리면 자식 프로세스도 종료된다.
pub async fn run(opts: RunOptions<'_>, mut on_activity: impl FnMut(Activity)) -> Result<RunResult> {
    let started = Instant::now();
    let kind = opts.kind;
    let exe = locate(kind).ok_or_else(|| anyhow!("OpenCode CLI를 찾지 못했습니다. `npm i -g opencode-ai@latest`로 설치해 주세요."))?;
    let mut cmd = command_for(kind, &exe);
    cmd.current_dir(opts.cwd);
    let model = opts.model.as_deref().filter(|m| !m.is_empty());
    let m = model.unwrap_or(OPENCODE_DEFAULT_MODEL);
    if !m.starts_with("opencode/") && !m.starts_with("opencode-go/") {
        bail!("OpenCode는 opencode/ 또는 opencode-go/ 모델만 씁니다.");
    }
    let effective_model = Some(m.to_string());
    let title_cut: String = opts.title.as_deref().unwrap_or("업무").chars().take(40).collect();
    let title_arg = format!("일손 · {title_cut}");
    cmd.args(["run", "--format", "json", "--title", &title_arg, "-m", m]);
    cmd.args(level_args(kind, opts.level));
    for (k, v) in level_env(kind, opts.level) {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| anyhow!("OpenCode를 실행하지 못했습니다: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(opts.prompt.as_bytes()).await?;
        stdin.shutdown().await?;
    }
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("출력을 읽을 수 없습니다"))?;
    let mut stderr = child.stderr.take().ok_or_else(|| anyhow!("오류 출력을 읽을 수 없습니다"))?;
    let err_task = tokio::spawn(async move {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s).await;
        s
    });

    let mut col = Collector::default();
    col.out.model = effective_model;
    let mut lines = BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await? {
        if let Ok(ev) = serde_json::from_str::<Value>(&line) {
            col.line(kind, &ev, &mut on_activity);
        } else {
            col.raw_line(kind, &line);
        }
    }
    let code = child.wait().await?;
    let stderr = err_task.await.unwrap_or_default();
    for line in stderr.lines() {
        col.raw_line(kind, line);
    }
    col.finish(kind, code.success());
    col.out.duration_ms = started.elapsed().as_millis() as u64;
    match col.status {
        Some(Ok(())) => Ok(col.out),
        Some(Err(e)) => bail!(friendly(&format!("{e}\n{stderr}")).unwrap_or_else(|| format!("OpenCode 작업이 끝나지 못했습니다. {}", tail(&format!("{e}\n{stderr}"))))),
        None => bail!(friendly(&stderr).unwrap_or_else(|| format!(
            "OpenCode가 결과 없이 종료되었습니다 (코드 {}). {}",
            code.code().unwrap_or(-1),
            tail(&stderr)
        ))),
    }
}

fn tail(s: &str) -> String {
    let t: Vec<&str> = s.trim().lines().filter(|l| !l.trim().is_empty()).rev().take(3).collect();
    t.into_iter().rev().collect::<Vec<_>>().join(" / ")
}

/// 사장님이 직접 실행할 때 쓰는 명령 (안전 모드).
pub fn handoff_command(_kind: Kind, folder: &str, agent: &str, line: &str) -> String {
    let cd = if cfg!(windows) { format!("cd /d \"{folder}\" && ") } else { format!("cd \"{folder}\" && ") };
    format!("{cd}opencode --agent {agent} --prompt \"{line}\"")
}

/// 로그인할 수 있게 새 터미널 창에서 OpenCode CLI를 연다.
pub fn open_login_terminal(kind: Kind) -> Result<()> {
    let exe = locate(kind).ok_or_else(|| anyhow!("OpenCode CLI를 찾지 못했습니다."))?;
    let args: &[&str] = &["auth", "login"];
    #[cfg(windows)]
    {
        std::process::Command::new("cmd").args(["/c", "start", "OpenCode 로그인", "cmd", "/k"]).arg(&exe).args(args).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = args;
        std::process::Command::new("open").args(["-a", "Terminal"]).arg(&exe).spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("x-terminal-emulator").arg("-e").arg(&exe).args(args).spawn()?;
    }
    Ok(())
}

/// 작업 폴더에서 새 터미널을 연다 (안전 모드에서 사장님이 명령을 붙여 넣도록).
pub fn open_terminal(folder: &str) -> Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd").args(["/c", "start", "일손 작업 폴더", "cmd", "/k", "cd", "/d"]).arg(folder).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").args(["-a", "Terminal"]).arg(folder).spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("x-terminal-emulator").current_dir(folder).spawn()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(kind: Kind, lines: &[&str]) -> (Collector, Vec<Activity>) {
        let mut col = Collector::default();
        let mut acts = vec![];
        for l in lines {
            if let Ok(ev) = serde_json::from_str::<Value>(l) {
                col.line(kind, &ev, &mut |a| acts.push(a));
            } else {
                col.raw_line(kind, l);
            }
        }
        col.finish(kind, true);
        (col, acts)
    }

    #[test]
    fn parses_opencode_stream() {
        let (col, acts) = collect(Kind::Opencode, &[
            r#"{"type":"step_start","timestamp":1760000001,"sessionID":"ses_1","part":{"id":"prt_1","type":"step-start"}}"#,
            r#"{"type":"tool_use","timestamp":1760000002,"sessionID":"ses_1","part":{"id":"prt_2","type":"tool","tool":"edit","state":{"status":"completed","input":{"filePath":"src/index.ts"}}}}"#,
            r#"permission requested: bash (ls -la); auto-rejecting"#,
            r#"{"type":"step_finish","timestamp":1760000003,"sessionID":"ses_1","part":{"id":"prt_3","type":"step-finish","reason":"tool-calls","cost":0,"tokens":{"total":1200,"input":900,"output":50,"reasoning":20,"cache":{"read":250,"write":0}}}}"#,
            r#"{"type":"step_start","timestamp":1760000004,"sessionID":"ses_1","part":{"id":"prt_4","type":"step-start"}}"#,
            r#"{"type":"text","timestamp":1760000005,"sessionID":"ses_1","part":{"id":"prt_5","type":"text","text":"안녕하세요, 일손입니다."}}"#,
            r#"{"type":"step_finish","timestamp":1760000006,"sessionID":"ses_1","part":{"id":"prt_6","type":"step-finish","reason":"stop","cost":0,"tokens":{"total":1500,"input":300,"output":40,"reasoning":10,"cache":{"read":1150,"write":0}}}}"#,
        ]);
        assert!(matches!(col.status, Some(Ok(()))));
        assert_eq!(col.out.conversation_id.as_deref(), Some("ses_1"));
        assert_eq!(col.out.response, "안녕하세요, 일손입니다.");
        assert_eq!(col.out.input_tokens, 1200);
        assert_eq!(col.out.cache_read_tokens, 1400);
        assert_eq!(col.out.output_tokens, 90);
        assert_eq!(col.out.reasoning_tokens, 30);
        assert_eq!(col.out.reported_cost_usd, Some(0.0));
        assert_eq!(col.out.denied, vec!["bash".to_string()]);
        assert!(acts.iter().any(|a| a.kind == "typing" && a.label.contains("src/index.ts")));
    }

    #[test]
    fn opencode_permission_json_is_valid() {
        for lvl in [Level::ReadOnly, Level::Edit, Level::Sandbox, Level::Yolo] {
            let envs = level_env(Kind::Opencode, lvl);
            assert_eq!(envs.len(), 1);
            assert_eq!(envs[0].0, "OPENCODE_PERMISSION");
            let parsed: Value = serde_json::from_str(&envs[0].1).expect("OPENCODE_PERMISSION must be valid JSON");
            assert!(parsed.is_object());
        }
        assert!(is_opencode_secret_env("GROQ_API_KEY"));
        assert!(is_opencode_secret_env("CUSTOM_SERVICE_KEY"));
        assert!(is_opencode_secret_env("GITHUB_TOKEN"));
        assert!(!is_opencode_secret_env("OPENCODE_DISABLE_AUTOUPDATE"));
        assert!(!is_opencode_secret_env("PATH"));
        assert!(!is_opencode_secret_env("APPDATA"));
    }

    #[test]
    fn yolo_flags_match_opencode() {
        assert_eq!(level_args(Kind::Opencode, Level::Yolo), vec!["--auto"]);
        assert_eq!(level_args(Kind::Opencode, Level::ReadOnly), Vec::<&str>::new());
    }
}
