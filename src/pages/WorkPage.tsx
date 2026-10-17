import { useEffect, useMemo, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  api,
  errorText,
  fmtTokens,
  fmtUsd,
  isDev,
  relativeTime,
  shortModelLabel,
  won,
  type DiffSummary,
  type Task,
  type TaskStatus,
  type UsageRow,
} from "../api";
import { useStore } from "../store";
import { Avatar, Empty, Icon, StatusBadge, TokenChip } from "../components/ui";
import { DeliverableView } from "../components/DeliverableView";

const tabs: { id: string; label: string; match: TaskStatus[] }[] = [
  { id: "all", label: "전체", match: [] },
  { id: "attention", label: "확인 필요", match: ["awaiting_approval"] },
  { id: "running", label: "진행 중", match: ["running", "queued"] },
  { id: "stopped", label: "멈춤·실패", match: ["paused", "failed", "cancelled"] },
  { id: "done", label: "완료", match: ["completed"] },
];

function DiffViewer({ taskId }: { taskId: string }) {
  const [diff, setDiff] = useState<DiffSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.taskDiff(taskId).then(setDiff).catch((e) => setError(errorText(e)));
  }, [taskId]);
  if (error) return <p className="callout callout--warning">{error}</p>;
  if (!diff) return <p className="field__hint">변경 내용을 불러오는 중…</p>;
  const all = diff.patch.split("\n");
  const lines = all.slice(0, 4000);
  return (
    <div className="diff">
      <ul className="diff__files">
        {diff.files.map((f) => (
          <li key={f.path}>
            <code>{f.path}</code>
            <span className="diff__add">+{f.added}</span>
            <span className="diff__del">−{f.removed}</span>
          </li>
        ))}
      </ul>
      <pre className="diff__patch">
        {lines.map((l, i) => (
          <span key={i} className={l.startsWith("+") && !l.startsWith("+++") ? "is-add" : l.startsWith("-") && !l.startsWith("---") ? "is-del" : l.startsWith("@@") ? "is-hunk" : l.startsWith("diff ") ? "is-file" : ""}>
            {l}
            {"\n"}
          </span>
        ))}
      </pre>
      {(diff.truncated || all.length > 4000) &&<p className="field__hint">변경이 길어 앞부분만 보여 줍니다. 전체는 작업 폴더에서 확인하세요.</p>}
    </div>
  );
}

