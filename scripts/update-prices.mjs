#!/usr/bin/env node
/**
 * models.dev/api.json(MIT)에서 anthropic, openai, google, deepseek, opencode, opencode-go
 * 공급자의 모델 가격을 가져와 src-tauri/resources/prices.json을 갱신한다.
 * 네트워크 실패 시 기존 파일을 보존한다.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT_PATH = path.resolve(__dirname, "../src-tauri/resources/prices.json");
const PROVIDERS = ["anthropic", "openai", "google", "deepseek", "opencode", "opencode-go"];

const OVERRIDES = {
  "deepseek/deepseek-v4.1-flash": {
    note: "DeepSeek V4.1 Flash 피크 정가 기준 ($0.30/$1.20, 캐시 $0.006). 오프피크 시간대는 절반($0.15/$0.60)."
  },
  "google/gemini-3.8-flash": {
    note: "2027-01-01부터 공식 API 단가 인상 예정."
  },
  "google/gemini-4-argon": {
    note: "2026-12-01부터 공식 API 단가 인상 예정."
  }
};

async function main() {
  const existing = fs.existsSync(OUT_PATH)
    ? JSON.parse(fs.readFileSync(OUT_PATH, "utf-8"))
    : { models: {} };

  let data;
  try {
    const res = await fetch("https://models.dev/api.json", {
      signal: AbortSignal.timeout(15000)
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    data = await res.json();
  } catch (err) {
    console.warn(`[prices] models.dev 조회 실패 (${err.message}) — 기존 가격표를 유지합니다.`);
    return;
  }

  const models = { ...(existing.models || {}) };
  for (const prov of PROVIDERS) {
    const provObj = data?.[prov];
    if (!provObj || typeof provObj.models !== "object") continue;
    for (const [id, m] of Object.entries(provObj.models)) {
      if (m.deprecated) continue;
      const cost = m.cost || {};
      const limit = m.limit || {};
      const key = `${prov}/${id}`;
      const isFree = id.endsWith("-free") || (cost.input === 0 && cost.output === 0);
      const siblingKey = id.endsWith("-free")
        ? `${prov}/${id.replace(/-free$/, "")}`
        : undefined;
      models[key] = {
        input: Number(cost.input ?? 0),
        output: Number(cost.output ?? 0),
        cache_read: Number(cost.cache_read ?? 0),
        cache_write: Number(cost.cache_write ?? cost.input ?? 0),
        context: Number(limit.context ?? 128000),
        output_limit: Number(limit.output ?? 16000),
        ...(isFree ? { free: true } : {}),
        ...(siblingKey ? { sibling: siblingKey } : {})
      };
    }
  }

  const today = new Date().toISOString().slice(0, 10);
  const out = {
    generated_at: today,
    source: "models.dev api.json (MIT) + LiteLLM model_prices_and_context_window.json (MIT)",
    unit: "USD per 1M tokens",
    models,
    overrides: OVERRIDES
  };

  fs.mkdirSync(path.dirname(OUT_PATH), { recursive: true });
  fs.writeFileSync(OUT_PATH, JSON.stringify(out, null, 2) + "\n", "utf-8");
  console.log(`[prices] ${Object.keys(models).length}개 모델 저장 완료 (${OUT_PATH})`);
}

main();
