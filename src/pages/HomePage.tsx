import { useEffect, useState } from "react";
import { api, fmtTokens, relativeTime, won, type Task, type UsageSummary } from "../api";
import { useStore } from "../store";
import { Avatar, Empty, Icon, StatusBadge } from "../components/ui";

function TaskRow({ task }: { task: Task }) {
  const { employee, navigate } = useStore();
  const e = employee(task.employeeId);
  return (
    <button className="task-row" onClick={() => navigate("work", task.id)}>
      <Avatar employee={e} size={34} />
      <div className="task-row__body">
        <strong>{task.title}</strong>
        <small>
          {e?.name} · {relativeTime(task.updatedAt)}
          {task.status === "awaiting_approval" && task.pendingGate ? ` · ${task.pendingGate.summary}` : ""}
          {task.status === "failed" && task.error ? ` · ${task.error}` : ""}
        </small>
      </div>
      {task.status === "running" ? (
        <span className="mini-progress">
          <i style={{ width: `${task.progress}%` }} />
          <em>{task.progress}%</em>
        </span>
      ) : (
        <StatusBadge task={task.status} />
      )}
    </button>
  );
}

export function HomePage() {
  const { boot, tasks, newTask, navigate } = useStore();
  const [summary, setSummary] = useState<UsageSummary | null>(null);

  useEffect(() => {
    api.usageSummary("7d").then(setSummary).catch(() => undefined);
  }, []);

  if (!boot) return null;
  const attention = tasks.filter((t) => ["awaiting_approval", "failed", "paused"].includes(t.status));
  const awaitingApproval = tasks.filter((t) => t.status === "awaiting_approval");
  const newResults = tasks.filter((t) => t.status === "completed" && (!t.seenAt || new Date(t.updatedAt).getTime() > new Date(t.seenAt).getTime()));
  const running = tasks.filter((t) => t.status === "running" || t.status === "queued");
  const done = tasks.filter((t) => t.status === "completed");
  const weekAgo = Date.now() - 7 * 86400_000;
  const doneWeek = done.filter((t) => new Date(t.updatedAt).getTime() > weekAgo).length;
  const setup = boot.employees.filter((e) => e.availability.status === "setup_required");
  const usedEngines = new Set(boot.employees.map((e) => e.engine?.id).filter(Boolean));
  const engineMissing = boot.engines.some((e) => usedEngines.has(e.id) && e.installed && !e.signedIn && e.message !== "확인 중");
  const safe = boot.settings.engineMode !== "auto" || !boot.settings.autoAckAt;

  return (
    <div className="page">
      {(awaitingApproval.length > 0 || newResults.length > 0) && (
        <div
          className="inbox-banner"
          onClick={() => {
            const target = awaitingApproval[0] || newResults[0];
            if (target) navigate("work", target.id);
            else navigate("work");
          }}
        >
          <span>
            📥 <strong>인박스</strong>: 사장님 차례 <strong>{awaitingApproval.length}건</strong> · 새 결과 <strong>{newResults.length}건</strong>
          </span>
          <span style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
            업무 바로 확인하기 <Icon name="chevron" size={14} />
          </span>
        </div>
      )}

      <section className="hero">
        <div className="hero__copy">
          <span className="eyebrow">오늘의 회사</span>
          <h2>
            {attention.length > 0 ? (
              <>
                사장님 확인이 필요한 일이
                <br />
                <em>{attention.length}건</em> 있어요.
              </>
            ) : running.length > 0 ? (
              <>
                직원들이 <em>{running.length}건</em>을
                <br />
                처리하고 있어요.
              </>
            ) : (
              <>
                맡길 일 하나를 골라
                <br />
                <em>직원에게</em> 넘겨 보세요.
              </>
            )}
          </h2>
          <p>
            API 키 없이 사장님 구독(Antigravity·Claude Code·Codex·OpenCode)으로 일합니다. 코드 병합과 파일 이동은 항상 사장님 확인을 받은 뒤에 진행합니다.
            {summary && summary.yesterdayTasks > 0 && (
              <span style={{ display: "block", marginTop: 4, color: "var(--brand)" }}>
                어제 완료: {summary.yesterdayTasks}건 · {fmtTokens(summary.yesterdayTokens)} 토큰
              </span>
            )}
          </p>
          <button className="btn btn--primary" onClick={() => newTask()}>
            <Icon name="plus" size={16} /> 새 업무 맡기기
          </button>
        </div>
        <div className="hero__stats">
          <div className="ring" style={{ ["--p" as string]: `${Math.min(100, running.length * 25)}%` }}>
            <strong>{running.length}</strong>
            <span>진행 중</span>
          </div>
          <ul>
            <li><i className="dot dot--warning" />확인 필요 <b>{attention.length}</b></li>
            <li><i className="dot dot--positive" />이번 주 완료 <b>{doneWeek}</b></li>
            <li><i className="dot dot--positive" />이번 주 환산 <b>{won(summary?.weekKrw ?? 0)}</b></li>
            <li><i className="dot dot--neutral" />실행 방식 <b>{safe ? "안전 모드" : "자동 모드"}</b></li>
          </ul>
        </div>
      </section>

      {(engineMissing || setup.length > 0) && (
        <section className="callout callout--info setup-banner">
          <div>
            <strong>{engineMissing ? "담당 구독 중 로그인이 필요한 엔진이 있어요." : `설정이 필요한 직원이 ${setup.length}명 있어요.`}</strong>
            <p>
              {setup.map((e) => `${e.name}(${e.availability.missing.join(", ")})`).join(" · ")}
            </p>
          </div>
          <button className="btn btn--sm" onClick={() => navigate("connections")}>
            연결 및 설정 <Icon name="chevron" size={14} />
          </button>
        </section>
      )}

      <div className="home-grid">
        <section className="card">
          <header className="card__head">
            <div>
              <span className="eyebrow">사장님 차례</span>
              <h3>확인이 필요해요</h3>
            </div>
            <span className="count">{attention.length}건</span>
          </header>
          {attention.length ? attention.slice(0, 6).map((t) => <TaskRow key={t.id} task={t} />) : <Empty title="지금은 확인할 일이 없어요" />}
        </section>
        <section className="card">
          <header className="card__head">
            <div>
              <span className="eyebrow">진행 상황</span>
              <h3>처리 중인 업무</h3>
            </div>
            <button className="link-btn" onClick={() => navigate("work")}>
              전체 업무 <Icon name="chevron" size={14} />
            </button>
          </header>
          {running.length ? running.slice(0, 6).map((t) => <TaskRow key={t.id} task={t} />) : <Empty title="처리 중인 업무가 없어요" />}
        </section>
        <section className="card">
          <header className="card__head">
            <div>
              <span className="eyebrow">결과물</span>
              <h3>최근 결과</h3>
            </div>
          </header>
          {done.length ? done.slice(0, 6).map((t) => <TaskRow key={t.id} task={t} />) : <Empty title="아직 완료된 업무가 없어요">첫 업무를 맡기면 결과가 여기에 쌓입니다.</Empty>}
        </section>
        <section className="card">
          <header className="card__head">
            <div>
              <span className="eyebrow">바로 맡기기</span>
              <h3>우리 직원들</h3>
            </div>
            <button className="link-btn" onClick={() => navigate("employees")}>
              모든 직원 <Icon name="chevron" size={14} />
            </button>
          </header>
          <div className="staff-grid">
            {boot.employees.map((e) => (
              <button key={e.id} className="staff-chip" onClick={() => newTask(e.id)}>
                <Avatar employee={e} size={38} showStatus />
                <span>
                  <strong>{e.name}</strong>
                  <small>{e.role}</small>
                </span>
              </button>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