function TaskDetail({ id }: { id: string }) {
  const { employee, lastEvent, toast, refresh, newTask, navigate, activities } = useStore();
  const [task, setTask] = useState<Task | null>(null);
  const [usages, setUsages] = useState<UsageRow[]>([]);
  const [busy, setBusy] = useState(false);
  const [showDiff, setShowDiff] = useState(false);
  const [detailTab, setDetailTab] = useState<"deliverable" | "evidence" | "diff" | "tokens" | "timeline">("deliverable");

  useEffect(() => {
    let alive = true;
    api.getTask(id).then((t) => {
      if (alive) {
        setTask(t);
        api.markTaskSeen(id).catch(() => undefined);
      }
    }).catch(() => alive && setTask(null));
    api.usageForTask(id).then((u) => alive && setUsages(u)).catch(() => alive && setUsages([]));
    return () => {
      alive = false;
    };
  }, [id, lastEvent?.id === id ? lastEvent.at : 0]);

  if (!task) return <Empty title="업무를 불러오는 중입니다" />;
  const e = employee(task.employeeId);
  const totalTokens = usages.reduce((sum, u) => sum + u.inputTokens + u.outputTokens + u.cacheReadTokens + u.cacheWriteTokens, 0);
  const totalKrw = usages.reduce((sum, u) => sum + u.estKrw, 0);
  const totalUsd = usages.reduce((sum, u) => sum + (u.reportedCostUsd ?? u.estCostUsd ?? 0), 0);
  const lastUsage = usages[usages.length - 1];

  const act = async (fn: () => Promise<unknown>, done?: string) => {
    setBusy(true);
    try {
      await fn();
      if (done) toast(done, "success");
      await refresh();
    } catch (err) {
      toast(errorText(err), "error");
    } finally {
      setBusy(false);
    }
  };
  const gate = task.pendingGate;
  const safe = task.input._engine !== "auto";
  const stepDef = e?.steps.find((s) => s.label === task.currentStep);
  const watching = safe && task.status === "running" && stepDef?.gate === "handoff";
  const live = task.currentActor ? activities[task.currentActor] : undefined;
  const isCode = isDev(task.employeeId);
  const copy = async (text: string, msg: string) => {
    await navigator.clipboard.writeText(text);
    toast(msg, "success");
  };

  return (
    <article className="detail task-detail">
      <header className="task-detail__head">
        <div className="task-detail__who">
          <Avatar employee={e} size={36} />
          <span>
            {e?.name} · {e?.role}
          </span>
          <StatusBadge task={task.status} />
          <span className="tag">{safe ? "안전 모드" : "자동 모드"}</span>
        </div>
        <h2>{task.title}</h2>
        <p>
          맡긴 때 {new Date(task.createdAt).toLocaleString("ko-KR")} · 마지막 변경 {relativeTime(task.updatedAt)}
        </p>
        {usages.length > 0 && (
          <dl className="task-header__tokens">
            <dt>총 토큰:</dt>
            <dd>{fmtTokens(totalTokens)}</dd>
            <dt>환산 비용:</dt>
            <dd>{won(totalKrw)} ({fmtUsd(totalUsd)})</dd>
            {lastUsage && (
              <>
                <dt>엔진:</dt>
                <dd>{lastUsage.engine}</dd>
                <dt>모델:</dt>
                <dd>{shortModelLabel(lastUsage.model)}</dd>
              </>
            )}
          </dl>
        )}
      </header>

      {["running", "queued", "awaiting_approval"].includes(task.status) && (
        <div className="progress">
          <div className="progress__bar">
            <i style={{ width: `${task.progress}%` }} />
          </div>
          <span>
            {task.currentStep ?? "준비 중"} · {task.progress}%
            {task.status === "running" && live && live.taskId === task.id && ` · ${employee(live.employeeId)?.name ?? ""}: ${live.label}`}
          </span>
        </div>
      )}

      {task.status === "awaiting_approval" && gate && gate.kind === "handoff" && (
        <section className="gate-card gate-card--handoff">
          <span className="eyebrow">사장님 차례 · 직접 실행</span>
          <h3>{gate.title}</h3>
          <p className="gate-card__summary">{gate.summary}</p>
          <ol className="handoff-steps">
            {gate.details.map((d) => (
              <li key={d}>{d.replace(/^\d+\.\s*/, "")}</li>
            ))}
          </ol>
          <div className="handoff-tools">
            <button className="btn btn--sm" disabled={!gate.folder} onClick={() => gate.folder && act(() => api.openTerminal(gate.folder!), "작업 폴더에서 터미널을 열었습니다.")}>
              <Icon name="terminal" size={14} /> 터미널 열기
            </button>
            {gate.prompt && (
              <button className="btn btn--ghost btn--sm" onClick={() => copy(gate.prompt!, "에이전트에게 할 말을 복사했습니다.")}>
                <Icon name="copy" size={14} /> 할 말 복사
              </button>
            )}
            {gate.command && (
              <button className="btn btn--ghost btn--sm" onClick={() => copy(gate.command!, "터미널 명령을 복사했습니다.")}>
                <Icon name="copy" size={14} /> 터미널 명령 복사
              </button>
            )}
            {gate.folder && (
              <button className="btn btn--ghost btn--sm" onClick={() => revealItemInDir(gate.folder!)}>
                <Icon name="folder" size={14} /> 폴더 보기
              </button>
            )}
          </div>
          <div className="gate-card__actions">
            <button className="btn btn--ghost" disabled={busy} onClick={() => act(() => api.declineTask(task.id), "이번 단계는 진행하지 않았습니다.")}>
              이번엔 안 할게요
            </button>
            <button className="btn btn--primary" disabled={busy} onClick={() => act(() => api.approveTask(task.id), "지켜보기 시작합니다. 보고서가 생기면 자동으로 넘어갑니다.")}>
              <Icon name="play" size={16} /> 실행했어요 · 지켜봐 주세요
            </button>
          </div>
        </section>
      )}

      {task.status === "awaiting_approval" && gate && gate.kind !== "handoff" && (
        <section className="gate-card">
          <span className="eyebrow">{gate.kind === "merge" ? "병합 확인" : "변경 확인"}</span>
          <h3>{gate.title}</h3>
          <p className="gate-card__summary">{gate.summary}</p>
          <ul>
            {gate.details.map((d) => (
              <li key={d}>{d}</li>
            ))}
          </ul>
          {gate.kind === "merge" && (
            <div className="handoff-tools">
              <button className="btn btn--ghost btn--sm" onClick={() => setShowDiff((v) => !v)}>
                <Icon name="doc" size={14} /> {showDiff ? "변경 내용 닫기" : "변경 내용 보기"}
              </button>
              {gate.folder && (
                <>
                  <button className="btn btn--ghost btn--sm" onClick={() => act(() => api.openTerminal(gate.folder!), "작업 폴더에서 터미널을 열었습니다.")}>
                    <Icon name="terminal" size={14} /> 터미널 열기
                  </button>
                  <button className="btn btn--ghost btn--sm" onClick={() => revealItemInDir(gate.folder!)}>
                    <Icon name="folder" size={14} /> 작업 폴더 열기
                  </button>
                </>
              )}
            </div>
          )}
          {showDiff && <DiffViewer taskId={task.id} />}
          <div className="gate-card__actions">
            <button className="btn btn--ghost" disabled={busy} onClick={() => act(() => api.declineTask(task.id), gate.kind === "merge" ? "병합하지 않았습니다. 작업 브랜치는 남아 있습니다." : "이번 단계는 진행하지 않았습니다.")}>
              {gate.kind === "merge" ? "병합하지 않기" : "이번엔 안 할게요"}
            </button>
            <button className="btn btn--primary" disabled={busy} onClick={() => act(() => api.approveTask(task.id), "확인했습니다. 업무를 이어갑니다.")}>
              <Icon name="check" size={16} /> {gate.kind === "merge" ? "병합하기" : "옮겨 주세요"}
            </button>
          </div>
        </section>
      )}

      {watching && (
        <section className="callout callout--info watch-card">
          <strong>직원이 사장님 구독 CLI에서 일하는 중이에요</strong>
          <p>직원들이 남기는 상태·보고서 파일과 바뀐 파일을 지켜보고 있습니다. 보고서가 생기면 자동으로 다음 단계로 넘어갑니다.</p>
          <div className="handoff-tools">
            <button
              className="btn btn--sm"
              disabled={busy}
              onClick={() => act(() => api.finishExternal(task.id), "완료로 처리합니다.")}
            >
              <Icon name="check" size={14} /> 끝났어요 (완료 처리)
            </button>
          </div>
        </section>
      )}

      {task.status === "failed" && (
        <section className="callout callout--critical">
          <strong>업무가 멈췄습니다</strong>
          <p>{task.error}</p>
          <small>끝낸 단계는 저장되어 있어, 이어하기를 누르면 실패한 단계부터 다시 합니다.</small>
        </section>
      )}

      <div className="task-detail__actions">
        {task.status === "running" && (
          <button className="btn btn--ghost btn--sm" disabled={busy} onClick={() => act(() => api.pauseTask(task.id), "멈춤을 요청했습니다.")}>
            <Icon name="pause" size={16} /> 멈추기
          </button>
        )}
        {["paused", "failed", "cancelled"].includes(task.status) && (
          <button className="btn btn--primary btn--sm" disabled={busy} onClick={() => act(() => api.resumeTask(task.id))}>
            <Icon name="play" size={16} /> 이어하기
          </button>
        )}
        {isCode && ["paused", "failed", "cancelled"].includes(task.status) && (
          <button
            className="btn btn--ghost btn--sm"
            disabled={busy}
            onClick={async () => {
              const ok = await ask("병합하지 않은 작업 공간과 작업 브랜치를 지울까요?\n원래 저장소는 바뀌지 않습니다.", { title: "작업 공간 지우기", kind: "warning", okLabel: "지우기", cancelLabel: "취소" });
              if (ok) act(() => api.discardTask(task.id), "작업 공간을 지웠습니다.");
            }}
          >
            <Icon name="trash" size={16} /> 작업 공간 지우기
          </button>
        )}
        <button className="btn btn--ghost btn--sm" onClick={() => newTask(task.employeeId, Object.fromEntries(Object.entries(task.input).filter(([k]) => !k.startsWith("_"))))}>
          <Icon name="refresh" size={16} /> 같은 내용으로 다시 맡기기
        </button>
        {task.status !== "running" && (
          <button
            className="btn btn--ghost btn--sm btn--danger"
            disabled={busy}
            onClick={async () => {
              const ok = await ask("이 업무와 결과를 목록에서 지울까요?\n(파일 정리로 옮긴 파일은 그대로 남습니다)", { title: "업무 지우기", kind: "warning", okLabel: "지우기", cancelLabel: "취소" });
              if (ok) act(() => api.deleteTask(task.id).then(() => navigate("work")), "업무를 지웠습니다.");
            }}
          >
            <Icon name="trash" size={16} /> 지우기
          </button>
        )}
      </div>

      <nav className="tabs" style={{ marginTop: 20 }}>
        <button
          className={`tab ${detailTab === "deliverable" ? "is-active" : ""}`}
          onClick={() => setDetailTab("deliverable")}
        >
          결과물
        </button>
        <button
          className={`tab ${detailTab === "evidence" ? "is-active" : ""}`}
          onClick={() => setDetailTab("evidence")}
        >
          증빙
        </button>
        <button
          className={`tab ${detailTab === "diff" ? "is-active" : ""}`}
          onClick={() => setDetailTab("diff")}
        >
          변경 내용
        </button>
        <button
          className={`tab ${detailTab === "tokens" ? "is-active" : ""}`}
          onClick={() => setDetailTab("tokens")}
        >
          토큰 {usages.length > 0 && <span className="tab__badge">{fmtTokens(totalTokens)}</span>}
        </button>
        <button
          className={`tab ${detailTab === "timeline" ? "is-active" : ""}`}
          onClick={() => setDetailTab("timeline")}
        >
          기록 <span className="tab__badge">{task.events.length}</span>
        </button>
      </nav>

      {detailTab === "deliverable" && (
        <section className="result">
          {task.result ? (
            <DeliverableView task={task} result={task.result} />
          ) : (
            <Empty title="아직 완성된 결과물이 없습니다">
              {task.status === "running" ? "직원이 업무를 진행하고 있습니다." : "업무를 진행하면 여기에 최종 결과물이 표시됩니다."}
            </Empty>
          )}
        </section>
      )}

      {detailTab === "evidence" && (
        <section className="card">
          <h3>검증 및 증빙 자료</h3>
          {task.result?.kind === "document" && task.result.checks && task.result.checks.length > 0 && (
            <div style={{ marginBottom: 16 }}>
              <h4>자동 점검 항목</h4>
              <ul style={{ paddingLeft: 20, margin: "8px 0" }}>
                {task.result.checks.map((c, i) => (
                  <li key={i} style={{ color: c.ok ? "var(--ok)" : "var(--warn)" }}>
                    <strong>{c.label}</strong>: {c.value} ({c.ok ? "통과" : "주의"})
                  </li>
                ))}
              </ul>
            </div>
          )}
          {task.result?.kind === "document" && task.result.sources && task.result.sources.length > 0 && (
            <div style={{ marginBottom: 16 }}>
              <h4>참고 자료 및 출처</h4>
              <ul style={{ paddingLeft: 20, margin: "8px 0" }}>
                {task.result.sources.map((s, i) => (
                  <li key={i}>
                    <a href={s.url} target="_blank" rel="noreferrer">{s.title || s.url}</a>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {task.result?.kind === "code" && (
            <div>
              <h4>코드 검증 결과</h4>
              <p>판정: <strong>{task.result.verdict || "정상 검증 완료"}</strong></p>
              {task.result.mergeCommit && <p>커밋 해시: <code>{task.result.mergeCommit}</code></p>}
            </div>
          )}
          {(!task.result || (task.result.kind !== "document" && task.result.kind !== "code")) && (
            <Empty title="별도의 증빙 자료가 없습니다">
              업무 실행 중 수집된 검증 데이터와 출처가 여기에 정리됩니다.
            </Empty>
          )}
        </section>
      )}

      {detailTab === "diff" && (
        <section className="card">
          <h3>작업 공간 변경 내용</h3>
          <DiffViewer taskId={task.id} />
        </section>
      )}

      {detailTab === "tokens" && (
        <section className="card">
          <h3>단계별 토큰 및 환산 비용</h3>
          {usages.length === 0 ? (
            <Empty title="기록된 토큰 사용량이 없습니다">
              자동 모드로 실행하거나 구독 CLI를 통해 작업이 진행되면 단계별 토큰이 기록됩니다.
            </Empty>
          ) : (
            <div style={{ overflowX: "auto", marginTop: 12 }}>
              <table className="usage-table">
                <thead>
                  <tr>
                    <th>단계</th>
                    <th>담당 직원</th>
                    <th>엔진</th>
                    <th>모델</th>
                    <th className="num">입력 토큰</th>
                    <th className="num">출력 토큰</th>
                    <th className="num">환산 비용</th>
                    <th>기록 시각</th>
                  </tr>
                </thead>
                <tbody>
                  {usages.map((u) => (
                    <tr key={u.id}>
                      <td><strong>{u.stepKey}</strong></td>
                      <td>{u.actor}</td>
                      <td>{u.engine}</td>
                      <td>
                        {shortModelLabel(u.model)}
                        {u.estCostUsd === 0 && <span className="model-chip model-chip--free" style={{ marginLeft: 4 }}>무료</span>}
                      </td>
                      <td className="num">{fmtTokens(u.inputTokens)}</td>
                      <td className="num">{fmtTokens(u.outputTokens)}</td>
                      <td className="num">{won(u.estKrw)}</td>
                      <td style={{ fontSize: 11.5, color: "var(--muted)" }}>
                        {new Date(u.at).toLocaleTimeString("ko-KR", { hour: "2-digit", minute: "2-digit", second: "2-digit" })}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>
      )}

      {detailTab === "timeline" && (
        <>
          <section className="timeline">
            <h3>업무 진행</h3>
            <ol>
              {task.steps.map((s) => {
                const stepUsage = usages.find((u) => u.stepKey === s.key);
                return (
                  <li key={s.key} className={`timeline__step timeline__step--${s.status}`}>
                    <i />
                    <div>
                      <strong>{s.label}</strong>
                      {s.note && <p>{s.note}</p>}
                      {stepUsage && (
                        <div style={{ marginTop: 4 }}>
                          <TokenChip
                            model={stepUsage.model}
                            input={stepUsage.inputTokens}
                            output={stepUsage.outputTokens}
                            krw={stepUsage.estKrw}
                            free={stepUsage.estCostUsd === 0}
                          />
                        </div>
                      )}
                    </div>
                    <time>{s.finishedAt ? new Date(s.finishedAt).toLocaleTimeString("ko-KR", { hour: "2-digit", minute: "2-digit" }) : ""}</time>
                  </li>
                );
              })}
            </ol>
          </section>

          <details className="event-log">
            <summary>업무 기록 {task.events.length}건</summary>
            <ul>
              {task.events.map((ev, i) => (
                <li key={i} className={`event event--${ev.level}`}>
                  <time>{new Date(ev.at).toLocaleString("ko-KR", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", second: "2-digit" })}</time>
                  <span>{ev.message}</span>
                </li>
              ))}
            </ul>
          </details>
        </>
      )}
    </article>
  );
}

export function WorkPage() {
  const { tasks, selectedTaskId, navigate, employee, newTask } = useStore();
  const [tab, setTab] = useState("all");
  const filtered = useMemo(() => {
    const t = tabs.find((x) => x.id === tab)!;
    return t.match.length ? tasks.filter((x) => t.match.includes(x.status)) : tasks;
  }, [tasks, tab]);
  const selected = selectedTaskId && tasks.some((t) => t.id === selectedTaskId) ? selectedTaskId : filtered[0]?.id;

  return (
    <div className="page">
      <div className="toolbar">
        <div className="tabs" role="tablist">
          {tabs.map((t) => {
            const count = t.match.length ? tasks.filter((x) => t.match.includes(x.status)).length : tasks.length;
            return (
              <button key={t.id} role="tab" aria-selected={tab === t.id} className={`tab ${tab === t.id ? "is-on" : ""}`} onClick={() => setTab(t.id)}>
                {t.label} <b>{count}</b>
              </button>
            );
          })}
        </div>
      </div>
      {tasks.length === 0 ? (
        <div className="card">
          <Empty title="아직 맡긴 업무가 없어요">
            <button className="btn btn--primary" onClick={() => newTask()}>
              <Icon name="plus" size={16} /> 첫 업무 맡기기
            </button>
          </Empty>
        </div>
      ) : (
        <div className="master-detail">
          <aside className="master">
            {filtered.map((t) => {
              const e = employee(t.employeeId);
              const isUnread = (!t.seenAt || new Date(t.updatedAt).getTime() > new Date(t.seenAt).getTime()) && t.status !== "queued";
              return (
                <button
                  key={t.id}
                  className={`work-item ${selected === t.id ? "is-on" : ""} ${t.status === "awaiting_approval" ? "task-card--waiting" : ""}`}
                  onClick={() => navigate("work", t.id)}
                >
                  <div className="work-item__top">
                    <span>
                      <Avatar employee={e} size={22} /> {e?.name}
                    </span>
                    <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                      {isUnread && selected !== t.id && <span className="inbox-badge">새 소식</span>}
                      <StatusBadge task={t.status} />
                    </div>
                  </div>
                  <strong>{t.title}</strong>
                  <small>{relativeTime(t.updatedAt)}{t.currentStep && t.status === "running" ? ` · ${t.currentStep}` : ""}</small>
                  {(t.status === "running" || t.status === "queued") && (
                    <span className="mini-progress mini-progress--full">
                      <i style={{ width: `${t.progress}%` }} />
                    </span>
                  )}
                </button>
              );
            })}
            {!filtered.length && <Empty title="이 상태의 업무가 없어요" />}
          </aside>
          {selected ? <TaskDetail id={selected} /> : <Empty title="업무를 골라 주세요" />}
        </div>
      )}
    </div>
  );
}
