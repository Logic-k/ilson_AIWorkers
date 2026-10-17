import { useState } from "react";
import { engineHue, engineShort, relativeTime } from "../api";
import { useStore } from "../store";
import { OfficeScene } from "../components/OfficeScene";
import { Avatar, Empty, Icon, StatusBadge } from "../components/ui";

const kindIcon: Record<string, string> = { typing: "⌨", reading: "📖", running: "▶", thinking: "💭", talking: "💬" };

export function OfficePage() {
  const { boot, tasks, activities, feed, employee, navigate, newTask, showResume } = useStore();
  const [selected, setSelected] = useState<string | null>(null);
  if (!boot) return null;
  const approvals = tasks.filter((t) => t.status === "awaiting_approval");
  const running = tasks.filter((t) => t.status === "running" || t.status === "queued");
  const today = new Date().toDateString();
  const doneToday = tasks.filter((t) => t.status === "completed" && new Date(t.updatedAt).toDateString() === today).length;
  const sel = selected ? employee(selected) : undefined;
  const selTask = sel && tasks.find((t) => ["running", "awaiting_approval", "queued", "paused", "failed"].includes(t.status) && (t.currentActor === sel.id || t.employeeId === sel.id));
  const safe = boot.settings.engineMode !== "auto";
  const connected = boot.engines.filter((e) => e.signedIn);

  return (
    <div className="page office-page">
      <div className="office-hud">
        <span className="hud-chip">
          <i className="dot dot--info" />
          진행 중 <b>{running.length}</b>
        </span>
        <button className={`hud-chip ${approvals.length ? "is-alert" : ""}`} onClick={() => navigate("work")}>
          <i className="dot dot--warning" />
          사장님 차례 <b>{approvals.length}</b>
        </button>
        <span className="hud-chip">
          <i className="dot dot--positive" />
          오늘 완료 <b>{doneToday}</b>
        </span>
        <span className="hud-chip">{safe ? "안전 모드 · 사장님이 실행" : boot.settings.yolo ? "자동 모드 · YOLO" : "자동 모드 · 직원들이 주고받음"}</span>
        {connected.map((e) => (
          <span key={e.id} className="hud-chip" title={e.plan}>
            <span className="engine-dot" style={{ background: `hsl(${engineHue[e.id]} 55% 45%)` }}>{engineShort[e.id]}</span>
            {e.label}
          </span>
        ))}
        <span className="hud-chip hud-chip--quiet">API 키 0개 · 구독으로 일함</span>
      </div>

      <div className="office-layout">
        <div className="office-frame">
          <OfficeScene
            employees={boot.employees}
            tasks={tasks}
            activities={activities}
            selected={selected}
            onSelect={setSelected}
            onBoss={() => navigate("work")}
            onTaskClick={(taskId) => navigate("work", taskId)}
          />
        </div>

        <aside className="office-side">
          {sel ? (
            <section className="card office-card">
              <header className="office-card__head">
                <Avatar employee={sel} size={44} showStatus />
                <div>
                  <strong>{sel.name}</strong>
                  <small>
                    {sel.role} · {sel.team}
                    {sel.engine && ` · ${sel.engine.label}${sel.engine.auto ? " (자동)" : ""}`}
                  </small>
                </div>
                <button className="icon-btn" onClick={() => setSelected(null)} aria-label="선택 해제">
                  <Icon name="close" size={16} />
                </button>
              </header>
              <p className="office-card__motto">“{sel.motto}”</p>
              {selTask ? (
                <button className="task-row" onClick={() => navigate("work", selTask.id)}>
                  <div className="task-row__body">
                    <strong>{selTask.title}</strong>
                    <small>{activities[sel.id]?.taskId === selTask.id ? activities[sel.id].label : selTask.currentStep ?? ""}</small>
                  </div>
                  <StatusBadge task={selTask.status} />
                </button>
              ) : (
                <p className="office-card__idle">지금은 맡은 일이 없어요.</p>
              )}
              <div className="office-card__actions">
                <button className="btn btn--ghost btn--sm" onClick={() => showResume(sel.id)}>
                  입사지원서
                </button>
                <button className="btn btn--primary btn--sm" onClick={() => newTask(sel.id)}>
                  <Icon name="plus" size={14} /> 업무 맡기기
                </button>
              </div>
            </section>
          ) : (
            <section className="card office-card office-card--hint">
              <strong>직원을 눌러 보세요</strong>
              <p>지금 하는 일을 보거나 바로 업무를 맡길 수 있어요. 1층 사장석을 누르면 결재함으로 갑니다.</p>
            </section>
          )}

          <section className="card">
            <header className="card__head">
              <div>
                <span className="eyebrow">결재함</span>
                <h3>사장님 차례</h3>
              </div>
              <span className="count">{approvals.length}건</span>
            </header>
            {approvals.length ? (
              approvals.map((t) => {
                const e = employee(t.employeeId);
                return (
                  <button key={t.id} className="task-row" onClick={() => navigate("work", t.id)}>
                    <Avatar employee={e} size={30} />
                    <div className="task-row__body">
                      <strong>{t.pendingGate?.title ?? t.title}</strong>
                      <small>
                        {e?.name} · {t.title}
                      </small>
                    </div>
                    <Icon name="chevron" size={14} />
                  </button>
                );
              })
            ) : (
              <Empty title="확인할 일이 없어요" />
            )}
          </section>

          <section className="card live-feed">
            <header className="card__head">
              <div>
                <span className="eyebrow">OFFICE LIVE</span>
                <h3>실시간 업무 채널</h3>
              </div>
            </header>
            {feed.length ? (
              <ul>
                {feed.slice(0, 14).map((a, i) => {
                  const e = employee(a.employeeId);
                  return (
                    <li key={`${a.at}-${i}`}>
                      <Avatar employee={e} size={22} />
                      <span>
                        <b>{e?.name}</b>
                        {a.engine && <em className="engine-dot" style={{ background: `hsl(${engineHue[a.engine]} 55% 45%)` }}>{engineShort[a.engine]}</em>} {kindIcon[a.kind] ?? ""} {a.label}
                      </span>
                      <time>{relativeTime(a.at)}</time>
                    </li>
                  );
                })}
              </ul>
            ) : (
              <Empty title="아직 조용해요">업무를 맡기면 직원들이 하는 일이 여기에 실시간으로 올라옵니다.</Empty>
            )}
          </section>
        </aside>
      </div>
    </div>
  );
}
