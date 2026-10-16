import { useMemo, useState } from "react";
import { isDev, modeLabel, relativeTime } from "../api";
import { useStore } from "../store";
import { Avatar, Empty, Icon, StatusBadge } from "../components/ui";
import { GuardAck } from "../components/NewTaskDialog";

const filters: { id: "all" | "dev" | "office"; label: string }[] = [
  { id: "all", label: "전체" },
  { id: "dev", label: "개발팀" },
  { id: "office", label: "사무팀" },
];

export function EmployeesPage() {
  const { boot, tasks, newTask, showResume, navigate } = useStore();
  const [filter, setFilter] = useState<"all" | "dev" | "office">("all");
  const [q, setQ] = useState("");
  const [selected, setSelected] = useState("taeo");
  const list = useMemo(
    () =>
      (boot?.employees ?? []).filter(
        (e) =>
          (filter === "all" || (filter === "dev") === isDev(e.id)) &&
          (!q || [e.name, e.role, e.team, e.summary, ...e.capabilities].join(" ").includes(q.trim())),
      ),
    [boot, filter, q],
  );
  if (!boot) return null;
  const e = list.find((x) => x.id === selected) ?? list[0];
  const lastTask = e && tasks.find((t) => t.employeeId === e.id);

  return (
    <div className="page">
      <div className="toolbar">
        <label className="search">
          <Icon name="search" size={16} />
          <input value={q} onChange={(ev) => setQ(ev.target.value)} placeholder="이름, 역할, 할 수 있는 일로 찾기" />
        </label>
        <div className="chips">
          {filters.map((f) => (
            <button key={f.id} className={`chip ${filter === f.id ? "is-on" : ""}`} onClick={() => setFilter(f.id)}>
              {f.label}
            </button>
          ))}
        </div>
      </div>
      <div className="master-detail">
        <aside className="master">
          <span className="master__count">{list.length}명</span>
          {list.map((x) => (
            <button key={x.id} className={`emp-item ${e?.id === x.id ? "is-on" : ""}`} onClick={() => setSelected(x.id)}>
              <Avatar employee={x} size={42} showStatus />
              <span>
                <strong>{x.name}</strong>
                <small>{x.role}</small>
                <span className="emp-item__tags">
                  <StatusBadge employee={x.availability.status} />
                  <em>{modeLabel[x.mode]}</em>
                </span>
              </span>
            </button>
          ))}
          {!list.length && <Empty title="조건에 맞는 직원이 없어요" />}
        </aside>
        {e && (
          <article className="detail emp-detail">
            <header className="emp-detail__head">
              <Avatar employee={e} size={84} showStatus />
              <div>
                <div className="emp-detail__badges">
                  <StatusBadge employee={e.availability.status} />
                  <span className="tag">{modeLabel[e.mode]}</span>
                </div>
                <h2>{e.name}</h2>
                <p>
                  {e.role} · {e.team}
                </p>
              </div>
            </header>
            <p className="emp-detail__summary">{e.summary}</p>
            <blockquote className="motto">“{e.motto}”</blockquote>
            <dl className="info-grid">
              <div>
                <dt>받는 정보</dt>
                <dd>{e.inputSummary}</dd>
              </div>
              <div>
                <dt>만드는 결과</dt>
                <dd>{e.outputSummary}</dd>
              </div>
              <div>
                <dt>비용 기준</dt>
                <dd>{e.costSummary}</dd>
              </div>
              <div>
                <dt>최근 업무</dt>
                <dd>{lastTask ? `${relativeTime(lastTask.updatedAt)} · ${lastTask.title}` : "아직 없음"}</dd>
              </div>
            </dl>
            <section>
              <h4>할 수 있는 일</h4>
              <div className="tags">
                {e.capabilities.map((c) => (
                  <span key={c} className="tag">
                    {c}
                  </span>
                ))}
              </div>
            </section>
            <section>
              <h4>업무 흐름</h4>
              <ol className="flow">
                {e.steps.map((s) => (
                  <li key={s.key}>
                    {s.label}
                    {s.gate && <em>{s.gate === "cost" ? "비용 확인" : "변경 확인"}</em>}
                  </li>
                ))}
              </ol>
            </section>
            {e.availability.status === "setup_required" && (
              <div className="callout callout--warning">
                <strong>먼저 준비해 주세요: {e.availability.missing.join(", ")}</strong>
                <button className="btn btn--sm" onClick={() => navigate("connections")}>
                  연결 및 설정으로
                </button>
              </div>
            )}
            {e.availability.status === "guarded" && (
              <div className="callout callout--critical">
                <strong>같은 문제로 두 번 실패해 잠시 멈춰 두었습니다.</strong>
                <p>{e.availability.guard}</p>
                <GuardAck employeeId={e.id} />
              </div>
            )}
            {e.notice && <p className="preflight__notice">{e.notice}</p>}
            <footer className="detail__actions">
              <button className="btn btn--ghost" onClick={() => showResume(e.id)}>
                입사지원서 보기
              </button>
              <button className="btn btn--primary" onClick={() => newTask(e.id)}>
                <Icon name="plus" size={16} /> 업무 맡기기
              </button>
            </footer>
          </article>
        )}
      </div>
    </div>
  );
}
