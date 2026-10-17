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
  type StepOutput,
  type Task,
  type TaskStatus,
  type UsageRow,
} from "../api";
import { useStore } from "../store";
import { Avatar, Empty, Icon, Markdown, StatusBadge, TokenChip } from "../components/ui";
import { DeliverableView } from "../components/DeliverableView";

const tabs: { id: string; label: string; match: TaskStatus[] }[] = [
  { id: "all", label: "전체", match: [] },
  { id: "attention", label: "확인 필요", match: ["awaiting_approval"] },
  { id: "running", label: "진행 중", match: ["running", "queued"] },
  { id: "stopped", label: "멈춤·실패", match: ["paused", "failed", "cancelled"] },
  { id: "done", label: "완료", match: ["completed"] },
];

const STEP_ACTOR_FALLBACK: Record<string, string> = {
  research: "seojin",
  plan: "taeo",
  backend: "geonwoo",
  frontend: "yuna",
  test: "sohee",
  review: "minhyuk",
  report: "taeo",
};

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
  const [stepOutputs, setStepOutputs] = useState<StepOutput[]>([]);
  const [selectedStepKey, setSelectedStepKey] = useState<string | null>(null);
  const [followupPrompt, setFollowupPrompt] = useState("");
  const [submittingFollowup, setSubmittingFollowup] = useState(false);
  const [busy, setBusy] = useState(false);
  const [showDiff, setShowDiff] = useState(false);
  const [detailTab, setDetailTab] = useState<"deliverable" | "evidence" | "diff" | "tokens" | "timeline">("deliverable");

  useEffect(() => {
    setSelectedStepKey(null);
    setFollowupPrompt("");
  }, [id]);

  useEffect(() => {
    let alive = true;
    api.getTask(id).then((t) => {
      if (alive) {
        setTask(t);
        api.markTaskSeen(id).catch(() => undefined);
      }
    }).catch(() => alive && setTask(null));
    api.usageForTask(id).then((u) => alive && setUsages(u)).catch(() => alive && setUsages([]));
    api.taskStepOutputs(id).then((outs) => alive && setStepOutputs(outs)).catch(() => alive && setStepOutputs([]));
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
  const liveByActor = task.currentActor ? activities[task.currentActor] : undefined;
  const liveForTask = Object.values(activities).find((a) => a.taskId === task.id);
  const live = liveByActor && liveByActor.taskId === task.id ? liveByActor : liveForTask;
  const activeActorEmp = employee(live?.employeeId ?? task.currentActor ?? task.employeeId);
  const isCode = isDev(task.employeeId);

  const outputFolder =
    (task.result?.kind === "code" ? task.result.workspace?.repo : undefined) ||
    (task.result?.kind === "organize" ? task.result.folder : undefined) ||
    (typeof task.input.folder === "string" && task.input.folder.trim() ? task.input.folder.trim() : undefined);

  const entryHtml = task.result?.kind === "code" ? task.result.entryHtml : undefined;
  const followupRound = Number(task.input._followupRound ?? 1);
  const followupHistory = Array.isArray(task.input._followupHistory)
    ? (task.input._followupHistory as { round?: number; prompt?: string; at?: string }[])
    : [];

  const selectedStepOutput = stepOutputs.find((o) => o.key === selectedStepKey);

  const copy = async (text: string, msg: string) => {
    await navigator.clipboard.writeText(text);
    toast(msg, "success");
  };

  const handleFollowupSubmit = async () => {
    const trimmed = followupPrompt.trim();
    if (!trimmed) {
      toast("추가로 지시할 업무 내용을 입력해 주세요.", "error");
      return;
    }
    setSubmittingFollowup(true);
    try {
      const updated = await api.followupTask(task.id, trimmed);
      setTask(updated);
      setFollowupPrompt("");
      toast("추가 업무를 배정하고 같은 작업 공간에서 바로 이어서 시작했습니다!", "success");
      await refresh();
    } catch (err) {
      toast(errorText(err), "error");
    } finally {
      setSubmittingFollowup(false);
    }
  };

  const followupSuggestions = isCode
    ? [
        "다크 모드와 테마 색상 전환 기능 추가해줘",
        "UI 디자인을 더 세련되게 다듬고 애니메이션 효과 넣어줘",
        "키보드 단축키와 데이터 내보내기·저장 기능 추가해줘",
        "버그나 레이아웃 깨짐 없는지 점검하고 반응형 보강해줘",
      ]
    : [
        "상단에 핵심 요약 표와 실행 체크리스트를 추가해줘",
        "구체적인 수치와 실제 활용 예시를 더 풍부하게 보강해줘",
        "고객에게 바로 보낼 수 있게 말투를 더 정중하고 설득력 있게 다듬어줘",
      ];

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
          {followupRound > 1 && <span className="tag" style={{ background: "rgba(37,99,235,0.12)", color: "#1d4ed8", fontWeight: 700 }}>{followupRound}차 연속 작업</span>}
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

      {/* ───────── 실시간 AI 협업 스튜디오 (OpenHands / ChatDev 스타일 워크스테이션) ───────── */}
      <section className={`live-studio ${task.status === "running" ? "is-running" : ""}`}>
        <div className="live-studio__head">
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span className={`live-studio__dot ${task.status === "running" ? "is-on" : ""}`} />
            <strong>⚡ 실시간 AI 협업 워크스테이션</strong>
            <span className="live-studio__sub">
              {task.status === "running"
                ? `${activeActorEmp?.name ?? "직원"}(${activeActorEmp?.role ?? "담당"})님이 작업 중 · ${task.progress}%`
                : `진행률 ${task.progress}% · 완료된 단계 카드를 누르면 중간 산출물을 바로 열어볼 수 있습니다`}
            </span>
          </div>
          {outputFolder && (
            <button className="btn btn--ghost btn--sm" onClick={() => revealItemInDir(outputFolder)}>
              <Icon name="folder" size={14} /> 작업 폴더 열기
            </button>
          )}
        </div>

        {/* SOP 릴레이 파이프라인 바 */}
        <div className="sop-relay">
          {task.steps.map((s, idx) => {
            const actorId = e?.steps.find((es) => es.key === s.key)?.actor ?? STEP_ACTOR_FALLBACK[s.key] ?? task.employeeId;
            const stepEmp = employee(actorId);
            const hasOutput = stepOutputs.some((o) => o.key === s.key);
            const isSelected = selectedStepKey === s.key;
            return (
              <button
                key={s.key}
                type="button"
                className={`sop-node sop-node--${s.status} ${isSelected ? "is-selected" : ""} ${hasOutput ? "has-output" : ""}`}
                onClick={() => {
                  if (hasOutput) {
                    setSelectedStepKey(isSelected ? null : s.key);
                  } else if (s.note) {
                    toast(`${s.label}: ${s.note}`, "info");
                  }
                }}
                title={hasOutput ? "클릭하여 이 단계의 산출물 보기" : s.note ?? s.label}
              >
                <div className="sop-node__top">
                  <span className="sop-node__idx">STEP {idx + 1}</span>
                  <span className={`sop-node__status sop-node__status--${s.status}`}>
                    {s.status === "done" ? "✓ 완료" : s.status === "running" ? "● 작업 중" : s.status === "failed" ? "✕ 실패" : "대기"}
                  </span>
                </div>
                <strong className="sop-node__title">{s.label}</strong>
                <div className="sop-node__who">
                  <Avatar employee={stepEmp} size={18} />
                  <span>{stepEmp?.name ?? actorId}</span>
                  {hasOutput && <em className="sop-node__doc-badge">📄 문서 보기</em>}
                </div>
              </button>
            );
          })}
        </div>

        {/* 단계별 산출물 실시간 프리뷰 패널 */}
        {selectedStepOutput && (
          <div className="sop-preview">
            <header className="sop-preview__head">
              <strong>📄 {selectedStepOutput.title}</strong>
              <div style={{ display: "flex", gap: 6 }}>
                <button className="btn btn--ghost btn--sm" onClick={() => copy(selectedStepOutput.markdown, "단계 산출물을 복사했습니다.")}>
                  <Icon name="copy" size={13} /> 복사
                </button>
                <button className="btn btn--ghost btn--sm" onClick={() => setSelectedStepKey(null)}>
                  닫기
                </button>
              </div>
            </header>
            <div className="sop-preview__body">
              <Markdown text={selectedStepOutput.markdown} />
            </div>
          </div>
        )}

        {/* 실시간 에이전트 터미널 & 팀 인계 피드 (진행 중이거나 최근 활동이 있을 때 표시) */}
        {(task.status === "running" || task.status === "queued" || task.events.length > 0) && (
          <div className="live-studio__grid">
            <div className="live-terminal">
              <div className="live-terminal__bar">
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <Avatar employee={activeActorEmp} size={22} />
                  <strong>{activeActorEmp?.name ?? "AI 직원"}</strong>
                  <small>{activeActorEmp?.role}</small>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                  {live?.tool && <span className="live-terminal__tool">[{live.tool.toUpperCase()}]</span>}
                  <span className="live-terminal__kind">
                    {task.status === "running"
                      ? live?.kind === "typing"
                        ? "✍️ 코드·문서 작성 중"
                        : live?.kind === "running"
                          ? "⚙️ 명령·검증 실행 중"
                          : live?.kind === "reading"
                            ? "🔍 파일·웹 분석 중"
                            : "🧠 설계·추론 중"
                      : task.status === "completed"
                        ? "✅ 작업 완료"
                        : "⏸ 대기 중"}
                  </span>
                </div>
              </div>
              <div className="live-terminal__status">
                {task.status === "running" && live ? live.label : task.currentStep ?? "모든 단계가 처리되었습니다."}
              </div>
              <pre className="live-terminal__screen">
                {live?.detail
                  ? live.detail
                  : task.status === "running"
                    ? `> OpenCode 에이전트(${activeActorEmp?.name ?? ""})가 '${task.currentStep ?? "현재 단계"}' 작업을 수행하고 있습니다...\n> 파일 작성·수정 또는 추론 결과가 나오는 즉시 이 화면에 실시간 스트리밍됩니다.`
                    : task.steps
                        .filter((s) => s.note)
                        .map((s) => `[${s.label}] ${s.note}`)
                        .join("\n") || "작업 완료 후 아래 결과물 탭에서 전체 산출물을 확인하거나, 추가 업무 지시를 내릴 수 있습니다."}
              </pre>
            </div>

            <div className="live-handoffs">
              <div className="live-handoffs__head">
                <strong>💬 팀 협업 인계 & 실시간 작업 로그</strong>
                <small>최근 {Math.min(task.events.length, 8)}건</small>
              </div>
              <ul className="live-handoffs__list">
                {task.events.slice(-8).reverse().map((ev, i) => {
                  const isHandoff = ev.message.includes("[업무 인계]") || ev.message.includes("[추가 업무 지시");
                  return (
                    <li key={i} className={`live-handoffs__item ${isHandoff ? "is-handoff" : `is-${ev.level}`}`}>
                      <time>{new Date(ev.at).toLocaleTimeString("ko-KR", { hour: "2-digit", minute: "2-digit", second: "2-digit" })}</time>
                      <span>{ev.message}</span>
                    </li>
                  );
                })}
              </ul>
            </div>
          </div>
        )}
      </section>

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

      {/* ───────── 세션 내 연속 추가 업무 지시 카드 (Item #1) ───────── */}
      {!["running", "queued"].includes(task.status) && (
        <section className="followup-box">
          <div className="followup-box__head">
            <div>
              <span className="eyebrow">연속 세션 · OpenCode 자동 담당자 배정</span>
              <h3>💬 이 작업 결과물에 바로 추가 업무 지시하기 ({followupRound + 1}차 작업)</h3>
              <p>
                새로 처음부터 만들지 않고, <strong>현재 만들어진 폴더와 파일 위에서 그대로 이어서</strong> 기능 추가·수정·개선을 수행합니다. 지시 내용을 분석해 OpenCode가 필요한 직원(기획·UI·로직·QA 등)만 자동으로 투입하고 제목을 갱신합니다.
              </p>
            </div>
          </div>

          {followupHistory.length > 0 && (
            <div className="followup-box__history">
              <strong>이전 연속 지시 내역:</strong>
              <ul>
                {followupHistory.map((h, i) => (
                  <li key={i}>
                    <span className="badge badge--neutral">{h.round ?? i + 2}차</span> {h.prompt}
                  </li>
                ))}
              </ul>
            </div>
          )}

          <div className="followup-box__chips">
            {followupSuggestions.map((sug) => (
              <button
                key={sug}
                type="button"
                className="chip"
                onClick={() => setFollowupPrompt(sug)}
              >
                + {sug}
              </button>
            ))}
          </div>

          <div className="followup-box__form">
            <textarea
              rows={2}
              value={followupPrompt}
              onChange={(ev) => setFollowupPrompt(ev.target.value)}
              onKeyDown={(ev) => {
                if (ev.key === "Enter" && (ev.ctrlKey || ev.metaKey)) {
                  ev.preventDefault();
                  handleFollowupSubmit();
                }
              }}
              placeholder="예: 다크모드 버튼을 헤더 오른쪽에 추가하고, 단축키 Ctrl+S로 저장되게 고쳐줘 (Ctrl+Enter로 바로 지시)"
            />
            <button
              type="button"
              className="btn btn--primary"
              disabled={submittingFollowup || !followupPrompt.trim()}
              onClick={handleFollowupSubmit}
            >
              <Icon name="spark" size={16} />
              {submittingFollowup ? "배정 중…" : "이어서 업무 시작"}
            </button>
          </div>
        </section>
      )}

      <div className="task-detail__actions">
        {entryHtml && (
          <button
            className="btn btn--primary btn--sm"
            onClick={() => act(() => api.launchProject(entryHtml), "만들어진 프로그램을 실행했습니다.")}
          >
            <Icon name="play" size={16} /> 프로그램 바로 실행
          </button>
        )}
        {outputFolder && (
          <button className="btn btn--ghost btn--sm" onClick={() => revealItemInDir(outputFolder)}>
            <Icon name="folder" size={16} /> 산출물 폴더 열기
          </button>
        )}
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
