import { useEffect, useMemo, useState } from "react";
import {
  api,
  errorText,
  fmtTokens,
  fmtUsd,
  isDev,
  modeLabel,
  won,
  type Employee,
  type Estimate,
} from "../api";
import { useStore } from "../store";
import { Avatar, Icon, Modal, StatusBadge } from "./ui";
import { TaskForm, defaultValues, missingFields, type Values } from "./TaskForm";
import { QuoteSheet, previewQuote } from "./QuoteSheet";
import { OneLineAutoBox } from "./OneLineAutoBox";

function Picker({ onPick }: { onPick: (e: Employee) => void }) {
  const { boot } = useStore();
  const groups: [string, Employee[]][] = [
    ["개발팀 · 코드를 만드는 직원 (직접 지정)", (boot?.employees ?? []).filter((e) => isDev(e.id))],
    ["사무팀 · 글·조사·견적·정리 (직접 지정)", (boot?.employees ?? []).filter((e) => !isDev(e.id))],
  ];
  return (
    <>
    <OneLineAutoBox compact />
    {groups.map(([title, list]) => (
    <section key={title} className="picker-group">
    <h3>{title}</h3>
    <div className="picker">
      {list.map((e) => (
        <button key={e.id} className="picker__card" onClick={() => onPick(e)}>
          <Avatar employee={e} size={46} showStatus />
          <div>
            <strong>{e.name}</strong>
            <small>
              {e.role} · {modeLabel[e.mode]}
            </small>
            <p>{e.summary}</p>
            <StatusBadge employee={e.availability.status} />
          </div>
        </button>
      ))}
    </div>
    </section>
    ))}
    </>
  );
}

function Preflight({ employee, estimate }: { employee: Employee; estimate?: Estimate | null }) {
  const { navigate, closeNewTask } = useStore();
  const { status, missing, guard } = employee.availability;
  return (
    <section className="preflight">
      <h3>실행 전 점검</h3>
      <dl>
        <div>
          <dt>실행 방식</dt>
          <dd>{modeLabel[employee.mode]}</dd>
        </div>
        <div>
          <dt>업무 흐름</dt>
          <dd>{employee.steps.map((s) => s.label + (s.gate ? " (확인)" : "")).join(" → ")}</dd>
        </div>
        <div>
          <dt>비용</dt>
          <dd>{employee.costSummary}</dd>
        </div>
        {estimate && (
          <div>
            <dt>예상 사용량</dt>
            <dd>
              약 <strong style={{ fontFamily: "var(--font-mono)" }}>{fmtTokens(estimate.estTotal)}</strong> 토큰
              {estimate.free ? (
                <span className="model-chip model-chip--free" style={{ marginLeft: 6 }}>실제 0원 (무료 모델)</span>
              ) : (
                <> · 환산 <strong style={{ fontFamily: "var(--font-mono)" }}>{won(estimate.estKrw)}</strong> ({fmtUsd(estimate.estUsd)})</>
              )}
              <small style={{ display: "block", color: "var(--muted)", fontSize: 11 }}>
                산출 기준: {estimate.basis === "history" ? `동일 직원 과거 ${estimate.samples}건 평균` : "글자 수 기반 휴리스틱 추정"}
              </small>
            </dd>
          </div>
        )}
      </dl>
      {status === "setup_required" && (
        <div className="callout callout--warning">
          <strong>먼저 준비해 주세요: {missing.join(", ")}</strong>
          <button
            className="btn btn--sm"
            onClick={() => {
              closeNewTask();
              navigate("connections");
            }}
          >
            연결 및 설정으로 <Icon name="chevron" size={14} />
          </button>
        </div>
      )}
      {status === "guarded" && (
        <div className="callout callout--critical">
          <strong>같은 문제로 두 번 실패해 잠시 멈춰 두었습니다.</strong>
          <p>{guard}</p>
          <GuardAck employeeId={employee.id} />
        </div>
      )}
      {employee.notice && <p className="preflight__notice">{employee.notice}</p>}
    </section>
  );
}

export function GuardAck({ employeeId }: { employeeId: string }) {
  const { refresh, toast } = useStore();
  return (
    <button
      className="btn btn--sm"
      onClick={async () => {
        await api.acknowledgeGuard(employeeId);
        await refresh();
        toast("확인했습니다. 다시 업무를 맡길 수 있어요.", "success");
      }}
    >
      조치했어요
    </button>
  );
}

