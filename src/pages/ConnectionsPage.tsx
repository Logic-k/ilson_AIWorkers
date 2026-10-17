import { useEffect, useState } from "react";
import { api, engineHue, engineShort, errorText, type EngineId, type EngineStatus, type Settings } from "../api";
import { useStore } from "../store";
import { Avatar, Icon } from "../components/ui";

/** 엔진별 자동 실행 시 약관 안내 (공식 문서 기준 요약) */
const policy: Partial<Record<EngineId, string>> = {
  opencode: "OpenCode는 MIT 라이선스 오픈소스 CLI입니다. 기본 DeepSeek 무료 모델을 지원하며, 로그인 없이 즉시 사용할 수 있습니다. 선택적으로 콘솔 키를 등록해 유료 모델을 사용할 수도 있습니다.",
};

const planOptions: Partial<Record<EngineId, string[]>> = {
  opencode: ["Free (DeepSeek)", "OpenCode Zen", "Custom"],
};

function EngineCard({ e, s, onSave, onRecheck }: { e: EngineStatus; s: Settings; onSave: (s: Settings) => void; onRecheck: () => void }) {
  const { toast } = useStore();
  const [ack, setAck] = useState(false);
  const acked = !!s.autoAcks[e.id];
  const tone = !e.installed ? "neutral" : e.signedIn ? "positive" : "warning";
  const label = !e.installed ? "설치 안 됨" : e.signedIn ? "구독 로그인됨" : "로그인 필요";
  return (
    <article className="engine-card">
      <header>
        <span className="engine-badge" style={{ background: `hsl(${engineHue[e.id]} 55% 45%)` }}>
          {engineShort[e.id]}
        </span>
        <div>
          <strong>{e.label}</strong>
          <small>{e.plan}</small>
        </div>
        <span className={`badge badge--${tone}`}>
          <i />
          {label}
        </span>
      </header>
      <p className="engine-card__meta">
        {e.installed ? `v${e.version ?? "?"}${e.login ? ` · ${e.login}` : ""}` : "이 PC에서 찾지 못했습니다"}
      </p>
      {e.message && <p className="engine-card__msg">{e.message}</p>}
      <div className="engine-card__row">
        {e.installed && e.id !== "opencode" && (
          <button
            className="btn btn--ghost btn--sm"
            onClick={async () => {
              try {
                await api.openLoginTerminal(e.id);
                toast(`터미널에서 ${e.label}에 구독 계정으로 로그인한 뒤 ‘다시 확인’을 눌러 주세요.`);
              } catch (err) {
                toast(errorText(err), "error");
              }
            }}
          >
            <Icon name="key" size={14} /> 로그인 창 열기
          </button>
        )}
        {e.installed && e.id === "opencode" && (
          <button
            className="btn btn--ghost btn--sm"
            onClick={async () => {
              try {
                await api.openLoginTerminal(e.id);
                toast("터미널에서 opencode auth 또는 모델 키를 설정할 수 있습니다.");
              } catch (err) {
                toast(errorText(err), "error");
              }
            }}
          >
            <Icon name="key" size={14} /> 콘솔 키 등록 (선택)
          </button>
        )}
        <button className="btn btn--ghost btn--sm" onClick={onRecheck}>
          <Icon name="refresh" size={14} /> 다시 확인
        </button>
        <select
          className="engine-card__model"
          style={{ maxWidth: 120 }}
          title="구독 요금제 선택"
          value={s.plans?.[e.id] ?? e.plan}
          onChange={(ev) => onSave({ ...s, plans: { ...(s.plans ?? {}), [e.id]: ev.target.value } })}
        >
          {(planOptions[e.id] ?? [e.plan]).map((p) => (
            <option key={p} value={p}>{p}</option>
          ))}
        </select>
        {e.canAuto && e.models.length > 1 && (
          <select className="engine-card__model" value={s.models[e.id] ?? ""} onChange={(ev) => onSave({ ...s, models: { ...s.models, [e.id]: ev.target.value } })}>
            <option value="">기본 모델</option>
            {e.models.filter((m) => m.id).map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
        )}
      </div>
      {e.canAuto && (
        <div className="engine-card__auto">
          {acked ? (
            <p>
              <Icon name="check" size={14} /> 자동 실행 동의함{" "}
              <button
                className="link-btn link-btn--danger"
                onClick={() => {
                  const next = { ...s.autoAcks };
                  delete next[e.id];
                  onSave({ ...s, autoAcks: next });
                }}
              >
                철회
              </button>
            </p>
          ) : (
            <>
              <small>{policy[e.id]}</small>
              <label className="check">
                <input type="checkbox" checked={ack} onChange={(ev) => setAck(ev.target.checked)} />
                <span>위험을 이해했고, 제 계정 하나로 이 PC에서만 자동 실행을 허락합니다.</span>
              </label>
              <button className="btn btn--sm" disabled={!ack} onClick={() => onSave({ ...s, autoAcks: { ...s.autoAcks, [e.id]: new Date().toISOString() } })}>
                자동 실행 허락
              </button>
            </>
          )}
        </div>
      )}
    </article>
  );
}

const textFields: { key: keyof Settings; label: string; placeholder?: string; wide?: boolean }[] = [
  { key: "businessName", label: "상호", placeholder: "견적서 공급자 이름으로 쓰입니다" },
  { key: "ownerName", label: "대표자" },
  { key: "bizRegNo", label: "사업자등록번호", placeholder: "000-00-00000" },
  { key: "industry", label: "업종", placeholder: "예: 동네 카페, 필라테스 학원" },
  { key: "region", label: "지역", placeholder: "예: 서울 성동구" },
  { key: "phone", label: "전화" },
  { key: "email", label: "이메일" },
  { key: "address", label: "주소", wide: true },
];

export function ConnectionsPage() {
  const { boot, refresh, toast } = useStore();
  const [s, setS] = useState<Settings | null>(boot?.settings ?? null);
  const [dirty, setDirty] = useState(false);
  const [checking, setChecking] = useState(false);
  useEffect(() => {
    if (boot && !dirty) setS(boot.settings);
  }, [boot, dirty]);
  if (!boot || !s) return null;
  const set = <K extends keyof Settings>(k: K, v: Settings[K]) => {
    setS({ ...s, [k]: v });
    setDirty(true);
  };
  const saveAll = async (next = s) => {
    try {
      await api.saveSettings(next);
      setS(next);
      setDirty(false);
      await refresh();
      toast("설정을 저장했습니다.", "success");
    } catch (e) {
      toast(errorText(e), "error");
    }
  };
  const recheck = async () => {
    setChecking(true);
    try {
      await api.engineStatus();
      await refresh();
    } catch (e) {
      toast(errorText(e), "error");
    } finally {
      setChecking(false);
    }
  };
  const aiStaff = boot.employees.filter((e) => e.engine);
  const usable = boot.engines.filter((e) => e.installed);
  const auto = s.engineMode === "auto";

  return (
    <div className="page settings settings--wide">
      <section className="settings__intro">
        <span className="eyebrow">연결 센터</span>
        <h2>API 키 없이, 사장님 구독을 여러 개 연결해 한 팀처럼 씁니다.</h2>
      </section>

      <section className="card">
        <header className="card__head">
          <div>
            <span className="eyebrow">구독 엔진</span>
            <h3>연결된 AI 구독 {boot.engines.filter((e) => e.signedIn).length}개</h3>
          </div>
          <button className="btn btn--ghost btn--sm" disabled={checking} onClick={recheck}>
            <Icon name="refresh" size={14} /> {checking ? "확인 중…" : "모두 다시 확인"}
          </button>
        </header>
        <div className="engine-grid">
          {boot.engines.map((e) => (
            <EngineCard key={e.id} e={e} s={s} onSave={saveAll} onRecheck={recheck} />
          ))}
        </div>
        <small className="conn-card__foot">
          상태 확인은 사용량을 쓰지 않습니다. 앱은 로그인 정보·토큰을 읽지 않고, 실행할 때 API 키 관련 환경 변수(GEMINI_API_KEY, ANTHROPIC_API_KEY, OPENAI_API_KEY 등)를 지워 구독 로그인으로만 동작하게 합니다. API 키로 로그인된 CLI는 ‘로그인 필요’로 표시하고 쓰지 않습니다.
        </small>
      </section>

      <section className="card">
        <header className="card__head">
          <div>
            <span className="eyebrow">실행 방식</span>
            <h3>직원들은 어떻게 일할까요?</h3>
          </div>
        </header>
        <div className="model-list" role="radiogroup">
          <label className={`model ${!auto ? "is-on" : ""}`}>
            <input type="radio" name="mode" checked={!auto} onChange={() => saveAll({ ...s, engineMode: "safe" })} />
            <span>
              <strong>안전 모드 (권장)</strong>
              <small>앱이 지시서와 에이전트 정의를 준비하고, 사장님이 OpenCode CLI에서 직접 실행합니다. 앱은 진행을 지켜보며 사무실 연출·다음 단계·병합을 맡습니다.</small>
            </span>
          </label>
          <label className={`model ${auto ? "is-on" : ""}`}>
            <input type="radio" name="mode" checked={auto} onChange={() => saveAll({ ...s, engineMode: "auto" })} />
            <span>
              <strong>자동 모드</strong>
              <small>
                사장님이 ‘자동 실행 허락’한 엔진은 앱이 직접 실행해, 직원들이 한 작업 공간에서 단계마다 일을 주고받으며 끝까지 진행합니다. 허락하지 않은 엔진을 쓰는 직원 차례에서는 안전 모드처럼 사장님께 실행을 부탁합니다.
              </small>
            </span>
          </label>
        </div>
        <label className={`yolo ${s.yolo ? "is-on" : ""}`}>
          <input type="checkbox" checked={s.yolo} onChange={(e) => saveAll({ ...s, yolo: e.target.checked })} />
          <span>
            <strong>YOLO 모드</strong>
            <small>자동 모드에서 파일을 고치는 단계는 모든 확인을 생략합니다(각 CLI의 dangerously-skip / bypass 옵션). 별도 작업 공간에서 일하고 병합은 여전히 사장님 확인을 받지만, 명령이 샌드박스 없이 실행될 수 있습니다.</small>
          </span>
        </label>
        <label className={`yolo ${s.autoOpenFolder !== false ? "is-on" : ""}`} style={{ marginTop: 8 }}>
          <input
            type="checkbox"
            checked={s.autoOpenFolder !== false}
            onChange={(e) => saveAll({ ...s, autoOpenFolder: e.target.checked })}
          />
          <span>
            <strong>작업 완료 후 산출물 폴더 자동으로 띄우기</strong>
            <small>원클릭 자동업무나 개발·문서 작업이 완료되면, 결과물이 저장된 폴더를 윈도우 탐색기로 즉시 열어 직접 폴더를 찾는 번거로움을 없앱니다.</small>
          </span>
        </label>
      </section>

      <section className="card">
        <header className="card__head">
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", width: "100%", flexWrap: "wrap", gap: 10 }}>
            <div>
              <span className="eyebrow">팀 구성</span>
              <h3>직원별 담당 구독</h3>
            </div>
            <button
              className="btn btn--ghost btn--sm"
              onClick={() => {
                const next = { ...s.assignments };
                for (const emp of aiStaff) {
                  next[emp.id] = "opencode";
                }
                saveAll({ ...s, assignments: next });
                toast("모든 AI 직원을 OpenCode(DeepSeek 무료)로 배정했습니다.", "success");
              }}
            >
              <Icon name="spark" size={14} /> 모든 AI 직원을 OpenCode(DeepSeek 무료)로 배정
            </button>
          </div>
        </header>
        <p className="field__hint">모든 AI 직원은 OpenCode(기본 DeepSeek Flash 무료 모델) 엔진으로 동작합니다.</p>
        <div className="assign-grid">
          {aiStaff.map((e) => (
            <label key={e.id} className="assign">
              <Avatar employee={e} size={30} />
              <span>
                <strong>{e.name}</strong>
                <small>{e.role}</small>
              </span>
              <select
                value={s.assignments[e.id] ?? "opencode"}
                onChange={(ev) => saveAll({ ...s, assignments: { ...s.assignments, [e.id]: ev.target.value as EngineId } })}
              >
                {boot.engines.map((en) => (
                  <option key={en.id} value={en.id} disabled={!en.installed}>
                    {en.label}
                    {!en.installed ? " (설치 안 됨)" : !en.signedIn ? " (로그인 필요)" : !en.canAuto ? " (안전 모드만)" : ""}
                  </option>
                ))}
              </select>
            </label>
          ))}
        </div>
        {usable.length === 0 && <p className="callout callout--warning">이 PC에서 OpenCode CLI를 찾지 못했습니다. `npm i -g opencode`로 설치해 주세요.</p>}
      </section>

      <section className="card">
        <header className="card__head">
          <div>
            <span className="eyebrow">YOLO 키워드 & 안전 원칙</span>
            <h3>권한 단계별로 실제 쓰는 CLI 옵션</h3>
            <p style={{ color: "var(--brand)", fontWeight: 600, fontSize: 13, marginTop: 4 }}>
              🛡️ 돈 쓰거나 파일을 지우기 전에는 반드시 사장님께 먼저 물어봅니다.
            </p>
          </div>
        </header>
        <div className="levels">
          <table>
            <thead>
              <tr>
                <th>엔진</th>
                <th>읽기 (설계·리뷰)</th>
                <th>편집</th>
                <th>샌드박스 실행</th>
                <th>YOLO</th>
              </tr>
            </thead>
            <tbody>
              {boot.levels.map((l) => (
                <tr key={l.engine}>
                  <td>{boot.engines.find((e) => e.id === l.engine)?.label}</td>
                  <td><code>{l.readOnly || "(기본: 헤드리스에서 자동 거부)"}</code></td>
                  <td><code>{l.edit}</code></td>
                  <td><code>{l.sandbox}</code></td>
                  <td><code>{l.yolo}</code></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <small className="field__hint">설계·리뷰는 항상 ‘읽기’, 개발·테스트는 업무의 ‘명령 실행 허용’에 따라 ‘편집’ 또는 ‘샌드박스 실행’, YOLO 모드를 켜면 ‘YOLO’로 실행합니다.</small>
      </section>

      <section className="card">
        <header className="card__head">
          <div>
            <span className="eyebrow">사업장 정보</span>
            <h3>견적서와 글에 들어갈 우리 회사</h3>
          </div>
        </header>
        <div className="form-grid">
          {textFields.map((f) => (
            <div key={f.key} className={`field ${f.wide ? "field--wide" : ""}`}>
              <label className="field__label">{f.label}</label>
              <input value={String(s[f.key] ?? "")} placeholder={f.placeholder} onChange={(e) => set(f.key, e.target.value as never)} />
            </div>
          ))}
        </div>
      </section>

      <section className="card">
        <header className="card__head">
          <div>
            <span className="eyebrow">브랜드 문체</span>
            <h3>우리 회사 말투</h3>
          </div>
        </header>
        <div className="form-grid">
          <div className="field">
            <label className="field__label">말투</label>
            <select value={s.brandTone} onChange={(e) => set("brandTone", e.target.value)}>
              {["친근한", "전문적인", "담백한", "유쾌한", "정중한"].map((t) => (
                <option key={t}>{t}</option>
              ))}
            </select>
          </div>
          <div className="field">
            <label className="field__label">브랜드 키워드</label>
            <input value={s.brandKeywords} placeholder="예: 수제, 매일 아침, 동네 단골" onChange={(e) => set("brandKeywords", e.target.value)} />
          </div>
          <div className="field field--wide">
            <label className="field__label">쓰지 말 표현</label>
            <input value={s.bannedWords} placeholder="쉼표로 구분 · 예: 최저가, 무조건, 100%" onChange={(e) => set("bannedWords", e.target.value)} />
          </div>
          <div className="field field--wide">
            <label className="field__label">평소 글 예시</label>
            <textarea rows={4} value={s.sampleText} placeholder="예전에 직접 쓴 글을 붙여 넣으면 말투를 따라 씁니다." onChange={(e) => set("sampleText", e.target.value)} />
          </div>
        </div>
      </section>

      <section className="card card--quiet">
        <h3>데이터</h3>
        <p>
          업무 기록과 작업 공간(git worktree)은 이 PC에만 저장됩니다: <code>{boot.dataDir}</code>
        </p>
      </section>

      <div className={`save-bar ${dirty ? "is-on" : ""}`}>
        <span>저장하지 않은 변경이 있습니다.</span>
        <button className="btn btn--ghost btn--sm" onClick={() => setDirty(false)}>
          되돌리기
        </button>
        <button className="btn btn--primary btn--sm" onClick={() => saveAll()}>
          저장
        </button>
      </div>
    </div>
  );
}
