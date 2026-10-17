import { useEffect, useState } from "react";
import {
  api,
  errorText,
  fmtTokens,
  fmtUsd,
  shortModelLabel,
  won,
  type EngineQuota,
  type UsageSummary,
} from "../api";
import { useStore } from "../store";
import { Icon } from "../components/ui";
import { Meter } from "../components/Meter";

export function UsagePage() {
  const { boot, refresh, toast } = useStore();
  const settings = boot?.settings;
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [quotas, setQuotas] = useState<EngineQuota[]>([]);
  const [loading, setLoading] = useState(true);
  const [checkingQuota, setCheckingQuota] = useState(false);
  const [refreshingFx, setRefreshingFx] = useState(false);
  const [range, setRange] = useState<"7d" | "30d" | "all">("30d");
  const [activeTab, setActiveTab] = useState<"engine" | "model" | "actor" | "day">("engine");

  const loadData = async (refreshQuota = false) => {
    try {
      const [s, q] = await Promise.all([
        api.usageSummary(range),
        api.quotaStatus(refreshQuota),
      ]);
      setSummary(s);
      setQuotas(q);
    } catch (err) {
      toast(errorText(err), "error");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, [range]);

  const handleRefreshQuota = async () => {
    setCheckingQuota(true);
    try {
      const q = await api.quotaStatus(true);
      setQuotas(q);
      toast("구독 쿼터 상태를 새로 고쳤습니다.");
    } catch (err) {
      toast(errorText(err), "error");
    } finally {
      setCheckingQuota(false);
    }
  };

  const handleRefreshFx = async () => {
    setRefreshingFx(true);
    try {
      const newSettings = await api.refreshFx();
      await api.saveSettings(newSettings);
      await refresh();
      const s = await api.usageSummary(range);
      setSummary(s);
      toast("환율을 새로 고쳤습니다.");
    } catch (err) {
      toast(errorText(err), "error");
    } finally {
      setRefreshingFx(false);
    }
  };

  if (loading && !summary) {
    return <div className="loading">사용량 및 예산 정보를 불러오는 중...</div>;
  }

  const s = summary;
  const dailyTokenCap = settings?.budgets?.dailyTokens ?? null;
  const dailyKrwCap = settings?.budgets?.dailyKrw ?? null;

  return (
    <div className="page">
      {/* 상단 타이틀 및 환율 */}
      <section className="card">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: 12 }}>
          <div>
            <div className="eyebrow">USAGE & BUDGET</div>
            <h2>사용량 및 예산 모니터링</h2>
            <p style={{ color: "var(--muted)", fontSize: 13, marginTop: 4 }}>
              실제 결제 0원의 로컬 구독 CLI 사용량, 표준 환산 비용, 일일 예산 상한을 확인합니다.
            </p>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <span style={{ fontSize: 13, color: "var(--muted)" }}>
              기준 환율: <strong style={{ fontFamily: "var(--font-mono)" }}>1 USD = ₩{s?.usdKrw?.toLocaleString() ?? 1450}</strong>
              {s?.fxUpdatedAt && ` (${new Date(s.fxUpdatedAt).toLocaleDateString()})`}
            </span>
            <button
              className="btn btn--ghost btn--sm"
              onClick={handleRefreshFx}
              disabled={refreshingFx}
              title="한국수출입은행 환율 새로 고침"
            >
              <Icon name="refresh" size={14} />
              {refreshingFx ? "갱신 중..." : "환율 새로 고침"}
            </button>
          </div>
        </div>
      </section>

      {/* KPI 카드 4종 */}
      <div className="usage-grid">
        <article className="usage-card">
          <small style={{ color: "var(--muted)", fontWeight: 600 }}>오늘 사용량</small>
          <div className="usage-card__val">{fmtTokens(s?.todayTokens ?? 0)} 토큰</div>
          <div className="usage-card__sub">
            환산: <strong style={{ fontFamily: "var(--font-mono)" }}>{won(s?.todayKrw ?? 0)}</strong> ({fmtUsd(s?.todayUsd ?? 0)})
          </div>
        </article>

        <article className="usage-card">
          <small style={{ color: "var(--muted)", fontWeight: 600 }}>이번 주 누적</small>
          <div className="usage-card__val">{fmtTokens(s?.weekTokens ?? 0)} 토큰</div>
          <div className="usage-card__sub">
            환산: <strong style={{ fontFamily: "var(--font-mono)" }}>{won(s?.weekKrw ?? 0)}</strong> ({fmtUsd(s?.weekUsd ?? 0)})
          </div>
        </article>

        <article className="usage-card">
          <small style={{ color: "var(--muted)", fontWeight: 600 }}>이번 달 누적</small>
          <div className="usage-card__val">{fmtTokens(s?.monthTokens ?? 0)} 토큰</div>
          <div className="usage-card__sub">
            환산: <strong style={{ fontFamily: "var(--font-mono)" }}>{won(s?.monthKrw ?? 0)}</strong> ({fmtUsd(s?.monthUsd ?? 0)})
          </div>
        </article>

        <article className="usage-card" style={{ background: "var(--ok-soft)", borderColor: "#b8e2cb" }}>
          <small style={{ color: "var(--ok)", fontWeight: 700 }}>추가 API 청구액</small>
          <div className="usage-card__val" style={{ color: "var(--ok)" }}>₩0</div>
          <div className="usage-card__sub" style={{ color: "var(--ok)" }}>
            구독 계정 100% 활용 · 종량제 결제 없음
          </div>
        </article>
      </div>

      {/* 일일 예산 상한 (Budget Caps) */}
      <section className="card">
        <header className="card__head">
          <div>
            <h3>일일 예산 상한 (안전 가드)</h3>
            <p>설정한 상한의 80%에 도달하면 경고하고, 100% 도달 시 자동 실행이 일시 중지됩니다.</p>
          </div>
        </header>

        <div style={{ display: "grid", gap: 16, marginTop: 12 }}>
          {dailyTokenCap && dailyTokenCap > 0 ? (
            <Meter
              label="오늘 토큰 예산"
              value={s?.todayTokens ?? 0}
              max={dailyTokenCap}
              formatValue={fmtTokens}
              formatMax={fmtTokens}
              warnAt={0.8}
              badAt={1.0}
            />
          ) : (
            <div style={{ fontSize: 13, color: "var(--muted)" }}>
              일일 토큰 상한이 설정되지 않았습니다 (무제한).
            </div>
          )}

          {dailyKrwCap && dailyKrwCap > 0 ? (
            <Meter
              label="오늘 환산 비용 예산"
              value={s?.todayKrw ?? 0}
              max={dailyKrwCap}
              formatValue={(v) => won(v)}
              formatMax={(m) => won(m)}
              warnAt={0.8}
              badAt={1.0}
            />
          ) : (
            <div style={{ fontSize: 13, color: "var(--muted)" }}>
              일일 환산 비용 상한이 설정되지 않았습니다 (무제한).
            </div>
          )}
        </div>
      </section>

      {/* 구독 엔진별 쿼터 현황 */}
      <section className="card">
        <header className="card__head" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <h3>구독 엔진별 사용량 한도 (쿼터)</h3>
            <p>Claude Code, Codex, OpenCode, Agy의 공식 한도 및 추정치입니다.</p>
          </div>
          <button
            className="btn btn--ghost btn--sm"
            onClick={handleRefreshQuota}
            disabled={checkingQuota}
          >
            <Icon name="refresh" size={14} />
            {checkingQuota ? "확인 중..." : "지금 확인"}
          </button>
        </header>

        <div className="usage-grid" style={{ marginTop: 14 }}>
          {quotas.map((q) => (
            <article key={q.engine} className="quota-card">
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                <strong>{q.label}</strong>
                <span className="badge badge--neutral" style={{ fontSize: 11 }}>
                  {q.plan} · {q.source === "cli" ? "CLI 조회" : "앱 추정치"}
                </span>
              </div>
              {q.windows.length === 0 ? (
                <div style={{ color: "var(--muted)", fontSize: 12 }}>표시할 한도 정보가 없습니다.</div>
              ) : (
                q.windows.map((w, idx) => (
                  <div key={idx} style={{ marginTop: 6 }}>
                    <Meter
                      label={w.label}
                      value={w.percent ?? 0}
                      max={100}
                      formatValue={(v) => `${v}%`}
                      formatMax={() => "100%"}
                      subLabel={w.resetsAt ? `초기화: ${new Date(w.resetsAt).toLocaleTimeString()}` : w.usedLabel}
                      warnAt={0.8}
                      badAt={1.0}
                    />
                  </div>
                ))
              )}
              {q.message && <div style={{ fontSize: 11.5, color: "var(--muted)" }}>{q.message}</div>}
            </article>
          ))}
        </div>
      </section>

      {/* 세부 분석 탭 테이블 */}
      <section className="card">
        <header className="card__head" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <h3>세부 사용량 분석</h3>
            <p>models.dev 표준 단가표({s?.priceVersion || "embedded"})를 기준으로 산출되었습니다.</p>
          </div>
          <div style={{ display: "flex", gap: 6 }}>
            <button
              className={`btn btn--sm ${range === "7d" ? "btn--primary" : "btn--ghost"}`}
              onClick={() => setRange("7d")}
            >
              7일
            </button>
            <button
              className={`btn btn--sm ${range === "30d" ? "btn--primary" : "btn--ghost"}`}
              onClick={() => setRange("30d")}
            >
              30일
            </button>
            <button
              className={`btn btn--sm ${range === "all" ? "btn--primary" : "btn--ghost"}`}
              onClick={() => setRange("all")}
            >
              전체
            </button>
          </div>
        </header>

        <nav className="tabs" style={{ marginTop: 14 }}>
          <button
            className={`tab ${activeTab === "engine" ? "is-active" : ""}`}
            onClick={() => setActiveTab("engine")}
          >
            엔진별 ({s?.byEngine.length ?? 0})
          </button>
          <button
            className={`tab ${activeTab === "model" ? "is-active" : ""}`}
            onClick={() => setActiveTab("model")}
          >
            모델별 ({s?.byModel.length ?? 0})
          </button>
          <button
            className={`tab ${activeTab === "actor" ? "is-active" : ""}`}
            onClick={() => setActiveTab("actor")}
          >
            직원별 ({s?.byActor.length ?? 0})
          </button>
          <button
            className={`tab ${activeTab === "day" ? "is-active" : ""}`}
            onClick={() => setActiveTab("day")}
          >
            일별 추이 ({s?.byDay.length ?? 0})
          </button>
        </nav>

        {activeTab === "engine" && (
          <div style={{ overflowX: "auto" }}>
            <table className="usage-table">
              <thead>
                <tr>
                  <th>구독 엔진</th>
                  <th className="num">실행 횟수</th>
                  <th className="num">입력 토큰</th>
                  <th className="num">출력 토큰</th>
                  <th className="num">총 토큰</th>
                  <th className="num">환산 비용 (원화)</th>
                  <th className="num">환산 비용 (USD)</th>
                </tr>
              </thead>
              <tbody>
                {(!s?.byEngine || s.byEngine.length === 0) ? (
                  <tr><td colSpan={7} style={{ textAlign: "center", color: "var(--muted)", padding: 24 }}>기록된 엔진 사용량이 없습니다.</td></tr>
                ) : (
                  s.byEngine.map((e) => (
                    <tr key={e.engine}>
                      <td><strong>{e.engine}</strong></td>
                      <td className="num">{e.runs.toLocaleString()}회</td>
                      <td className="num">{fmtTokens(e.inputTokens)}</td>
                      <td className="num">{fmtTokens(e.outputTokens)}</td>
                      <td className="num"><strong>{fmtTokens(e.totalTokens)}</strong></td>
                      <td className="num">{won(e.estKrw)}</td>
                      <td className="num">{fmtUsd(e.estUsd)}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        )}

        {activeTab === "model" && (
          <div style={{ overflowX: "auto" }}>
            <table className="usage-table">
              <thead>
                <tr>
                  <th>엔진</th>
                  <th>모델</th>
                  <th className="num">실행 횟수</th>
                  <th className="num">총 토큰</th>
                  <th className="num">환산 비용 (원화)</th>
                  <th className="num">환산 비용 (USD)</th>
                </tr>
              </thead>
              <tbody>
                {(!s?.byModel || s.byModel.length === 0) ? (
                  <tr><td colSpan={6} style={{ textAlign: "center", color: "var(--muted)", padding: 24 }}>기록된 모델 사용량이 없습니다.</td></tr>
                ) : (
                  s.byModel.map((m, idx) => (
                    <tr key={idx}>
                      <td>{m.engine}</td>
                      <td>
                        <strong>{shortModelLabel(m.model)}</strong>
                        {m.free && <span className="model-chip model-chip--free" style={{ marginLeft: 6 }}>무료</span>}
                      </td>
                      <td className="num">{m.runs.toLocaleString()}회</td>
                      <td className="num"><strong>{fmtTokens(m.totalTokens)}</strong></td>
                      <td className="num">{m.free ? "₩0 (무료)" : won(m.estKrw)}</td>
                      <td className="num">{m.free ? "$0.00" : fmtUsd(m.estUsd)}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        )}

        {activeTab === "actor" && (
          <div style={{ overflowX: "auto" }}>
            <table className="usage-table">
              <thead>
                <tr>
                  <th>직원</th>
                  <th className="num">실행 횟수</th>
                  <th className="num">입력 토큰</th>
                  <th className="num">출력 토큰</th>
                  <th className="num">총 토큰</th>
                  <th className="num">환산 비용 (원화)</th>
                </tr>
              </thead>
              <tbody>
                {(!s?.byActor || s.byActor.length === 0) ? (
                  <tr><td colSpan={6} style={{ textAlign: "center", color: "var(--muted)", padding: 24 }}>기록된 직원 사용량이 없습니다.</td></tr>
                ) : (
                  s.byActor.map((a) => (
                    <tr key={a.actor}>
                      <td><strong>{a.actor}</strong></td>
                      <td className="num">{a.runs.toLocaleString()}회</td>
                      <td className="num">{fmtTokens(a.inputTokens)}</td>
                      <td className="num">{fmtTokens(a.outputTokens)}</td>
                      <td className="num"><strong>{fmtTokens(a.totalTokens)}</strong></td>
                      <td className="num">{won(a.estKrw)}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        )}

        {activeTab === "day" && (
          <div style={{ overflowX: "auto" }}>
            <table className="usage-table">
              <thead>
                <tr>
                  <th>일자</th>
                  <th className="num">실행 횟수</th>
                  <th className="num">총 토큰</th>
                  <th className="num">환산 비용 (원화)</th>
                  <th className="num">환산 비용 (USD)</th>
                </tr>
              </thead>
              <tbody>
                {(!s?.byDay || s.byDay.length === 0) ? (
                  <tr><td colSpan={5} style={{ textAlign: "center", color: "var(--muted)", padding: 24 }}>기록된 일별 사용량이 없습니다.</td></tr>
                ) : (
                  s.byDay.map((d) => (
                    <tr key={d.day}>
                      <td><strong style={{ fontFamily: "var(--font-mono)" }}>{d.day}</strong></td>
                      <td className="num">{d.runs.toLocaleString()}회</td>
                      <td className="num"><strong>{fmtTokens(d.totalTokens)}</strong></td>
                      <td className="num">{won(d.estKrw)}</td>
                      <td className="num">{fmtUsd(d.estUsd)}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}