function TaskFormView({ employee, prefill, onBack }: { employee: Employee; prefill?: Values; onBack?: () => void }) {
  const { boot, closeNewTask, navigate, toast, refresh } = useStore();
  const [title, setTitle] = useState("");
  const [values, setValues] = useState<Values>(() => ({ ...defaultValues(employee.form), ...(prefill ?? {}) }));
  const [recent, setRecent] = useState<Values | null>(null);
  const [recentUsed, setRecentUsed] = useState(false);
  const [ack, setAck] = useState(false);
  const [busy, setBusy] = useState(false);
  const [estimate, setEstimate] = useState<Estimate | null>(null);
  const missing = useMemo(() => missingFields(employee.form, values), [employee, values]);
  const blocked = employee.availability.status === "setup_required" || employee.availability.status === "guarded";
  const isQuote = employee.id === "doyun";
  const handoff = employee.steps.some((s) => s.gate === "handoff");
  const coding = isDev(employee.id);

  useEffect(() => {
    if (!prefill) api.getRecentInput(employee.id).then(setRecent).catch(() => undefined);
  }, [employee.id, prefill]);

  useEffect(() => {
    const t = window.setTimeout(() => {
      api.estimateTask(employee.id, values).then(setEstimate).catch(() => setEstimate(null));
    }, 500);
    return () => window.clearTimeout(t);
  }, [employee.id, values]);

  const submit = async () => {
    setBusy(true);
    try {
      const task = await api.createTask(employee.id, title, values);
      closeNewTask();
      await refresh();
      navigate("work", task.id);
      toast(`${employee.name}에게 업무를 맡겼습니다.`, "success");
    } catch (e) {
      toast(errorText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const form = (
    <div className="task-form">
      {recent && (
        <button
          type="button"
          className="chip chip--recent"
          disabled={recentUsed}
          onClick={() => {
            setValues({ ...defaultValues(employee.form), ...recent });
            setRecentUsed(true);
          }}
        >
          <Icon name="refresh" size={14} /> {recentUsed ? "최근 설정을 불러왔습니다" : "최근 설정 불러오기"}
        </button>
      )}
      <div className="field">
        <label className="field__label">업무 이름</label>
        <input value={title} maxLength={120} placeholder="비워 두면 입력 내용으로 이름을 지어 드립니다" onChange={(e) => setTitle(e.target.value)} />
      </div>
      <TaskForm form={employee.form} values={values} onChange={setValues} />
      <Preflight employee={employee} estimate={estimate} />
      <label className="check check--ack">
        <input type="checkbox" checked={ack} onChange={(e) => setAck(e.target.checked)} />
        <span>
          {coding && employee.id !== "minhyuk"
            ? "팀은 별도 작업 공간(git worktree)에서 일하고, 제가 변경을 확인해 ‘병합하기’를 눌러야만 원래 저장소에 반영됩니다."
            : employee.id === "sua"
              ? "파일을 옮기기 전에 계획을 보여 주고, 제가 확인한 뒤에만 옮깁니다."
              : "입력한 내용으로 업무를 맡깁니다."}
          {handoff && " AI 단계는 안전 모드라 제가 OpenCode에서 직접 실행합니다(앱이 지시서를 준비합니다)."}
        </span>
      </label>
    </div>
  );

  return (
    <Modal
      wide={isQuote}
      title={
        <span className="modal-title-emp">
          <Avatar employee={employee} size={32} /> {employee.name}에게 업무 맡기기
        </span>
      }
      description={`${employee.role} · ${employee.inputSummary} → ${employee.outputSummary}`}
      onClose={closeNewTask}
      footer={
        <>
          {missing.length > 0 && <span className="foot-hint">필수 입력: {missing.join(", ")}</span>}
          {onBack && (
            <button className="btn btn--ghost" onClick={onBack}>
              다른 직원
            </button>
          )}
          <button className="btn btn--ghost" onClick={closeNewTask}>
            취소
          </button>
          <button className="btn btn--primary" disabled={!ack || missing.length > 0 || blocked || busy} onClick={submit}>
            {busy ? "맡기는 중…" : isQuote ? "견적서 만들기" : "업무 맡기기"}
          </button>
        </>
      }
    >
      {isQuote && boot ? (
        <div className="split">
          {form}
          <aside className="split__preview">
            <span className="eyebrow">실시간 미리보기</span>
            <QuoteSheet quote={previewQuote(values, boot.settings)} compact />
          </aside>
        </div>
      ) : (
        form
      )}
    </Modal>
  );
}

export function NewTaskDialog() {
  const { newTaskState, closeNewTask, employee } = useStore();
  const [picked, setPicked] = useState<string | undefined>(newTaskState.employeeId);
  useEffect(() => setPicked(newTaskState.employeeId), [newTaskState]);
  if (!newTaskState.open) return null;
  const emp = picked ? employee(picked) : undefined;
  if (!emp)
    return (
      <Modal title="누구에게 맡길까요?" description="직원을 고르면 그 직원의 업무 양식으로 이어집니다." onClose={closeNewTask} wide>
        <Picker onPick={(e) => setPicked(e.id)} />
      </Modal>
    );
  return <TaskFormView key={emp.id} employee={emp} prefill={newTaskState.prefill} onBack={newTaskState.employeeId ? undefined : () => setPicked(undefined)} />;
}
