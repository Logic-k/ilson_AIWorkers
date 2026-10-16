use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::engine::Kind;

/// 업무 상태. 모든 직원이 같은 상태 흐름(업무 계약)을 공유한다.
pub mod status {
    pub const QUEUED: &str = "queued";
    pub const AWAITING_APPROVAL: &str = "awaiting_approval";
    pub const RUNNING: &str = "running";
    pub const PAUSED: &str = "paused";
    pub const COMPLETED: &str = "completed";
    pub const FAILED: &str = "failed";
    pub const CANCELLED: &str = "cancelled";
}

/// 실행 전에 사장님 확인이 필요한 지점(파일 이동, 코드 병합 등).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate {
    pub kind: String,
    /// 승인 기록에 남는 키 (예: "merge:merge", "handoff:team")
    #[serde(default)]
    pub key: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub details: Vec<String>,
    pub estimate_krw: Option<i64>,
    pub max_krw: Option<i64>,
    /// 안전 모드 지시서 정보: 작업 폴더, 에이전트 이름, 터미널 명령, 에이전트에게 할 말
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    /// 이 단계를 실행할 엔진 (agy, claude, codex, kiro)
    #[serde(default)]
    pub engine: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepRow {
    pub idx: i64,
    pub key: String,
    pub label: String,
    pub status: String,
    pub note: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRow {
    pub at: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub employee_id: String,
    pub title: String,
    pub status: String,
    pub input: Value,
    pub progress: i64,
    pub current_step: Option<String>,
    pub current_actor: Option<String>,
    pub pending_gate: Option<Gate>,
    pub approved_gates: Vec<String>,
    pub cost_usd: f64,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub seen_at: Option<String>,
    #[serde(default)]
    pub steps: Vec<StepRow>,
    #[serde(default)]
    pub events: Vec<EventRow>,
}

/// 일일 토큰/원화 예산 상한 (전체 또는 엔진별).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BudgetCap {
    pub daily_tokens: Option<u64>,
    pub daily_krw: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Budgets {
    pub daily_tokens: Option<u64>,
    pub daily_krw: Option<i64>,
    pub per_engine: HashMap<String, BudgetCap>,
}

/// 앱 설정. 사업장 정보와 문체는 블로그·견적·숏폼 직원이 함께 쓴다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Antigravity 모델 ID. 비우면 agy 기본값 (자동 모드에서만 쓰임).
    pub model: String,
    /// "safe"(기본: 사장님이 Antigravity에서 실행) 또는 "auto"(앱이 agy를 직접 실행)
    pub engine_mode: String,
    /// (이전 버전) Antigravity 자동 모드 동의 시각
    pub auto_ack_at: Option<String>,
    /// 엔진별 자동 실행 위험 안내 동의 시각 (agy, claude, codex, opencode)
    pub auto_acks: HashMap<String, String>,
    /// 직원별 담당 구독 엔진 (예: taeo → claude)
    pub assignments: HashMap<String, String>,
    /// 엔진별 모델 (비우면 각 CLI 기본값)
    pub models: HashMap<String, String>,
    /// 자동 모드에서 파일을 고치는 단계를 모두 YOLO(확인 생략)로 실행
    pub yolo: bool,
    /// 환산 비용 계산용 USD/KRW 환율 (기본 1343.0)
    pub usd_krw: f64,
    /// 마지막 환율 갱신 시각
    pub fx_updated_at: Option<String>,
    /// 엔진별 구독 요금제 (agy: pro|ultra5|ultra20, claude: pro|max5|max20, codex: plus|pro, opencode: free|go|go_plus)
    pub plans: HashMap<String, String>,
    /// 일일 토큰·원화 예산 상한
    pub budgets: Budgets,
    pub business_name: String,
    pub owner_name: String,
    pub biz_reg_no: String,
    pub industry: String,
    pub region: String,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub brand_tone: String,
    pub brand_keywords: String,
    pub banned_words: String,
    pub sample_text: String,
}

impl Settings {
    /// 직원이 쓰는 구독 엔진. 정하지 않았으면 Antigravity.
    pub fn engine_for(&self, employee: &str) -> Kind {
        Kind::from_id(self.assignments.get(employee).map(String::as_str).unwrap_or("agy"))
    }

    /// 이 엔진을 앱이 직접 실행해도 되는지: 자동 모드 + 엔진별 위험 안내 동의.
    pub fn auto_ok(&self, kind: Kind) -> bool {
        self.engine_mode == "auto"
            && kind.can_auto()
            && (self.auto_acks.contains_key(kind.id()) || (kind == Kind::Agy && self.auto_ack_at.is_some()))
    }

    /// 업무 단위 자동 모드: 동의한 엔진이 하나라도 있으면 단계별로 나눠 진행한다.
    pub fn auto_mode(&self) -> bool {
        crate::engine::ALL.iter().any(|k| self.auto_ok(*k))
    }

    pub fn model_for(&self, kind: Kind) -> Option<String> {
        let m = self.models.get(kind.id()).cloned().unwrap_or_else(|| if kind == Kind::Agy { self.model.clone() } else { String::new() });
        let trimmed = m.trim();
        if !trimmed.is_empty() {
            Some(trimmed.to_string())
        } else if kind == Kind::Opencode {
            Some(crate::engine::OPENCODE_DEFAULT_MODEL.to_string())
        } else {
            None
        }
    }

    pub fn effective_usd_krw(&self) -> f64 {
        if self.usd_krw > 100.0 {
            self.usd_krw
        } else {
            1343.0
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: String::new(),
            engine_mode: "safe".into(),
            auto_ack_at: None,
            auto_acks: HashMap::new(),
            assignments: HashMap::new(),
            models: HashMap::new(),
            yolo: false,
            usd_krw: 1343.0,
            fx_updated_at: None,
            plans: HashMap::new(),
            budgets: Budgets::default(),
            business_name: String::new(),
            owner_name: String::new(),
            biz_reg_no: String::new(),
            industry: String::new(),
            region: String::new(),
            phone: String::new(),
            email: String::new(),
            address: String::new(),
            brand_tone: "친근한".into(),
            brand_keywords: String::new(),
            banned_words: String::new(),
            sample_text: String::new(),
        }
    }
}
