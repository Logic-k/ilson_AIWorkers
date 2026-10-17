use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

const RAW_PRICES: &str = include_str!("../resources/prices.json");



#[derive(Debug, Clone, Deserialize)]
struct PriceFile {
    generated_at: String,
    source: String,
    models: HashMap<String, RawModelPrice>,
    #[serde(default)]
    #[allow(dead_code)]
    overrides: HashMap<String, OverrideNote>,
}

#[derive(Debug, Clone, Deserialize)]
struct OverrideNote {
    #[allow(dead_code)]
    note: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawModelPrice {
    input: f64,
    output: f64,
    #[serde(default)]
    cache_read: f64,
    #[serde(default)]
    cache_write: f64,
    #[serde(default)]
    context: u64,
    #[serde(default)]
    output_limit: u64,
    #[serde(default)]
    free: bool,
    #[serde(default)]
    sibling: Option<String>,
}

/// 정규화된 모델 단가 (1M 토큰당 USD).
/// 무료(`-free`) 모델의 경우 `list_cost_zero = true`이고,
/// `input/output/cache_read/cache_write`에는 유료 형제 모델의 단가가 들어간다(환산 비용 계산용).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    pub model_key: String,
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub context: u64,
    pub output_limit: u64,
    pub list_cost_zero: bool,
}

fn table() -> &'static PriceFile {
    static TABLE: OnceLock<PriceFile> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(RAW_PRICES).expect("resources/prices.json 파싱 실패")
    })
}

pub fn generated_at() -> &'static str {
    &table().generated_at
}

pub fn source() -> &'static str {
    &table().source
}

pub fn default_model_for(_engine: &str) -> &'static str {
    crate::engine::OPENCODE_DEFAULT_MODEL
}

fn strip_effort_suffix(s: &str) -> &str {
    for suffix in ["-high", "-medium", "-low", "-minimal", "-max"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            return stripped;
        }
    }
    s
}

/// 엔진과 모델 ID를 받아 환산 비용 계산용 `Price`를 돌려준다.
pub fn resolve(engine: &str, model_id: Option<&str>) -> Option<Price> {
    let t = table();
    let raw = model_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default_model_for(engine));
    let low = raw.to_ascii_lowercase();

    // 1. 정확히 일치하는 키가 있는지 먼저 확인
    if let Some(found) = lookup_key(t, &low) {
        return Some(found);
    }

    // 2. OpenCode 모델 정규화
    match engine {
        "opencode" => {
            let is_free = low.ends_with("-free");
            let without_prov = low
                .strip_prefix("opencode-go/")
                .or_else(|| low.strip_prefix("opencode/"))
                .unwrap_or(&low);
            let base = without_prov.strip_suffix("-free").unwrap_or(without_prov);
            let clean = strip_effort_suffix(base);

            if low.starts_with("opencode-go/") {
                if let Some(p) = lookup_key(t, &format!("opencode-go/{clean}")) {
                    return Some(p);
                }
            }
            if is_free {
                if let Some(mut p) = lookup_key(t, &format!("opencode/{clean}"))
                    .or_else(|| lookup_key(t, &format!("deepseek/{clean}")))
                    .or_else(|| lookup_key(t, "opencode/deepseek-v4.1-flash"))
                {
                    p.model_key = if low.contains('/') {
                        low.clone()
                    } else {
                        format!("opencode/{low}")
                    };
                    p.list_cost_zero = true;
                    return Some(p);
                }
            }
            lookup_key(t, &format!("opencode/{clean}"))
                .or_else(|| lookup_key(t, &format!("deepseek/{clean}")))
                .or_else(|| lookup_key(t, "opencode/deepseek-v4.1-flash-free"))
        }
        _ => lookup_key(t, "opencode/deepseek-v4.1-flash-free"),
    }
}

fn lookup_key(t: &PriceFile, key: &str) -> Option<Price> {
    let raw = t.models.get(key)?;
    if raw.free {
        let sib_key = raw
            .sibling
            .as_deref()
            .unwrap_or("opencode/deepseek-v4.1-flash");
        let sib = t.models.get(sib_key).unwrap_or(raw);
        Some(Price {
            model_key: key.to_string(),
            input: sib.input,
            output: sib.output,
            cache_read: sib.cache_read,
            cache_write: sib.cache_write,
            context: raw.context,
            output_limit: raw.output_limit,
            list_cost_zero: true,
        })
    } else {
        Some(Price {
            model_key: key.to_string(),
            input: raw.input,
            output: raw.output,
            cache_read: raw.cache_read,
            cache_write: raw.cache_write,
            context: raw.context,
            output_limit: raw.output_limit,
            list_cost_zero: false,
        })
    }
}

/// 토큰 수와 단가로 환산 비용(USD)을 계산한다.
pub fn cost(
    price: &Price,
    input_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    output_tokens: u64,
) -> f64 {
    (input_tokens as f64 * price.input
        + cache_read_tokens as f64 * price.cache_read
        + cache_write_tokens as f64 * price.cache_write
        + output_tokens as f64 * price.output)
        / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricing_cost_matches_hand_calculation_within_one_cent() {
        // OpenCode DeepSeek V4.1 Flash (free uses sibling $0.30/M in, $0.006/M cache_read, $1.20/M out)
        // 200,000 input ($0.06) + 500,000 cache_read ($0.003) + 0 + 25,000 output ($0.03) = $0.093
        let ds = resolve("opencode", Some("opencode/deepseek-v4.1-flash-free")).expect("ds price");
        assert!(ds.list_cost_zero);
        let c3 = cost(&ds, 200_000, 500_000, 0, 25_000);
        assert!((c3 - 0.093).abs() < 0.01, "expected ~0.093, got {c3}");
    }

    #[test]
    fn pricing_resolve_normalizes_aliases_suffixes_and_free() {
        let free_model = resolve("opencode", Some("opencode/deepseek-v4.1-flash-free")).unwrap();
        assert!(free_model.list_cost_zero);
        assert!((free_model.input - 0.30).abs() < 1e-6);
        assert!((free_model.output - 1.20).abs() < 1e-6);

        let unknown_free = resolve("opencode", Some("opencode/custom-new-free")).unwrap();
        assert!(unknown_free.list_cost_zero);
        assert!(unknown_free.input > 0.0);
    }
}
