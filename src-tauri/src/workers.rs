//! 직원별 단계 구현. runner가 단계 순서·확인·체크포인트를 맡고, 여기서는 각 단계의 일만 한다.

use crate::engine::{self, Activity, Kind, Level};
use crate::gitops::{self, Workspace};
use crate::models::{Gate, Settings};
use crate::{db, roster, web, AppState};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

pub struct Ctx {
    pub app: AppHandle,
    pub task_id: String,
    pub employee_id: String,
    pub title: String,
    pub input: Value,
    pub outputs: HashMap<String, Value>,
    pub settings: Settings,
    pub http: reqwest::Client,
    pub data_dir: PathBuf,
}

pub struct StepResult {
    pub output: Value,
    pub note: Option<String>,
    pub cost_usd: f64,
}

impl StepResult {
    fn new(output: Value, note: impl Into<String>) -> Self {
        Self { output, note: Some(note.into()), cost_usd: 0.0 }
    }
}

impl Ctx {
    fn s(&self, key: &str) -> String {
        match &self.input[key] {
            Value::String(s) => s.trim().to_string(),
            Value::Number(n) => n.to_string(),
            _ => String::new(),
        }
    }
    fn n(&self, key: &str, default: i64) -> i64 {
        match &self.input[key] {
            Value::Number(n) => n.as_i64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
        .unwrap_or(default)
    }
    fn b(&self, key: &str, default: bool) -> bool {
        self.input[key].as_bool().unwrap_or(default)
    }
    fn out(&self, key: &str) -> &Value {
        self.outputs.get(key).unwrap_or(&Value::Null)
    }

    /// 사무실 화면과 업무 기록에 지금 하는 일을 알린다.
    fn report(&self, actor: &str, act: &Activity) {
        let payload = json!({
            "taskId": self.task_id, "employeeId": actor, "kind": act.kind, "label": act.label,
            "engine": self.settings.engine_for(actor).id(), "at": db::now(),
        });
        let state = self.app.state::<AppState>();
        state.activities.lock().unwrap().insert(actor.to_string(), payload.clone());
        if act.notable {
            if let Ok(conn) = state.db.lock() {
                let name = roster::find(actor).and_then(|e| e["name"].as_str().map(String::from)).unwrap_or_default();
                let _ = db::add_event(&conn, &self.task_id, "info", &format!("{name} · {}", act.label));
            }
        }
        let _ = self.app.emit("activity", payload);
    }

    fn short_id(&self) -> String {
        self.task_id.replace('-', "").chars().take(8).collect()
    }

    fn workspace(&self) -> Result<Workspace> {
        serde_json::from_value(self.out("setup")["workspace"].clone()).map_err(|_| anyhow!("작업 공간 정보가 없습니다. 업무를 다시 맡겨 주세요."))
    }
}

pub fn won(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

// ───────────────────────── 공통: 사업장·문체 ─────────────────────────

fn business_block(s: &Settings) -> String {
    let rows = [
        ("상호", &s.business_name),
        ("업종", &s.industry),
        ("지역", &s.region),
        ("연락처", &s.phone),
    ];
    let lines: Vec<String> = rows
        .iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| format!("- {k}: {}", v.trim()))
        .collect();
    if lines.is_empty() { "(입력된 사업장 정보 없음)".into() } else { lines.join("\n") }
}

fn brand_block(s: &Settings) -> String {
    let mut lines = vec![format!("- 말투: {}", if s.brand_tone.is_empty() { "친근한" } else { &s.brand_tone })];
    if !s.brand_keywords.trim().is_empty() {
        lines.push(format!("- 브랜드 키워드: {}", s.brand_keywords.trim()));
    }
    if !s.banned_words.trim().is_empty() {
        lines.push(format!("- 쓰지 말 표현: {}", s.banned_words.trim()));
    }
    if !s.sample_text.trim().is_empty() {
        lines.push(format!("- 평소 글 예시:\n\"\"\"\n{}\n\"\"\"", s.sample_text.trim()));
    }
    lines.join("\n")
}

fn opt_line(label: &str, value: &str) -> String {
    if value.is_empty() { String::new() } else { format!("- {label}: {value}\n") }
}

// ───────────────────────── 프롬프트 ─────────────────────────

const HARIN_SYSTEM: &str = r#"너는 소상공인 사장님을 돕는 한국어 블로그 작가 '하린'이다. 네이버 블로그에 올릴 발행 전 초안을 쓴다.

원칙
- 사장님이 준 사실만 사실로 쓴다. 가격·영업시간·수치·후기처럼 확인되지 않은 내용은 지어내지 말고 [확인 필요: 무엇]으로 남긴다.
- '최고', '1위', 의학적 효능 같은 과장이나 다른 가게를 깎아내리는 표현은 쓰지 않는다.
- 핵심 키워드는 제목과 첫 문단에 넣고, 본문 전체에 자연스럽게 4~7회 나오게 한다.
- 모바일에서 읽기 쉽게 문단은 2~3문장으로 짧게 쓰고, 소제목(###)으로 흐름을 나눈다. 표가 이해를 도우면 마크다운 표를 쓴다.
- 이모지는 많아야 소제목마다 하나.

아래 형식의 마크다운만 출력한다. 앞뒤 인사나 설명은 붙이지 않는다.

# (가장 추천하는 제목)
## 제목 후보
1. …
2. …
3. …
## 본문
(소제목은 ###)
## 마무리 문구 후보
- … (3개)
## 해시태그
#태그 #태그 … (8~12개, 한 줄)
## 올리기 전 확인할 부분
- … (사진, 사실 확인 등 3~5개)"#;

const SEOJIN_SYSTEM: &str = r#"너는 소상공인과 작은 회사를 돕는 자료조사원 '서진'이다. 주어진 수집 자료와 웹 검색 결과만 근거로 조사 브리프를 쓴다.

원칙
- 주장마다 근거를 단다. 수집 자료는 [1], [2]처럼 번호로, 웹 검색으로 찾은 근거는 (출처: 페이지 제목)으로 적는다.
- 확인된 사실 / 추정 / 근거 부족을 구분한다. 자료에 없는 내용은 지어내지 않는다.
- 숫자·가격·날짜는 자료에 나온 그대로 옮기고 기준 시점을 적는다.

아래 형식의 마크다운만 출력한다.

# (브리프 제목)
> 한 줄 결론
## 핵심 발견
- … [1]
## 비교표
(목적에 맞을 때만, 마크다운 표)
## 기회와 빈틈
## 다음 행동 후보
1. …
## 확실성 메모
- 확인됨: …
- 추정: …
- 근거 부족: …"#;

const MINJAE_SYSTEM: &str = r#"너는 작은 회사의 영업을 돕는 '민재'다. 공개된 정보만으로 잠재 고객 후보를 찾고 우선순위를 매긴다.

원칙
- 개인의 비공개 연락처나 개인정보는 수집하거나 추측하지 않는다. 연락 수단은 공개된 사업장 채널(공식 홈페이지, 공식 블로그·SNS, 대표 전화 등)만 근거 링크와 함께 적는다.
- 실제로 확인한 후보만 표에 넣는다. 확인이 부족하면 개수를 채우려고 지어내지 말고, 더 찾는 방법을 구체적으로 제안한다.
- 대량 발송이 아니라 한 곳씩 맞춤 연락하도록 돕는다.

아래 형식의 마크다운만 출력한다.

# 잠재 고객 후보: (타깃 요약)
## 먼저 찾은 고객의 모습
(타깃 정의 3줄)
## 후보 목록
| 순위 | 후보 | 맞는 이유 | 공개 채널·근거 | 첫 행동 |
|---|---|---|---|---|
## 첫 연락 문구
### 문구 A
### 문구 B
### 문구 C
## 더 찾는 방법
## 주의할 점"#;

const JIHO_SYSTEM: &str = r#"너는 숏폼 영상 대본 작가 '지호'다. 사장님이 휴대폰으로 직접 찍을 수 있는 현실적인 장면만 쓴다.

원칙
- 안마다 다른 타깃 또는 다른 첫 문장 전략을 쓴다.
- 자막은 한 줄 14자 안팎으로 짧게 쓴다.
- 확인되지 않은 가격이나 효능은 지어내지 말고 [확인 필요]로 둔다.

아래 형식의 마크다운만 출력한다. 요청한 안의 수만큼 반복한다.

# 숏폼 대본: (주제)
## A안 · (전략 이름)
- 타깃: …
- 후킹(0~2초): "…"

| 시간 | 화면 | 자막 | 대사 |
|---|---|---|---|

- 마무리 행동: …
- 해시태그: #… (5~8개)
- 촬영 팁: … (2개)"#;

const YENA_SYSTEM: &str = r#"너는 정부·지자체 지원사업 공고를 해석하는 '예나'다. 공고 자료와 회사 정보를 비교해 신청 가능성을 점검한다.

원칙
- 공고 자료에 있는 내용만 근거로 한다. 공고에 없거나 애매한 조건은 '확인 필요'로 표시하고, 무엇을 누구에게 물어야 하는지 적는다.
- 어려운 행정 용어에는 괄호로 쉬운 말을 붙인다.
- 마감일·금액·비율은 공고 표기 그대로 옮긴다.

아래 형식의 마크다운만 출력한다.

# 지원사업 점검: (사업명)
> 한 줄 판단 (신청 추천 / 조건부 가능 / 어려움 + 이유)
## 공고 요약
| 항목 | 내용 |
|---|---|
(사업명, 주관·전담기관, 지원 대상, 지원 내용·금액, 자부담, 신청 기간, 신청 방법)
## 신청 자격 점검
| 조건 | 우리 회사 | 판단 |
|---|---|---|
## 준비 서류
- [ ] …
## 사업계획서 목차 초안
1. … — 쓰는 요령
## 일정 거꾸로 세우기
## 담당자에게 물어볼 것"#;

fn choice_label<'a>(value: &str, table: &'a [(&str, &'a str)]) -> &'a str {
    table.iter().find(|(v, _)| *v == value).map(|(_, l)| *l).unwrap_or("")
}

fn pages_block(pages: &Value) -> String {
    let mut out = String::new();
    for (i, p) in pages.as_array().into_iter().flatten().enumerate() {
        out.push_str(&format!(
            "\n[{}] {}\n주소: {}\n\"\"\"\n{}\n\"\"\"\n",
            i + 1,
            p["title"].as_str().unwrap_or(""),
            p["url"].as_str().unwrap_or(""),
            p["text"].as_str().unwrap_or("")
        ));
    }
    out
}

fn val_s(input: &Value, key: &str) -> String {
    match &input[key] {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn val_n(input: &Value, key: &str, default: i64) -> i64 {
    match &input[key] {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
    .unwrap_or(default)
}

fn val_b(input: &Value, key: &str, default: bool) -> bool {
    input[key].as_bool().unwrap_or(default)
}

/// 비용 견적과 실제 호출이 같은 요청을 쓰도록 한 곳에서 만든다. (요청, 웹 검색 횟수, 보통 출력 토큰 수)
pub(crate) fn ai_request_from_input(
    s: &Settings,
    employee_id: &str,
    input: &Value,
    outputs: &HashMap<String, Value>,
) -> Result<(String, u32, u32)> {
    let searches = val_n(input, "webSearch", 0).clamp(0, 10) as u32;
    let out_val = |key: &str| outputs.get(key).unwrap_or(&Value::Null);
    let (system, user, typical) = match employee_id {
        "harin" => {
            let template = choice_label(
                &val_s(input, "template"),
                &[
                    ("info", "정보 정리형 — 목차와 비교표로 궁금증을 풀어 준다"),
                    ("review", "경험·후기형 — 방문 전후 이야기로 신뢰를 쌓는다"),
                    ("consult", "상담 유도형 — 고민을 짚고 문의로 이어지게 한다"),
                    ("list", "추천 리스트형 — 고르는 기준과 추천 목록으로 정리한다"),
                ],
            );
            let length = val_n(input, "length", 1500);
            let mut user = format!(
                "[요청]\n- 핵심 키워드: {}\n{}- 글 스타일: {template}\n- 분량: 본문 약 {length}자\n{}{}",
                val_s(input, "keyword"),
                opt_line("보조 키워드", &val_s(input, "subKeywords")),
                opt_line("마무리 행동 유도", &val_s(input, "cta")),
                opt_line("읽는 사람", &val_s(input, "audience")),
            );
            let facts = val_s(input, "facts");
            user.push_str(&format!("\n[꼭 들어갈 사실]\n{}\n", if facts.is_empty() { "(없음 — 사실이 필요한 곳은 [확인 필요]로 남길 것)" } else { &facts }));
            user.push_str(&format!("\n[사업장]\n{}\n", business_block(s)));
            if val_b(input, "useBrandStyle", true) {
                user.push_str(&format!("\n[브랜드 문체]\n{}\n", brand_block(s)));
            }
            (HARIN_SYSTEM.to_string(), user, (length as f64 * 1.4) as u32 + 2500)
        }
        "seojin" => {
            let purpose = choice_label(
                &val_s(input, "purpose"),
                &[
                    ("compare", "경쟁사 비교 — 포지션·가격대·강점·빈틈을 비교표로"),
                    ("trend", "시장·트렌드 — 최근 변화와 근거, 사장님께 주는 의미"),
                    ("reference", "레퍼런스 수집 — 참고할 만한 사례와 따라 할 점"),
                    ("factcheck", "사실 확인 — 주장별로 맞음/틀림/근거 부족 판정"),
                ],
            );
            let collected = out_val("collect");
            let mut user = format!(
                "[조사 주제]\n{}\n\n[조사 목적]\n{purpose}\n{}",
                val_s(input, "topic"),
                opt_line("꼭 알고 싶은 점", &val_s(input, "focus")),
            );
            user.push_str(&format!("\n[사장님 사업장]\n{}\n", business_block(s)));
            let pages = pages_block(&collected["pages"]);
            user.push_str(&format!(
                "\n[수집 자료]{}\n",
                if pages.is_empty() { "\n(없음 — 웹 검색 결과로만 조사)".to_string() } else { pages }
            ));
            if searches > 0 {
                user.push_str(&format!("\n웹 검색은 최대 {searches}회까지 쓸 수 있다. 수집 자료로 부족한 부분만 검색한다.\n"));
            }
            (SEOJIN_SYSTEM.to_string(), user, 4000)
        }
        "minjae" => {
            let channel = choice_label(
                &val_s(input, "channel"),
                &[
                    ("email", "이메일 (제목 포함, 5문장 이내)"),
                    ("blog", "블로그 이웃 신청·댓글 (2문장 이내, 광고 티 나지 않게)"),
                    ("instagram", "인스타그램 DM (3문장 이내)"),
                    ("call", "전화·방문 첫마디 스크립트 (30초 분량)"),
                ],
            );
            let mut user = format!(
                "[우리가 파는 것]\n{}\n\n[찾는 고객]\n{}\n{}- 첫 연락 채널: {channel}\n- 찾을 후보 수: {}곳\n{}",
                val_s(input, "offering"),
                val_s(input, "target"),
                opt_line("지역", &val_s(input, "region")),
                val_n(input, "count", 10),
                opt_line("우리만의 강점", &val_s(input, "strength")),
            );
            let candidates = val_s(input, "candidates");
            if !candidates.is_empty() {
                user.push_str(&format!("\n[이미 가진 후보 목록 — 이 후보들의 우선순위도 함께 매길 것]\n{candidates}\n"));
            }
            user.push_str(&format!("\n[사장님 사업장]\n{}\n", business_block(s)));
            if searches == 0 {
                user.push_str("\n웹 검색 없이 진행한다. 실제 상호를 지어내지 말고, 후보 유형과 찾는 방법·검색어를 중심으로 정리한다.\n");
            }
            let count = val_n(input, "count", 10) as u32;
            (MINJAE_SYSTEM.to_string(), user, 2000 + count * 250)
        }
        "jiho" => {
            let objective = choice_label(
                &val_s(input, "objective"),
                &[
                    ("save", "저장 유도 — 꿀팁형으로 저장·공유를 부른다"),
                    ("inquiry", "문의 전환 — 고민을 짚고 상담 문의로 이어 간다"),
                    ("visit", "방문·구매 — 보는 순간 가 보고 싶게 만든다"),
                    ("awareness", "브랜드 알리기 — 사장님과 가게의 이야기를 보여 준다"),
                ],
            );
            let platform = choice_label(
                &val_s(input, "platform"),
                &[("reels", "인스타그램 릴스"), ("shorts", "유튜브 쇼츠"), ("tiktok", "틱톡")],
            );
            let variants = val_n(input, "variants", 3).clamp(1, 3);
            let mut user = format!(
                "[요청]\n- 주제: {}\n- 목적: {objective}\n- 플랫폼: {platform}\n- 길이: {}초\n- 대본 수: {variants}안\n{}{}",
                val_s(input, "topic"),
                val_n(input, "duration", 30),
                opt_line("보는 사람", &val_s(input, "target")),
                opt_line("참고하고 싶은 점", &val_s(input, "reference")),
            );
            user.push_str(&format!("\n[사업장]\n{}\n\n[브랜드 문체]\n{}\n", business_block(s), brand_block(s)));
            (JIHO_SYSTEM.to_string(), user, 1200 + variants as u32 * 900)
        }
        "yena" => {
            let collected = out_val("collect");
            let age = choice_label(
                &val_s(input, "bizAge"),
                &[("pre", "예비창업"), ("0-1", "1년 미만"), ("1-3", "1~3년"), ("3-7", "3~7년"), ("7+", "7년 이상")],
            );
            let headcount = choice_label(
                &val_s(input, "headcount"),
                &[("0", "대표 1인"), ("1-4", "1~4명"), ("5-9", "5~9명"), ("10+", "10명 이상")],
            );
            let revenue = choice_label(
                &val_s(input, "revenue"),
                &[("none", "매출 없음"), ("lt1", "1억 미만"), ("1-10", "1억~10억"), ("10+", "10억 이상")],
            );
            let mut user = format!(
                "[우리 회사]\n{}\n- 업력: {age}\n- 상시 근로자: {headcount}\n- 작년 매출: {revenue}\n{}",
                business_block(s),
                opt_line("보유 인증·특이사항", &val_s(input, "extra")),
            );
            user.push_str(&format!("\n[공고 자료]{}\n", pages_block(&collected["pages"])));
            let text = collected["noticeText"]
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| val_s(input, "noticeText"));
            if !text.is_empty() {
                user.push_str(&format!("\n[사장님이 붙여 넣은 공고 본문]\n\"\"\"\n{text}\n\"\"\"\n"));
            }
            if searches > 0 {
                user.push_str(&format!("\n웹 검색을 최대 {searches}회 써서, 같은 회사가 함께 볼 만한 비슷한 지원사업도 끝에 '함께 볼 공고'로 2~3개 정리한다.\n"));
            }
            (YENA_SYSTEM.to_string(), user, 4500)
        }
        other => bail!("AI 단계가 없는 직원입니다: {other}"),
    };
    Ok((format!("{system}\n\n---\n{user}"), searches, typical))
}

pub(crate) fn ai_request(ctx: &Ctx) -> Result<(String, u32, u32)> {
    ai_request_from_input(&ctx.settings, &ctx.employee_id, &ctx.input, &ctx.outputs)
}

// ───────────────────────── 실행 방식: 안전 모드 / 자동 모드 ─────────────────────────
//
// 안전 모드(기본): 앱은 agy를 실행하지 않는다. 직원을 Antigravity 사용자 정의 에이전트(.agents/agents/ilson-*.md)와
// 업무 지시서(.ilson/TASK.md)로 준비해 두면 사장님이 Antigravity에서 실행하고, 앱은 보고서·상태 파일과 변경을 지켜본다.
// 자동 모드(설정에서 동의 필요): 앱이 사장님 PC의 공식 agy를 헤드리스로 실행한다.

pub fn is_safe(ctx: &Ctx) -> bool {
    ctx.input["_engine"] != "auto"
}

/// 이 직원의 일을 사장님이 직접 실행해야 하는지: 안전 모드 업무이거나, 그 엔진에 자동 실행 동의가 없을 때.
pub fn handoff_for(ctx: &Ctx, actor: &str) -> bool {
    is_safe(ctx) || !ctx.settings.auto_ok(ctx.settings.engine_for(actor))
}

/// AI가 해야 하는 일 한 건.
pub struct AiJob {
    pub actor: String,
    pub cwd: PathBuf,
    pub prompt: String,
    pub level: Level,
    /// 이 일을 하는 직원의 구독 엔진
    pub engine: Kind,
    /// 파일을 고치는 일이면 끝난 뒤 커밋한다.
    pub edits: bool,
    pub label: String,
    /// 안전 모드에서 함께 준비할 직원(에이전트 정의).
    pub team: Vec<String>,
}

pub enum Plan {
    Ai(AiJob),
    Skip(&'static str),
    Local,
}

const PROGRESS_DIR: &str = ".ilson";

fn name_of(id: &str) -> String {
    roster::find(id).and_then(|e| e["name"].as_str().map(String::from)).unwrap_or_else(|| id.to_string())
}

fn persona(id: &str) -> String {
    let e = roster::find(id).unwrap_or(Value::Null);
    let company = if roster::DEV_TEAM.contains(&id) { "AI 코딩 회사 '일손' 개발팀" } else { "AI 직원 사무소 '일손'" };
    format!(
        "너는 {company}의 {} '{}'다. 신조: “{}”",
        e["role"].as_str().unwrap_or(""),
        e["name"].as_str().unwrap_or(id),
        e["motto"].as_str().unwrap_or("")
    )
}

pub(crate) fn persona_text(id: &str) -> String {
    persona(id)
}

pub(crate) fn rules_preview(allow_run: bool, actor: &str) -> String {
    rules(allow_run, false, actor)
}

/// 끝맺음 규칙: 자동 모드는 답변으로, 안전 모드는 보고서 파일로 결과를 남긴다.
fn finish_rule(safe: bool, actor: &str, read_only: bool) -> String {
    if safe {
        let keep = if read_only { "\n- 보고서·상태 파일 말고는 어떤 파일도 고치지 않는다." } else { "" };
        format!(
            "- 시작할 때 `{PROGRESS_DIR}/status-{actor}.txt`에 지금 하는 일을 한 줄로 적고, 단계가 바뀔 때마다 고쳐 쓴다.\n- 끝나면 아래 형식의 보고서를 `{PROGRESS_DIR}/report-{actor}.md` 파일로 저장한다. 회사는 이 파일이 생기면 일이 끝난 것으로 본다.{keep}"
        )
    } else {
        "- 끝나면 아래 형식의 마크다운 보고서로만 답한다.".into()
    }
}

fn rules(allow_run: bool, safe: bool, actor: &str) -> String {
    let run = if allow_run {
        "- 필요하면 테스트·빌드 명령을 실행해 확인한다. 개발 서버처럼 끝나지 않는 명령은 띄우지 않고, 패키지 설치는 꼭 필요할 때만 한다."
    } else {
        "- 셸 명령은 실행하지 않는다. 테스트·빌드는 실행하지 말고, 사장님이 실행할 명령을 ‘확인 방법’에 적는다."
    };
    format!(
        "규칙\n- 현재 디렉터리(작업 공간) 안의 파일만 읽고 고친다. `{PROGRESS_DIR}/`와 `.agents/` 폴더는 회사 관리용이니 결과물로 쓰지 않는다.\n- 기존 코드 스타일과 구조를 따르고, 요청과 관계없는 리팩터링은 하지 않는다.\n- git 명령(commit, push, branch, reset, checkout 등)은 실행하지 않는다. 커밋은 회사가 대신 한다.\n- 비밀번호·API 키 같은 비밀 값을 만들거나 코드에 넣지 않는다.\n{run}\n{}\n\n## 한 일\n## 바꾼 파일\n## 확인 방법\n## 남은 문제",
        finish_rule(safe, actor, false)
    )
}

/// 사장님이 직접 실행할 때 팀원을 서브에이전트로 부를 수 있는 엔진인지.
fn supports_subagents(kind: Kind) -> bool {
    matches!(kind, Kind::Agy | Kind::Claude | Kind::Opencode)
}

fn plan_text(ctx: &Ctx) -> String {
    ctx.out("plan")["markdown"].as_str().unwrap_or("").to_string()
}

fn skipped(note: &str) -> StepResult {
    StepResult::new(json!({ "skipped": true }), note)
}

fn dev_for_fix(ctx: &Ctx) -> &'static str {
    match ctx.employee_id.as_str() {
        "geonwoo" => "geonwoo",
        "yuna" => "yuna",
        _ if ctx.s("devs") == "backend" => "geonwoo",
        _ => "yuna",
    }
}

fn run_mode(ctx: &Ctx, edits: bool) -> Level {
    match (edits, ctx.b("allowRun", false)) {
        (false, _) => Level::ReadOnly,
        (true, false) => Level::Edit,
        (true, true) => Level::Sandbox,
    }
}

fn review_prompt(focus: &str, patch: &str, safe: bool) -> String {
    format!(
        "{}\n아래 변경을 검토한다. 코드는 고치지 않는다. 필요하면 파일을 읽어 맥락을 확인한다. 확신이 없는 지적은 ‘확인 필요’라고 표시한다.\n\n[요청·중점]\n{}\n\n[변경 내용 (diff)]\n```diff\n{patch}\n```\n\n{}\n\n# 코드 리뷰\n## 요약\n## 반드시 고칠 것\n(없으면 '없음'. 있으면 파일:줄 · 문제 · 고치는 방법)\n## 고치면 좋은 것\n## 좋았던 점\n\n마지막 줄에는 정확히 `판정: 통과` 또는 `판정: 수정 필요` 중 하나만 쓴다.",
        persona("minhyuk"),
        if focus.is_empty() { "(따로 없음)" } else { focus },
        finish_rule(safe, "minhyuk", true)
    )
}

fn verdict(text: &str) -> &'static str {
    if text.contains("판정: 수정 필요") || text.contains("판정:수정 필요") {
        "수정 필요"
    } else {
        "통과"
    }
}

/// 이 단계에서 AI가 할 일. 안전 모드의 지시서 준비와 실제 실행이 같은 계획을 쓴다.
pub fn plan(ctx: &Ctx, key: &str) -> Result<Plan> {
    let safe_for = |actor: &str| handoff_for(ctx, actor);
    let emp = ctx.employee_id.as_str();
    let request = ctx.s("request");
    let job = |actor: &str, cwd: PathBuf, prompt: String, level: Level, edits: bool, label: &str, team: &[&str]| {
        Plan::Ai(AiJob {
            actor: actor.into(),
            cwd,
            prompt,
            level,
            engine: ctx.settings.engine_for(actor),
            edits,
            label: label.into(),
            team: team.iter().map(|s| s.to_string()).collect(),
        })
    };
    let ws_path = || ctx.workspace().map(|ws| PathBuf::from(ws.path));

    let business = matches!((emp, key), ("harin", "write") | ("seojin", "analyze") | ("minjae", "research") | ("jiho", "write") | ("yena", "analyze"));
    if business {
        let (prompt, searches, _) = ai_request(ctx)?;
        let tail = if safe_for(emp) {
            format!("\n\n---\n{}\n그 밖의 파일은 만들거나 고치지 말고, 셸 명령도 실행하지 않는다.", finish_rule(true, emp, true))
        } else {
            "\n\n---\n파일을 만들거나 고치지 말고, 셸 명령도 실행하지 마세요. 최종 답변을 위 형식의 마크다운으로만 출력하세요.".into()
        };
        let dir = ctx.data_dir.join("runs").join(&ctx.task_id);
        // 웹 검색은 헤드리스에서 허락을 물을 수 없어, 빈 폴더 + 샌드박스 안에서만 자동 승인한다.
        let level = if searches > 0 { Level::Sandbox } else { Level::ReadOnly };
        return Ok(job(emp, dir, format!("{}\n\n{prompt}{tail}", persona(emp)), level, false, "AI 작업", &[emp]));
    }
    if !roster::DEV_TEAM.contains(&emp) {
        return Ok(Plan::Local);
    }

    match key {
        "team" => {
            let devs = ctx.s("devs");
            let sub = supports_subagents(ctx.settings.engine_for("taeo"));
            let who = |id: &str, name: &str| if sub { format!("서브에이전트 ilson-{id}({name})에게") } else { format!("{name}의 역할로 직접") };
            let mut team = vec!["taeo"];
            let mut order = vec![format!("1. 설계: 저장소를 읽고 설계서(목표·관련 파일·변경 계획·수용 기준)를 `{PROGRESS_DIR}/plan.md`에 쓴다.")];
            let mut n = 2;
            if devs != "backend" {
                team.push("yuna");
                order.push(format!("{n}. 화면 개발: {} 설계서의 화면 부분을 구현한다.", who("yuna", "유나")));
                n += 1;
            }
            if devs != "frontend" {
                team.push("geonwoo");
                order.push(format!("{n}. 서버·로직 개발: {} 설계서의 서버·로직 부분을 구현한다.", who("geonwoo", "건우")));
                n += 1;
            }
            if ctx.b("withTests", true) {
                team.push("sohee");
                order.push(format!("{n}. 테스트: {} 테스트를 작성하고 실행한다.", who("sohee", "소희")));
                n += 1;
            }
            team.push("minhyuk");
            order.push(format!("{n}. 리뷰: {} 변경을 리뷰하고, ‘반드시 고칠 것’이 있으면 반영한다.", who("minhyuk", "민혁")));
            n += 1;
            order.push(format!("{n}. 마무리: 전체 결과를 아래 형식으로 `{PROGRESS_DIR}/report-taeo.md`에 쓴다. 이 파일이 생기면 회사가 업무를 마친 것으로 본다."));
            let prompt = format!(
                "# 업무 지시서 — {}\n\n{}\n\n## 요청\n{request}\n\n## 진행 순서\n{}\n\n## 진행 표시\n각 직원은 일을 시작할 때 `{PROGRESS_DIR}/status-<이름>.txt`에 지금 하는 일을 한 줄로 쓰고, 끝나면 `{PROGRESS_DIR}/report-<이름>.md`에 보고서를 쓴다. (이름: {})\n\n{}",
                ctx.title,
                persona("taeo"),
                order.join("\n"),
                team.join(", "),
                rules(ctx.b("allowRun", false), true, "taeo")
            );
            let team: Vec<&str> = if sub { team } else { vec!["taeo"] };
            Ok(job("taeo", ws_path()?, prompt, run_mode(ctx, true), true, "팀 개발", &team))
        }
        "plan" => {
            let devs = match ctx.s("devs").as_str() {
                "frontend" => "유나(화면)만",
                "backend" => "건우(서버·로직)만",
                _ => "유나(화면)와 건우(서버·로직) 둘 다",
            };
            let prompt = format!(
                "{}\n아래 요청을 이 저장소에서 구현하기 위한 설계서를 쓴다. 코드를 읽어 구조를 파악하되 파일은 고치지 않는다. 이번에 일할 개발자: {devs}.\n\n[요청]\n{request}\n\n{}\n\n# 설계: (제목)\n## 목표\n## 현재 구조 (관련 파일)\n## 변경 계획\n### 화면 — 유나 (해당 없으면 '없음')\n### 서버·로직 — 건우 (해당 없으면 '없음')\n## 수용 기준\n- [ ] …\n## 확인 필요",
                persona("taeo"),
                finish_rule(safe_for("taeo"), "taeo", true)
            );
            Ok(job("taeo", ws_path()?, prompt, Level::ReadOnly, false, "기획", &["taeo"]))
        }
        "frontend" | "backend" | "build" => {
            let actor = match (key, emp) {
                ("frontend", _) => "yuna",
                ("backend", _) => "geonwoo",
                (_, "geonwoo") => "geonwoo",
                _ => "yuna",
            };
            let devs = ctx.s("devs");
            if (key == "frontend" && devs == "backend") || (key == "backend" && devs == "frontend") {
                return Ok(Plan::Skip("이번 업무에서는 맡지 않았습니다."));
            }
            let part = if actor == "yuna" { "화면 — 유나" } else { "서버·로직 — 건우" };
            let plan = plan_text(ctx);
            let plan_block = if plan.is_empty() { String::new() } else { format!("\n[태오의 설계서 — ‘{part}’ 부분을 맡는다]\n{plan}\n") };
            // 안전 모드 개인 업무는 한 번의 실행 안에서 민혁에게 리뷰까지 받는다.
            let sub = supports_subagents(ctx.settings.engine_for(actor));
            let review = match (is_safe(ctx) && key == "build", sub) {
                (true, true) => "\n- 구현을 마치면 서브에이전트 ilson-minhyuk(민혁)에게 리뷰를 맡기고, ‘반드시 고칠 것’을 반영한 뒤 보고서를 쓴다.",
                (true, false) => "\n- 구현을 마치면 코드 리뷰어의 눈으로 변경을 다시 읽고(버그·보안·가독성), 고칠 점을 반영한 뒤 보고서의 ‘남은 문제’에 리뷰 결과를 적는다.",
                _ => "",
            };
            let prompt = format!("{}\n\n[요청]\n{request}\n{plan_block}\n{}{review}", persona(actor), rules(ctx.b("allowRun", false), safe_for(actor), actor));
            let team: &[&str] = if is_safe(ctx) && key == "build" && sub { &[actor, "minhyuk"] } else { &[actor] };
            Ok(job(actor, ws_path()?, prompt, run_mode(ctx, true), true, if actor == "yuna" { "화면 개발" } else { "서버·로직 개발" }, team))
        }
        "test" => {
            if emp == "taeo" && !ctx.b("withTests", true) {
                return Ok(Plan::Skip("이번 업무에서는 테스트를 맡기지 않았습니다."));
            }
            let ws = ctx.workspace()?;
            let changed = gitops::diff(&ws).map(|d| d.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
            let prompt = format!(
                "{}\n이번 작업에 대한 테스트를 작성하거나 보강한다. 명령 실행이 허용되면 테스트를 실행해 결과를 확인한다. 실패한 테스트를 지우거나 건너뛰게 고치지 않는다. 테스트 도구가 없으면 프로젝트에 맞는 가장 가벼운 방법을 쓰고 그 이유를 적는다.\n\n[요청]\n{request}\n\n[이번에 바뀐 파일]\n{}\n\n{}\n\n보고서의 ‘한 일’에는 추가한 테스트를, ‘확인 방법’에는 실행 결과(통과/실패 개수)를 적는다.",
                persona("sohee"),
                if changed.is_empty() { "(아직 없음 — 요청 범위의 기존 코드를 테스트)".to_string() } else { changed },
                rules(ctx.b("allowRun", false), safe_for("sohee"), "sohee")
            );
            Ok(job("sohee", PathBuf::from(ws.path), prompt, run_mode(ctx, true), true, "테스트", &["sohee"]))
        }
        "docs" => {
            let prompt = format!("{}\n코드를 읽고, 코드에 실제로 있는 내용만 문서로 쓴다.\n\n[요청]\n{request}\n\n{}", persona("daeun"), rules(false, safe_for("daeun"), "daeun"));
            Ok(job("daeun", ws_path()?, prompt, Level::Edit, true, "문서", &["daeun"]))
        }
        "review" if emp == "minhyuk" => {
            let (repo, patch) = gitops::review_target(&ctx.s("folder"), &ctx.s("scope"))?;
            // 안전 모드에서는 사장님 저장소에 관리용 파일을 남기지 않도록 업무 폴더에서 실행한다.
            let safe = safe_for("minhyuk");
            let cwd = if safe { ctx.data_dir.join("runs").join(&ctx.task_id) } else { PathBuf::from(&repo) };
            let mut prompt = review_prompt(&ctx.s("focus"), &patch, safe);
            if safe {
                prompt.push_str(&format!("\n\n원본 저장소는 `{repo}`에 있다. 맥락이 필요하면 그곳의 파일을 읽기만 한다."));
            }
            Ok(job("minhyuk", cwd, prompt, Level::ReadOnly, false, "코드 리뷰", &["minhyuk"]))
        }
        "review" => {
            let ws = ctx.workspace()?;
            let diff = gitops::diff(&ws)?;
            if diff.files.is_empty() {
                return Ok(Plan::Skip("바뀐 코드가 없어 리뷰를 건너뛰었습니다."));
            }
            let mut patch = diff.patch.clone();
            if diff.truncated {
                patch.push_str("\n…(diff가 길어 일부만 보냅니다. 필요한 파일은 직접 읽어 확인할 것)");
            }
            Ok(job("minhyuk", PathBuf::from(ws.path), review_prompt(&request, &patch, safe_for("minhyuk")), Level::ReadOnly, false, "코드 리뷰", &["minhyuk"]))
        }
        "fix" => {
            let review = ctx.out("review");
            if review["verdict"] != "수정 필요" {
                return Ok(Plan::Skip("리뷰를 통과해 반영할 것이 없습니다."));
            }
            let actor = dev_for_fix(ctx);
            let prompt = format!(
                "{}\n코드 리뷰어 민혁의 의견 중 ‘반드시 고칠 것’을 반영한다. 동의하지 않는 지적은 고치지 말고 보고서의 ‘남은 문제’에 이유를 적는다.\n\n[요청]\n{request}\n\n[민혁의 리뷰]\n{}\n\n{}",
                persona(actor),
                review["markdown"].as_str().unwrap_or(""),
                rules(ctx.b("allowRun", false), safe_for(actor), actor)
            );
            Ok(job(actor, ws_path()?, prompt, run_mode(ctx, true), true, "리뷰 반영", &[actor]))
        }
        _ => Ok(Plan::Local),
    }
}

// ───────────────────────── 안전 모드: 지시서 준비와 관찰 ─────────────────────────

fn agent_tools(id: &str, edits: bool, lead: bool) -> Vec<&'static str> {
    let mut tools = vec!["view_file", "list_dir", "find_by_name", "grep_search", "write_to_file"];
    if roster::DEV_TEAM.contains(&id) {
        if edits {
            tools.extend(["replace_file_content", "multi_replace_file_content", "run_command"]);
        }
        if lead {
            tools.push("invoke_subagent");
        }
    } else {
        tools.extend(["search_web", "read_url_content"]);
    }
    tools
}

/// lead: 이번 업무를 이끄는 직원 (팀원을 서브에이전트로 부를 수 있다).
fn agent_file(id: &str, team: &[String], lead: bool) -> String {
    let e = roster::find(id).unwrap_or(Value::Null);
    let edits = !matches!(id, "minhyuk") && roster::DEV_TEAM.contains(&id);
    let lead = lead && team.len() > 1;
    let tools = agent_tools(id, edits, lead).iter().map(|t| format!("  - {t}")).collect::<Vec<_>>().join("\n");
    let subagents: Vec<String> = team.iter().filter(|t| t.as_str() != id).map(|t| format!("ilson-{t}.md")).collect();
    let agents = if lead && !subagents.is_empty() {
        format!("agents:\n{}\n", subagents.iter().map(|a| format!("  - {a}")).collect::<Vec<_>>().join("\n"))
    } else {
        String::new()
    };
    let body = if roster::DEV_TEAM.contains(&id) {
        rules(id != "daeun" && id != "minhyuk", true, id)
    } else {
        finish_rule(true, id, true)
    };
    format!(
        "---\nname: ilson-{id}\ndescription: 일손의 {} {}. {}\ntools:\n{tools}\nmodel: inherit\nmainAgent: true\nsubagent: true\ncommandExecutionPolicy: sandbox\n{agents}---\n# System Prompt\n{}\n\n{body}\n",
        e["role"].as_str().unwrap_or(""),
        e["name"].as_str().unwrap_or(id),
        e["summary"].as_str().unwrap_or(""),
        persona(id)
    )
}

/// Claude Code 서브에이전트 정의 (.claude/agents/ilson-*.md).
fn claude_agent_file(id: &str) -> String {
    let e = roster::find(id).unwrap_or(Value::Null);
    let tools = if id == "minhyuk" || !roster::DEV_TEAM.contains(&id) { "Read, Grep, Glob, Write, WebSearch, WebFetch" } else { "Read, Grep, Glob, Write, Edit, MultiEdit, Bash" };
    let body = if roster::DEV_TEAM.contains(&id) { rules(id != "daeun" && id != "minhyuk", true, id) } else { finish_rule(true, id, true) };
    format!(
        "---\nname: ilson-{id}\ndescription: 일손의 {} {}. {}\ntools: {tools}\nmodel: inherit\n---\n{}\n\n{body}\n",
        e["role"].as_str().unwrap_or(""),
        e["name"].as_str().unwrap_or(id),
        e["summary"].as_str().unwrap_or(""),
        persona(id)
    )
}

/// OpenCode 에이전트 정의 (.opencode/agents/ilson-*.md).
fn opencode_agent_file(id: &str, team: &[String], lead: bool, model: &str, allow_run: bool) -> String {
    let e = roster::find(id).unwrap_or(Value::Null);
    let is_dev = roster::DEV_TEAM.contains(&id);
    let can_edit = is_dev && id != "minhyuk";
    let can_bash = can_edit && allow_run && id != "daeun";
    let can_web = !is_dev;
    let mode = if lead { "primary" } else { "subagent" };
    let mut body = if is_dev {
        rules(can_bash, true, id)
    } else {
        finish_rule(true, id, true)
    };
    if lead && team.len() > 1 {
        body.push_str("\n\n팀원은 `@ilson-<id>`로 부른다.");
    }
    format!(
        "---\ndescription: 일손의 {} {}. {}\nmode: {mode}\nmodel: {model}\npermission:\n  edit: {}\n  bash: {}\n  webfetch: {}\n  websearch: {}\n  external_directory: deny\n---\n{}\n\n{body}\n",
        e["role"].as_str().unwrap_or(""),
        e["name"].as_str().unwrap_or(id),
        e["summary"].as_str().unwrap_or(""),
        if can_edit { "allow" } else { "deny" },
        if can_bash { "allow" } else { "deny" },
        if can_web { "allow" } else { "deny" },
        if can_web { "allow" } else { "deny" },
        persona(id)
    )
}

/// 안전 모드: 에이전트 정의와 지시서를 작업 폴더에 쓰고, 이전 보고서는 치운다.
fn prepare_handoff(ctx: &Ctx, job: &AiJob) -> Result<()> {
    let progress = job.cwd.join(PROGRESS_DIR);
    std::fs::create_dir_all(&progress)?;
    let mut team = job.team.clone();
    if !team.contains(&job.actor) {
        team.insert(0, job.actor.clone());
    }
    match job.engine {
        Kind::Agy => {
            let agents_dir = job.cwd.join(".agents").join("agents");
            std::fs::create_dir_all(&agents_dir)?;
            for id in &team {
                std::fs::write(agents_dir.join(format!("ilson-{id}.md")), agent_file(id, &team, *id == job.actor))?;
            }
        }
        Kind::Claude if team.len() > 1 => {
            let agents_dir = job.cwd.join(".claude").join("agents");
            std::fs::create_dir_all(&agents_dir)?;
            for id in team.iter().filter(|t| **t != job.actor) {
                std::fs::write(agents_dir.join(format!("ilson-{id}.md")), claude_agent_file(id))?;
            }
        }
        Kind::Opencode => {
            let agents_dir = job.cwd.join(".opencode").join("agents");
            std::fs::create_dir_all(&agents_dir)?;
            let model = ctx
                .settings
                .model_for(Kind::Opencode)
                .unwrap_or_else(|| engine::OPENCODE_DEFAULT_MODEL.to_string());
            let allow_run = ctx.b("allowRun", false);
            for id in &team {
                std::fs::write(
                    agents_dir.join(format!("ilson-{id}.md")),
                    opencode_agent_file(id, &team, *id == job.actor, &model, allow_run),
                )?;
            }
        }
        _ => {}
    }
    let mut prompt = job.prompt.clone();
    if job.engine == Kind::Opencode && team.len() > 1 {
        prompt.push_str("\n\n팀원은 `@ilson-<id>`로 부른다.");
    }
    std::fs::write(progress.join("TASK.md"), &prompt)?;
    for entry in std::fs::read_dir(&progress)? {
        let p = entry?.path();
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name.starts_with("report-") || name.starts_with("status-") {
            let _ = std::fs::remove_file(&p);
        }
    }
    Ok(())
}

const START_LINE: &str = ".ilson/TASK.md 파일의 지시를 처음부터 끝까지 그대로 수행해 주세요.";

fn handoff_gate(ctx: &Ctx, key: &str) -> Result<Option<Gate>> {
    let Plan::Ai(job) = plan(ctx, key)? else { return Ok(None) };
    prepare_handoff(ctx, &job)?;
    let folder = job.cwd.to_string_lossy().to_string();
    let agent = format!("ilson-{}", job.actor);
    let name = name_of(&job.actor);
    let kind = job.engine;
    let say = if kind == Kind::Agy { format!("{agent} 에이전트로 {START_LINE}") } else { START_LINE.to_string() };
    let mut details = if kind == Kind::Agy {
        vec![
            "1. ‘Antigravity 열기’로 작업 폴더를 엽니다 (터미널을 쓴다면 ‘터미널 명령 복사’).".to_string(),
            format!("2. 에이전트에게 이렇게 말합니다: “{say}”"),
        ]
    } else if kind == Kind::Opencode {
        vec![
            "1. ‘터미널 열기’ → ‘터미널 명령 복사’(OpenCode 화면이 열리고 지시가 자동 입력됩니다).".to_string(),
            "2. OpenCode 터미널에 복사한 명령을 붙여 넣고 엔터를 누릅니다.".to_string(),
        ]
    } else {
        vec![
            format!("1. ‘터미널 열기’로 작업 폴더에서 터미널을 엽니다."),
            format!("2. ‘터미널 명령 복사’로 {}을 실행하는 명령을 붙여 넣습니다.", kind.label()),
        ]
    };
    details.push("3. 끝나면 앱이 보고서를 확인하고 자동으로 다음 단계로 넘어갑니다.".into());
    if job.team.len() > 1 {
        let others: Vec<String> = job.team.iter().filter(|t| **t != job.actor).map(|t| name_of(t)).collect();
        details.push(format!("{name}가 {}에게 서브에이전트로 일을 나눠 맡깁니다.", others.join(", ")));
    }
    details.push(format!("앱은 이 단계에서 {}를 직접 실행하지 않습니다. 사장님 {}으로 실행됩니다.", kind.label(), kind.plan()));
    Ok(Some(Gate {
        kind: "handoff".into(),
        key: format!("handoff:{key}"),
        title: format!("{name}에게 맡길 준비가 됐어요"),
        summary: format!("{}에서 실행해 주세요 · {}", kind.label(), job.label),
        details,
        estimate_krw: None,
        max_krw: None,
        folder: Some(folder.clone()),
        agent: Some(agent.clone()),
        command: Some(engine::handoff_command(kind, &folder, &agent, START_LINE)),
        prompt: Some(say),
        engine: Some(kind.id().into()),
    }))
}

fn mtime(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// 안전 모드: 사장님이 Antigravity에서 실행한 일을 지켜보다가 보고서가 생기면 돌아온다.
async fn watch_external(ctx: &Ctx, job: &AiJob) -> Result<(String, String)> {
    let progress = job.cwd.join(PROGRESS_DIR);
    let report = progress.join(format!("report-{}.md", job.actor));
    let git = job.edits;
    let mut last_len: Option<u64> = None;
    let mut seen_status: HashMap<String, std::time::SystemTime> = HashMap::new();
    let mut seen_files: HashSet<String> = HashSet::new();
    let mut active = job.actor.clone();
    ctx.report(&job.actor, &Activity::new("thinking", format!("{}에서 시작하길 기다리는 중", job.engine.label()), false));
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        let forced = ctx.app.state::<AppState>().external_done.lock().unwrap().remove(&ctx.task_id);
        // 직원별 상태 파일: 가장 최근에 바뀐 사람이 지금 일하는 사람이다.
        if let Ok(entries) = std::fs::read_dir(&progress) {
            let mut newest: Option<(std::time::SystemTime, String)> = None;
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let Some(id) = name.strip_prefix("status-").and_then(|n| n.strip_suffix(".txt")) else { continue };
                let Some(t) = mtime(&entry.path()) else { continue };
                if seen_status.get(id) != Some(&t) {
                    seen_status.insert(id.to_string(), t);
                    let line = std::fs::read_to_string(entry.path()).unwrap_or_default().lines().next().unwrap_or("").trim().chars().take(40).collect::<String>();
                    ctx.report(id, &Activity { kind: "typing".into(), label: if line.is_empty() { "작업 중".into() } else { line }, notable: false });
                }
                if newest.as_ref().is_none_or(|(nt, _)| t > *nt) {
                    newest = Some((t, id.to_string()));
                }
            }
            if let Some((_, id)) = newest {
                active = id;
            }
        }
        if git {
            if let Ok(files) = gitops::changed_files(&job.cwd) {
                for f in files {
                    if seen_files.insert(f.clone()) {
                        ctx.report(&active, &Activity { kind: "typing".into(), label: format!("고치는 중 · {}", f.rsplit('/').next().unwrap_or(&f)), notable: true });
                    }
                }
            }
        }
        let len = std::fs::metadata(&report).map(|m| m.len()).ok();
        let stable = len.is_some_and(|l| l > 0) && len == last_len;
        last_len = len;
        if stable || forced {
            break;
        }
    }
    let main = std::fs::read_to_string(&report).unwrap_or_else(|_| "(보고서 없이 사장님이 완료 처리했습니다.)".into());
    // 서브에이전트 보고서도 함께 모은다.
    let mut others = String::new();
    if let Ok(entries) = std::fs::read_dir(&progress) {
        let mut list: Vec<(String, PathBuf)> = entries
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                let id = n.strip_prefix("report-")?.strip_suffix(".md")?.to_string();
                (id != job.actor).then(|| (id, e.path()))
            })
            .collect();
        list.sort();
        for (id, p) in list {
            if let Ok(t) = std::fs::read_to_string(&p) {
                others.push_str(&format!("\n\n## {} 보고서\n\n{}", name_of(&id), t.trim()));
            }
        }
        if let Ok(plan) = std::fs::read_to_string(progress.join("plan.md")) {
            others.insert_str(0, &format!("\n\n## 설계서 · 태오\n\n{}", plan.trim()));
        }
    }
    Ok((format!("{}{}", main.trim(), others), format!("{}(사장님 구독)", job.engine.label())))
}

// ───────────────────────── 실행 ─────────────────────────

struct JobOut {
    markdown: String,
    model: Option<String>,
    note: String,
    cost_usd: f64,
}

async fn run_job(ctx: &Ctx, step_key: &str, job: &AiJob) -> Result<JobOut> {
    let state = ctx.app.state::<AppState>();
    let prompt_chars = job.prompt.chars().count() as u64;
    let est_input = crate::usage::estimate_prompt_tokens(job.engine, &job.prompt);

    if handoff_for(ctx, &job.actor) {
        let (markdown, model) = watch_external(ctx, job).await?;
        if let Ok(conn) = state.db.lock() {
            let _ = crate::usage::record(
                &conn,
                ctx,
                step_key,
                job,
                &crate::usage::Usage::default(),
                ctx.settings.model_for(job.engine).as_deref(),
                "safe",
                Some(est_input),
                Some(prompt_chars),
                "safe",
            );
        }
        return Ok(JobOut {
            markdown,
            note: format!("{model}에서 실행"),
            model: Some(model),
            cost_usd: 0.0,
        });
    }
    // 자동 모드: 한 번에 한 업무만 AI 엔진을 실행한다.
    let _turn = state.engine_lock.lock().await;
    let actor = job.actor.clone();
    // YOLO 스위치가 켜져 있으면 파일을 고치는 단계는 모든 확인을 생략한다.
    let level = if ctx.settings.yolo && job.level != Level::ReadOnly { Level::Yolo } else { job.level };
    let configured_model = ctx.settings.model_for(job.engine);
    let r = engine::run(
        engine::RunOptions {
            kind: job.engine,
            cwd: &job.cwd,
            prompt: job.prompt.clone(),
            model: configured_model.clone(),
            level,
            title: Some(ctx.title.clone()),
        },
        |a| ctx.report(&actor, &a),
    )
    .await?;

    let (eff_model, model_source) = if let Some(ref m) = r.model {
        let src = if job.engine == Kind::Opencode {
            if ctx.settings.models.get("opencode").is_some_and(|s| !s.trim().is_empty()) {
                "settings"
            } else {
                "default"
            }
        } else {
            "reported"
        };
        (Some(m.clone()), src)
    } else if let Some(ref cm) = configured_model {
        (Some(cm.clone()), "settings")
    } else {
        (
            Some(crate::pricing::default_model_for(job.engine.id()).to_string()),
            "default",
        )
    };

    let usage = crate::usage::Usage::from(&r);
    let cost_usd = if let Ok(conn) = state.db.lock() {
        crate::usage::record(
            &conn,
            ctx,
            step_key,
            job,
            &usage,
            eff_model.as_deref(),
            model_source,
            Some(est_input),
            Some(prompt_chars),
            "auto",
        )
        .unwrap_or(0.0)
    } else {
        0.0
    };

    let mut note = format!(
        "{} {} · {}",
        job.engine.label(),
        eff_model.clone().unwrap_or_default(),
        level.label()
    );
    if !r.denied.is_empty() {
        note.push_str(&format!(" · 거부된 동작: {}", r.denied.join(", ")));
    }
    Ok(JobOut {
        markdown: r.response.trim().to_string(),
        model: eff_model,
        note,
        cost_usd,
    })
}

/// AI 단계를 실행하고 결과를 정리한다 (커밋, 리뷰 판정 등).
async fn ai_step(ctx: &Ctx, key: &str, job: AiJob) -> Result<StepResult> {
    std::fs::create_dir_all(&job.cwd)?;
    let out = run_job(ctx, key, &job).await?;
    if out.markdown.trim().is_empty() {
        bail!("AI가 빈 결과를 돌려주었습니다. 이어하기로 다시 시도해 주세요.");
    }
    let mut note = out.note;
    let mut output = json!({ "markdown": out.markdown, "actor": job.actor, "model": out.model });
    if job.edits {
        let ws = ctx.workspace()?;
        match gitops::commit_all(&ws, &format!("{}: {} ({})", job.label, ctx.title, name_of(&job.actor)))? {
            Some(c) => {
                note.push_str(&format!(" · 커밋 {c}"));
                output["commit"] = json!(c);
            }
            None => note.push_str(" · 바뀐 파일 없음"),
        }
    }
    if key == "review" || key == "team" || (key == "build" && is_safe(ctx)) {
        let v = verdict(&out.markdown);
        if key == "review" || out.markdown.contains("판정:") {
            output["verdict"] = json!(v);
            note = format!("판정: {v} · {note}");
        }
    }
    Ok(StepResult { output, note: Some(note), cost_usd: out.cost_usd })
}

// ───────────────────────── 확인(게이트) ─────────────────────────

/// 이 단계 앞에서 받아야 하는 확인. (kind, 확인 키)
pub fn gate_kind(ctx: &Ctx, step_key: &str, static_gate: Option<&str>) -> Result<Option<String>> {
    if let Some(g) = static_gate {
        return Ok(Some(format!("{g}:{step_key}")));
    }
    if let Plan::Ai(job) = plan(ctx, step_key)? {
        if handoff_for(ctx, &job.actor) {
            return Ok(Some(format!("handoff:{step_key}")));
        }
    }
    Ok(None)
}

pub fn gate(ctx: &Ctx, step_key: &str, gate_key: &str) -> Result<Option<Gate>> {
    let mut g = match gate_key.split(':').next().unwrap_or("") {
        "apply" => organize_gate(ctx),
        "merge" => merge_gate(ctx)?,
        "handoff" => handoff_gate(ctx, step_key)?,
        _ => None,
    };
    if let Some(g) = g.as_mut() {
        g.key = gate_key.to_string();
    }
    Ok(g)
}

fn merge_gate(ctx: &Ctx) -> Result<Option<Gate>> {
    let ws = ctx.workspace()?;
    let diff = gitops::diff(&ws)?;
    if diff.files.is_empty() {
        return Ok(None);
    }
    let mut details: Vec<String> = diff.files.iter().take(12).map(|f| format!("{} (+{} −{})", f.path, f.added, f.removed)).collect();
    if diff.files.len() > 12 {
        details.push(format!("…외 {}개 파일", diff.files.len() - 12));
    }
    let verdict = ["review", "team", "build"].iter().find_map(|k| ctx.outputs.get(*k).and_then(|r| r["verdict"].as_str()).map(String::from));
    if let Some(v) = verdict {
        details.push(format!("민혁의 리뷰 판정: {v}"));
    }
    details.push("‘변경 내용 보기’에서 전체 diff를, ‘작업 폴더 열기’에서 실제 파일을 확인할 수 있습니다.".into());
    Ok(Some(Gate {
        kind: "merge".into(),
        key: String::new(),
        title: "팀이 만든 변경을 병합할까요?".into(),
        summary: format!("파일 {}개 · +{} −{} · {} → {}", diff.files.len(), diff.added, diff.removed, ws.branch, ws.base_branch),
        details,
        estimate_krw: None,
        max_krw: None,
        folder: Some(ws.path.clone()),
        agent: None,
        command: None,
        prompt: None,
        engine: None,
    }))
}

// ───────────────────────── 개발팀 결과 ─────────────────────────

fn code_report(ctx: &Ctx) -> Value {
    let mut md = format!("# {}\n\n", ctx.title);
    let mut sections = vec![];
    for (key, title) in [
        ("team", "팀 보고 · 태오"),
        ("plan", "설계 · 태오"),
        ("frontend", "화면 개발 · 유나"),
        ("backend", "서버·로직 개발 · 건우"),
        ("build", ""),
        ("test", "테스트 · 소희"),
        ("docs", "문서 · 다은"),
        ("review", "코드 리뷰 · 민혁"),
        ("fix", "리뷰 반영"),
    ] {
        let out = ctx.out(key);
        if out.is_null() || out["skipped"] == true {
            continue;
        }
        let Some(text) = out["markdown"].as_str().filter(|t| !t.trim().is_empty()) else { continue };
        let heading = if key == "build" { format!("개발 · {}", name_of(&ctx.employee_id)) } else { title.to_string() };
        sections.push(heading.clone());
        // 각자 쓴 보고서의 제목 단계를 한 단계 내려 한 문서로 묶는다.
        let body: String = text.lines().map(|l| if l.starts_with('#') { format!("#{l}") } else { l.to_string() }).collect::<Vec<_>>().join("\n");
        md.push_str(&format!("## {heading}\n\n{body}\n\n"));
    }
    let merge = ctx.out("merge");
    let verdict = ["review", "team", "build"].iter().find_map(|k| ctx.outputs.get(*k).and_then(|r| r["verdict"].as_str()).map(String::from));
    json!({
        "kind": "code",
        "title": ctx.title,
        "markdown": md,
        "sections": sections,
        "merged": merge["merged"],
        "mergeCommit": merge["commit"],
        "files": merge["files"],
        "added": merge["added"],
        "removed": merge["removed"],
        "commits": merge["commits"],
        "verdict": verdict,
        "workspace": ctx.out("setup")["workspace"],
    })
}

async fn local_coding_step(ctx: &Ctx, key: &str) -> Result<StepResult> {
    match key {
        "setup" => {
            let ws = gitops::prepare(&ctx.s("folder"), &ctx.data_dir.join("worktrees"), &ctx.short_id(), ctx.b("allowInit", true))?;
            let mut note = format!("작업 공간을 만들었습니다 · 브랜치 {} (기준 {})", ws.branch, ws.base_branch);
            if ws.initialized {
                note.push_str(" · 새 저장소를 만들고 현재 상태를 첫 커밋으로 기록했습니다");
            }
            if ws.dirty_main {
                note.push_str(" · 원래 폴더의 커밋하지 않은 변경은 이번 작업에 포함되지 않습니다");
            }
            Ok(StepResult::new(json!({ "workspace": ws }), note))
        }
        "merge" => {
            let ws = ctx.workspace()?;
            let diff = gitops::diff(&ws)?;
            let commits = gitops::log(&ws).unwrap_or_default();
            if diff.files.is_empty() {
                gitops::cleanup(&ws, true);
                return Ok(StepResult::new(
                    json!({ "merged": false, "files": [], "commits": [] }),
                    "바뀐 파일이 없어 병합할 것이 없습니다. 작업 공간을 정리했습니다.",
                ));
            }
            let merged = gitops::merge(&ws, &ctx.title)?;
            Ok(StepResult::new(
                json!({ "merged": true, "commit": merged, "files": diff.files, "added": diff.added, "removed": diff.removed, "commits": commits }),
                format!("‘{}’ 브랜치에 병합했습니다 · 커밋 {merged}", ws.base_branch),
            ))
        }
        other => Err(anyhow!("알 수 없는 개발 단계입니다: {other}")),
    }
}

// ───────────────────────── 단계 실행 ─────────────────────────

pub async fn execute(ctx: &Ctx, step_key: &str) -> Result<StepResult> {
    match plan(ctx, step_key)? {
        Plan::Skip(note) => return Ok(skipped(note)),
        Plan::Ai(job) => return ai_step(ctx, step_key, job).await,
        Plan::Local => {}
    }
    match (ctx.employee_id.as_str(), step_key) {
        ("harin", "prepare") => {
            let keyword = ctx.s("keyword");
            if keyword.is_empty() {
                bail!("핵심 키워드를 입력해 주세요.");
            }
            Ok(StepResult::new(
                json!({ "keyword": keyword }),
                format!("키워드 ‘{keyword}’로 약 {}자 초안을 준비합니다.", won(ctx.n("length", 1500))),
            ))
        }
        ("harin", "review") => Ok(blog_review(ctx)),
        ("jiho", "prepare") => {
            if ctx.s("topic").is_empty() {
                bail!("주제를 입력해 주세요.");
            }
            Ok(StepResult::new(json!({}), format!("{}안을 {}초 분량으로 준비합니다.", ctx.n("variants", 3), ctx.n("duration", 30))))
        }
        ("minjae", "prepare") => {
            if ctx.s("offering").is_empty() || ctx.s("target").is_empty() {
                bail!("파는 것과 찾는 고객을 입력해 주세요.");
            }
            let note = if ctx.n("webSearch", 0) > 0 {
                "공개 웹 자료로 후보를 찾습니다. 연락은 자동으로 보내지 않습니다."
            } else {
                "웹 검색 없이 후보 유형과 찾는 방법을 정리합니다."
            };
            Ok(StepResult::new(json!({}), note))
        }
        ("seojin", "collect") => collect_pages(ctx, "urls", 8, None).await,
        ("yena", "collect") => collect_pages(ctx, "noticeUrls", 5, Some("noticeText")).await,
        ("doyun", "compute") => quote_compute(ctx),
        ("sua", "scan") => organize_scan(ctx),
        ("sua", "plan") => organize_plan(ctx),
        ("sua", "apply") => organize_apply(ctx),
        (emp, key) if key != "deliver" && roster::DEV_TEAM.contains(&emp) => local_coding_step(ctx, key).await,
        (_, "deliver") => deliver(ctx),
        (emp, key) => Err(anyhow!("알 수 없는 단계입니다: {emp}/{key}")),
    }
}


async fn collect_pages(ctx: &Ctx, url_field: &str, max: usize, text_field: Option<&str>) -> Result<StepResult> {
    let mut urls = web::parse_urls(&ctx.s(url_field));
    let pasted = text_field.map(|f| ctx.s(f)).unwrap_or_default();
    let searches = ctx.n("webSearch", 0);
    if urls.is_empty() && pasted.is_empty() && searches == 0 {
        bail!(if text_field.is_some() {
            "공고 링크나 공고 본문 중 하나를 넣어 주세요."
        } else {
            "참고 링크를 넣거나 웹 검색을 켜 주세요."
        });
    }
    let mut notes = vec![];
    if urls.len() > max {
        notes.push(format!("링크가 {}개라 앞의 {max}개만 읽었습니다", urls.len()));
        urls.truncate(max);
    }
    let mut pages = vec![];
    let mut failures = vec![];
    for url in &urls {
        match web::fetch(&ctx.http, url).await {
            Ok(p) => {
                if p.truncated {
                    notes.push(format!(
                        "‘{}’은 {}자로 길어서 앞 {}자만 썼습니다",
                        p.title,
                        won(p.original_chars as i64),
                        won(web::MAX_CHARS_PER_PAGE as i64)
                    ));
                }
                pages.push(p);
            }
            Err(e) => failures.push(json!({ "url": url, "error": e.to_string() })),
        }
    }
    if pages.is_empty() && pasted.is_empty() && searches == 0 {
        bail!("링크를 하나도 읽지 못했습니다. 주소를 확인하거나 본문을 붙여 넣어 주세요. ({})", failures.len());
    }
    let mut note = if urls.is_empty() {
        "넣어 주신 링크가 없어 웹 검색으로 조사합니다.".to_string()
    } else {
        format!("링크 {}개 중 {}개를 읽었습니다.", urls.len(), pages.len())
    };
    if !failures.is_empty() {
        note.push_str(&format!(" {}개는 열지 못했습니다.", failures.len()));
    }
    if !notes.is_empty() {
        note.push_str(&format!(" ({})", notes.join(" · ")));
    }
    Ok(StepResult::new(json!({ "pages": pages, "failures": failures, "noticeText": pasted }), note))
}

fn section<'a>(markdown: &'a str, heading: &str) -> &'a str {
    let marker = format!("## {heading}");
    let Some(start) = markdown.find(&marker) else { return "" };
    let rest = &markdown[start + marker.len()..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    &rest[..end]
}

fn blog_review(ctx: &Ctx) -> StepResult {
    let md = ctx.out("write")["markdown"].as_str().unwrap_or("");
    let body = section(md, "본문");
    let body_chars = body.chars().filter(|c| !c.is_whitespace() && *c != '#' && *c != '|').count();
    let keyword = ctx.s("keyword");
    let kw_count = if keyword.is_empty() { 0 } else { body.matches(keyword.as_str()).count() };
    let target = ctx.n("length", 1500);
    let title = md.lines().find(|l| l.starts_with("# ")).map(|l| l[2..].trim()).unwrap_or("");
    let unverified = md.matches("[확인 필요").count();
    let banned: Vec<String> = ctx
        .settings
        .banned_words
        .split([',', '\n'])
        .map(str::trim)
        .filter(|w| !w.is_empty() && md.contains(*w))
        .map(String::from)
        .collect();
    let checks = json!([
        { "label": "본문 분량", "value": format!("공백 제외 {}자 (목표 {}자)", won(body_chars as i64), won(target)), "ok": body_chars as i64 >= target * 6 / 10 },
        { "label": "핵심 키워드 반복", "value": format!("본문에 {kw_count}회"), "ok": (3..=9).contains(&kw_count) },
        { "label": "제목 길이", "value": format!("{}자", title.chars().count()), "ok": !title.is_empty() && title.chars().count() <= 40 },
        { "label": "확인이 필요한 사실", "value": if unverified == 0 { "없음".to_string() } else { format!("{unverified}곳 — 올리기 전 채워 주세요") }, "ok": unverified == 0 },
        { "label": "쓰지 말 표현", "value": if banned.is_empty() { "발견되지 않음".to_string() } else { banned.join(", ") }, "ok": banned.is_empty() }
    ]);
    let passed = checks.as_array().map(|a| a.iter().filter(|c| c["ok"] == true).count()).unwrap_or(0);
    StepResult::new(json!({ "checks": checks }), format!("점검 5개 중 {passed}개 통과"))
}

fn sources_markdown(sources: &[Value]) -> String {
    if sources.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\n## 출처\n");
    for (i, s) in sources.iter().enumerate() {
        out.push_str(&format!("{}. [{}]({})\n", i + 1, s["title"].as_str().unwrap_or(""), s["url"].as_str().unwrap_or("")));
    }
    out
}

fn first_heading(md: &str) -> Option<String> {
    md.lines().find(|l| l.starts_with("# ")).map(|l| l[2..].trim().to_string()).filter(|t| !t.is_empty())
}

fn deliver(ctx: &Ctx) -> Result<StepResult> {
    let result = match ctx.employee_id.as_str() {
        "taeo" | "yuna" | "geonwoo" | "sohee" | "daeun" => code_report(ctx),
        "minhyuk" => {
            let review = ctx.out("review");
            let md = review["markdown"].as_str().unwrap_or("").to_string();
            json!({
                "kind": "document",
                "title": first_heading(&md).unwrap_or_else(|| ctx.title.clone()),
                "markdown": md,
                "meta": [
                    { "label": "검토 범위", "value": match ctx.s("scope").as_str() { "last1" => "최근 커밋 1개", "last3" => "최근 커밋 3개", _ => "커밋하지 않은 변경" } },
                    { "label": "판정", "value": review["verdict"] },
                ],
                "checks": null,
                "sources": [],
            })
        }
        "doyun" => {
            let q = ctx.out("compute").clone();
            json!({
                "kind": "quote",
                "title": format!("{} 견적서", q["client"]["name"].as_str().unwrap_or("")),
                "quote": q,
            })
        }
        "sua" => organize_report(ctx),
        emp => {
            let ai_key = match emp { "harin" | "jiho" => "write", "minjae" => "research", _ => "analyze" };
            let ai = ctx.out(ai_key);
            let mut md = ai["markdown"].as_str().unwrap_or("").to_string();
            // 수집 자료 번호([1]..)는 그대로 두고, 웹 검색 출처를 뒤에 잇는다.
            let sources: Vec<Value> = ctx.out("collect")["pages"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| json!({ "title": p["title"], "url": p["url"] }))
                .collect();
            md.push_str(&sources_markdown(&sources));
            let model_name = ai["model"].as_str().unwrap_or("Antigravity").to_string();
            let mut meta = vec![json!({ "label": "모델", "value": model_name })];
            match emp {
                "harin" => {
                    meta.insert(0, json!({ "label": "핵심 키워드", "value": ctx.s("keyword") }));
                    meta.insert(1, json!({ "label": "분량", "value": format!("약 {}자", won(ctx.n("length", 1500))) }));
                }
                "seojin" => meta.insert(0, json!({ "label": "조사 주제", "value": ctx.s("topic") })),
                "minjae" => meta.insert(0, json!({ "label": "찾는 고객", "value": ctx.s("target") })),
                "jiho" => meta.insert(0, json!({ "label": "주제", "value": ctx.s("topic") })),
                _ => {}
            }
            if !sources.is_empty() {
                meta.push(json!({ "label": "읽은 링크", "value": format!("{}개", sources.len()) }));
            }
            json!({
                "kind": "document",
                "title": first_heading(&md).unwrap_or_else(|| ctx.title.clone()),
                "markdown": md,
                "meta": meta,
                "checks": ctx.out("review")["checks"].clone(),
                "sources": sources,
                "truncated": ai["truncated"],
            })
        }
    };
    Ok(StepResult::new(result, "결과를 정리했습니다."))
}

// ───────────────────────── 견적서 (도윤) ─────────────────────────

pub fn korean_amount(mut n: u64) -> String {
    if n == 0 {
        return "영".into();
    }
    const DIGITS: [&str; 10] = ["", "일", "이", "삼", "사", "오", "육", "칠", "팔", "구"];
    const SMALL: [&str; 4] = ["천", "백", "십", ""];
    const BIG: [&str; 5] = ["", "만", "억", "조", "경"];
    let mut parts = vec![];
    let mut big = 0;
    while n > 0 {
        let chunk = n % 10_000;
        if chunk > 0 {
            let mut s = String::new();
            for (i, d) in [chunk / 1000, chunk / 100 % 10, chunk / 10 % 10, chunk % 10].into_iter().enumerate() {
                if d > 0 {
                    s.push_str(DIGITS[d as usize]);
                    s.push_str(SMALL[i]);
                }
            }
            s.push_str(BIG[big]);
            parts.push(s);
        }
        n /= 10_000;
        big += 1;
    }
    parts.reverse();
    parts.concat()
}

fn quote_compute(ctx: &Ctx) -> Result<StepResult> {
    let client = ctx.s("clientName");
    if client.is_empty() {
        bail!("받는 곳을 입력해 주세요.");
    }
    let s = &ctx.settings;
    if s.business_name.trim().is_empty() {
        bail!("공급자(우리 사업장) 상호가 없습니다. 연결 및 설정 > 사업장 정보에서 상호를 입력해 주세요.");
    }
    let mut items = vec![];
    for row in ctx.input["items"].as_array().into_iter().flatten() {
        let name = row["name"].as_str().unwrap_or("").trim().to_string();
        let spec = row["spec"].as_str().unwrap_or("").trim().to_string();
        let qty = row["qty"].as_f64().or_else(|| row["qty"].as_str().and_then(|v| v.parse().ok())).unwrap_or(1.0);
        let price = row["unitPrice"].as_f64().or_else(|| row["unitPrice"].as_str().and_then(|v| v.replace(',', "").parse().ok())).unwrap_or(0.0);
        if name.is_empty() && price == 0.0 {
            continue;
        }
        if qty < 0.0 || price < 0.0 {
            bail!("수량과 단가는 0 이상이어야 합니다 ({name}).");
        }
        let amount = (qty * price).round() as i64;
        items.push(json!({ "name": name, "spec": spec, "qty": qty, "unitPrice": price.round() as i64, "amount": amount }));
    }
    if items.is_empty() {
        bail!("작업 항목을 한 줄 이상 입력해 주세요.");
    }
    let sum: i64 = items.iter().map(|i| i["amount"].as_i64().unwrap_or(0)).sum();
    let vat_mode = ctx.s("vatMode");
    let (supply, vat, total) = match vat_mode.as_str() {
        "included" => {
            let supply = (sum as f64 / 1.1).round() as i64;
            (supply, sum - supply, sum)
        }
        "exempt" => (sum, 0, sum),
        _ => {
            let vat = (sum as f64 * 0.1).round() as i64;
            (sum, vat, sum + vat)
        }
    };
    let today = chrono::Local::now().date_naive();
    let date = chrono::NaiveDate::parse_from_str(&ctx.s("quoteDate"), "%Y-%m-%d").unwrap_or(today);
    let valid_days = ctx.n("validDays", 30).clamp(1, 365);
    let valid_until = date + chrono::Duration::days(valid_days);
    let number = format!("Q-{}-{}", date.format("%Y%m%d"), &uuid::Uuid::new_v4().simple().to_string()[..4].to_uppercase());
    let notes: Vec<String> = ctx
        .s("notes")
        .lines()
        .map(|l| l.trim().trim_start_matches(['-', '·', '*', '•']).trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let quote = json!({
        "number": number,
        "date": date.format("%Y-%m-%d").to_string(),
        "validUntil": valid_until.format("%Y-%m-%d").to_string(),
        "validDays": valid_days,
        "delivery": ctx.s("delivery"),
        "projectName": ctx.s("projectName"),
        "client": { "name": client, "contact": ctx.s("clientContact") },
        "supplier": {
            "name": s.business_name, "owner": s.owner_name, "bizRegNo": s.biz_reg_no,
            "address": s.address, "phone": s.phone, "email": s.email
        },
        "items": items,
        "vatMode": if vat_mode.is_empty() { "separate".to_string() } else { vat_mode },
        "supply": supply, "vat": vat, "total": total,
        "totalKorean": format!("일금 {}원정", korean_amount(total.max(0) as u64)),
        "notes": notes,
        "message": ctx.s("message"),
    });
    Ok(StepResult::new(quote, format!("항목 {}개 · 합계 {}원", items.len(), won(total))))
}

// ───────────────────────── 파일 정리 (수아) ─────────────────────────

fn category(ext: &str) -> &'static str {
    match ext {
        "pdf" | "doc" | "docx" | "hwp" | "hwpx" | "txt" | "rtf" | "odt" | "md" | "pages" => "문서",
        "xls" | "xlsx" | "csv" | "ods" | "numbers" | "ppt" | "pptx" | "key" | "odp" => "표·발표",
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "bmp" | "svg" | "tif" | "tiff" | "psd" | "ai" => "이미지",
        "mp4" | "mov" | "avi" | "mkv" | "wmv" | "webm" | "m4v" => "영상",
        "mp3" | "wav" | "m4a" | "aac" | "flac" | "ogg" => "음악·음성",
        "zip" | "rar" | "7z" | "tar" | "gz" | "alz" | "egg" => "압축",
        "exe" | "msi" | "dmg" | "pkg" | "apk" => "설치 파일",
        _ => "기타",
    }
}

fn check_folder(raw: &str) -> Result<PathBuf> {
    if raw.is_empty() {
        bail!("정리할 폴더를 골라 주세요.");
    }
    let path = Path::new(raw);
    if !path.is_dir() {
        bail!("폴더를 찾을 수 없습니다: {raw}");
    }
    let canon = path.canonicalize()?;
    let lower = canon.to_string_lossy().to_lowercase();
    let protected = ["\\windows", "\\program files", "\\programdata", "\\appdata", "/system", "/library", "/usr", "/bin", "/etc"];
    if canon.parent().is_none() || canon.parent().map(|p| p.parent().is_none()).unwrap_or(false) && lower.len() <= 4 {
        bail!("드라이브 전체는 정리할 수 없습니다. 하위 폴더를 골라 주세요.");
    }
    if protected.iter().any(|p| lower.contains(p)) {
        bail!("시스템 폴더는 정리하지 않습니다.");
    }
    Ok(path.to_path_buf())
}

fn organize_scan(ctx: &Ctx) -> Result<StepResult> {
    let folder = check_folder(&ctx.s("folder"))?;
    let mut files = vec![];
    let mut total: u64 = 0;
    for entry in std::fs::read_dir(&folder)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if !meta.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        if name.starts_with('.') || lower == "desktop.ini" || lower == "thumbs.db" || lower.ends_with(".lnk") || lower.ends_with(".url") {
            continue;
        }
        let ext = Path::new(&lower).extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
        let month = meta
            .modified()
            .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m").to_string())
            .unwrap_or_else(|_| "날짜 없음".into());
        total += meta.len();
        files.push(json!({ "name": name, "size": meta.len(), "month": month, "category": category(&ext) }));
    }
    let note = format!(
        "파일 {}개 ({:.1}MB)를 찾았습니다. 바로가기와 숨김 파일은 건드리지 않습니다.",
        files.len(),
        total as f64 / 1_048_576.0
    );
    Ok(StepResult::new(json!({ "folder": folder.to_string_lossy(), "files": files }), note))
}

fn organize_plan(ctx: &Ctx) -> Result<StepResult> {
    let scan = ctx.out("scan");
    let folder = PathBuf::from(scan["folder"].as_str().unwrap_or(""));
    let rule = ctx.s("rule");
    let mut planned: HashSet<PathBuf> = HashSet::new();
    let mut moves = vec![];
    let mut per_folder: HashMap<String, usize> = HashMap::new();
    for f in scan["files"].as_array().into_iter().flatten() {
        let name = f["name"].as_str().unwrap_or("");
        let cat = f["category"].as_str().unwrap_or("기타");
        let month = f["month"].as_str().unwrap_or("날짜 없음");
        let sub = match rule.as_str() {
            "month" => PathBuf::from(month),
            "type_month" => PathBuf::from(cat).join(month),
            _ => PathBuf::from(cat),
        };
        let dir = folder.join(&sub);
        let mut target = dir.join(name);
        let stem = Path::new(name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = Path::new(name).extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let mut n = 2;
        while target.exists() || planned.contains(&target) {
            target = dir.join(format!("{stem} ({n}){ext}"));
            n += 1;
        }
        planned.insert(target.clone());
        *per_folder.entry(sub.to_string_lossy().to_string()).or_default() += 1;
        moves.push(json!({
            "from": folder.join(name).to_string_lossy(),
            "to": target.to_string_lossy(),
            "label": format!("{name} → {}", target.strip_prefix(&folder).unwrap_or(&target).to_string_lossy()),
        }));
    }
    let mut folders: Vec<(String, usize)> = per_folder.into_iter().collect();
    folders.sort();
    let note = if moves.is_empty() {
        "옮길 파일이 없습니다.".to_string()
    } else {
        format!("파일 {}개를 폴더 {}개로 나누는 계획을 세웠습니다.", moves.len(), folders.len())
    };
    Ok(StepResult::new(json!({ "moves": moves, "folders": folders }), note))
}

fn organize_gate(ctx: &Ctx) -> Option<Gate> {
    let plan = ctx.out("plan");
    let moves = plan["moves"].as_array()?;
    if moves.is_empty() {
        return None;
    }
    let mut details: Vec<String> = moves.iter().take(10).map(|m| m["label"].as_str().unwrap_or("").to_string()).collect();
    if moves.len() > 10 {
        details.push(format!("…외 {}개", moves.len() - 10));
    }
    Some(Gate {
        kind: "apply".into(),
        title: "파일을 옮겨도 될까요?".into(),
        summary: format!(
            "파일 {}개를 폴더 {}개로 옮깁니다. 지우는 파일은 없고, 끝난 뒤 되돌릴 수 있습니다.",
            moves.len(),
            plan["folders"].as_array().map(|f| f.len()).unwrap_or(0)
        ),
        details,
        estimate_krw: None,
        max_krw: None,
        key: String::new(),
        folder: None,
        agent: None,
        command: None,
        prompt: None,
        engine: None,
    })
}

fn organize_apply(ctx: &Ctx) -> Result<StepResult> {
    let mut done = vec![];
    let (mut ok, mut skipped) = (0, 0);
    for m in ctx.out("plan")["moves"].as_array().into_iter().flatten() {
        let from = PathBuf::from(m["from"].as_str().unwrap_or(""));
        let to = PathBuf::from(m["to"].as_str().unwrap_or(""));
        let result = if !from.exists() {
            Err(anyhow!("원래 파일이 없습니다"))
        } else if to.exists() {
            Err(anyhow!("같은 이름의 파일이 이미 있습니다"))
        } else {
            to.parent()
                .map(std::fs::create_dir_all)
                .transpose()
                .and_then(|_| std::fs::rename(&from, &to))
                .map_err(|e| anyhow!(e))
        };
        match result {
            Ok(()) => {
                ok += 1;
                done.push(json!({ "from": m["from"], "to": m["to"], "label": m["label"], "status": "moved" }));
            }
            Err(e) => {
                skipped += 1;
                done.push(json!({ "from": m["from"], "to": m["to"], "label": m["label"], "status": "skipped", "reason": e.to_string() }));
            }
        }
    }
    let note = if skipped == 0 { format!("{ok}개를 옮겼습니다.") } else { format!("{ok}개를 옮기고 {skipped}개는 건너뛰었습니다.") };
    Ok(StepResult::new(json!({ "moves": done }), note))
}

fn organize_report(ctx: &Ctx) -> Value {
    let folder = ctx.out("scan")["folder"].as_str().unwrap_or("").to_string();
    let moves = ctx.out("apply")["moves"].as_array().cloned().unwrap_or_default();
    let mut md = format!("# 폴더 정리 보고\n\n- 폴더: `{folder}`\n");
    let moved = moves.iter().filter(|m| m["status"] == "moved").count();
    md.push_str(&format!("- 옮긴 파일: {moved}개\n- 건너뛴 파일: {}개\n\n", moves.len() - moved));
    md.push_str("## 폴더별\n\n| 폴더 | 파일 수 |\n|---|---|\n");
    for f in ctx.out("plan")["folders"].as_array().into_iter().flatten() {
        md.push_str(&format!("| {} | {} |\n", f[0].as_str().unwrap_or(""), f[1]));
    }
    let skipped: Vec<&Value> = moves.iter().filter(|m| m["status"] != "moved").collect();
    if !skipped.is_empty() {
        md.push_str("\n## 건너뛴 파일\n\n");
        for m in skipped {
            md.push_str(&format!("- {} — {}\n", m["label"].as_str().unwrap_or(""), m["reason"].as_str().unwrap_or("")));
        }
    }
    json!({
        "kind": "organize",
        "title": "폴더 정리 보고",
        "markdown": md,
        "folder": folder,
        "moves": moves,
        "undone": false,
    })
}

/// 완료된 정리 작업을 되돌린다. 옮긴 자리에 파일이 그대로 있고 원래 자리가 비어 있을 때만 되돌린다.
pub fn undo_organize(result: &mut Value) -> Result<(usize, usize)> {
    if result["undone"] == true {
        bail!("이미 되돌린 작업입니다.");
    }
    let (mut back, mut skipped) = (0, 0);
    let mut dirs: HashSet<PathBuf> = HashSet::new();
    for m in result["moves"].as_array().into_iter().flatten().rev() {
        if m["status"] != "moved" {
            continue;
        }
        let from = PathBuf::from(m["from"].as_str().unwrap_or(""));
        let to = PathBuf::from(m["to"].as_str().unwrap_or(""));
        if to.exists() && !from.exists() && std::fs::rename(&to, &from).is_ok() {
            back += 1;
            if let Some(p) = to.parent() {
                dirs.insert(p.to_path_buf());
                if let Some(pp) = p.parent() {
                    dirs.insert(pp.to_path_buf());
                }
            }
        } else {
            skipped += 1;
        }
    }
    // 정리하며 만든 빈 폴더만 지운다 (내용이 있으면 remove_dir이 실패하므로 안전).
    let root = PathBuf::from(result["folder"].as_str().unwrap_or(""));
    let mut dirs: Vec<PathBuf> = dirs.into_iter().filter(|d| d != &root && d.starts_with(&root)).collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for d in dirs {
        let _ = std::fs::remove_dir(&d);
    }
    result["undone"] = json!(true);
    Ok((back, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn korean_amount_formats_like_quotes() {
        assert_eq!(korean_amount(198_000), "일십구만팔천");
        assert_eq!(korean_amount(1_000_000), "일백만");
        assert_eq!(korean_amount(120_005_000), "일억이천만오천");
    }

    #[test]
    fn won_inserts_commas() {
        assert_eq!(won(1234567), "1,234,567");
        assert_eq!(won(999), "999");
    }
}
