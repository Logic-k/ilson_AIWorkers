import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "../api";
import { useStore } from "../store";
import { Avatar, Icon } from "./ui";

const PRESETS = [
  "메모 프로그램 만들고 싶어.",
  "뽀모도로 집중 타이머 프로그램 만들어줘",
  "소상공인 매출·지출 가계부 프로그램 만들고 싶어",
  "드래그 가능한 칸반 할일 관리 프로그램 만들어줘",
];

const SOP_AGENTS: { id: string; step: string; role: string }[] = [
  { id: "seojin", step: "1. 도메인·UX 리서치", role: "서진" },
  { id: "taeo", step: "2. 기획·설계(PRD)", role: "태오" },
  { id: "geonwoo", step: "3. 코어·로직 개발", role: "건우" },
  { id: "yuna", step: "4. 화면·UI 개발", role: "유나" },
  { id: "sohee", step: "5. QA·통합 검증", role: "소희" },
  { id: "minhyuk", step: "6. 코드 리뷰", role: "민혁" },
  { id: "daeun", step: "7. 착수 가이드·문서", role: "다은" },
];

function detectRoute(prompt: string, forceProgram: boolean): { empId: string; label: string } {
  if (forceProgram) return { empId: "taeo", label: "프로그램 전과정 자동 제작 (리서치 → 기획 → 개발 → QA → 착수)" };
  const lower = prompt.toLowerCase();
  const isProg = [
    "프로그램", "앱", "웹", "사이트", "게임", "계산기", "타이머", "메모", "투두", "todo",
    "가계부", "대시보드", "도구", "툴", "만들", "제작", "개발", "구현", "코딩", "자동화",
  ].some((k) => lower.includes(k));
  if (isProg || !prompt.trim()) {
    return { empId: "taeo", label: "프로그램 전과정 자동 제작 (리서치 → 기획 → 개발 → QA → 착수)" };
  }
  if (lower.includes("블로그") || lower.includes("포스팅")) {
    return { empId: "harin", label: "하린 · 네이버 블로그 원고 자동 작성" };
  }
  if (lower.includes("릴스") || lower.includes("쇼츠") || lower.includes("틱톡") || lower.includes("대본")) {
    return { empId: "jiho", label: "지호 · 숏폼 영상 대본 자동 작성" };
  }
  if (lower.includes("조사") || lower.includes("리서치") || lower.includes("트렌드") || lower.includes("비교")) {
    return { empId: "seojin", label: "서진 · 시장·경쟁사 자료 조사 브리프" };
  }
  return { empId: "taeo", label: "프로그램 전과정 자동 제작 (리서치 → 기획 → 개발 → QA → 착수)" };
}

export function OneLineAutoBox({
  compact = false,
  onStarted,
}: {
  compact?: boolean;
  onStarted?: () => void;
}) {
  const { employee, navigate, refresh, toast, closeNewTask } = useStore();
  const [prompt, setPrompt] = useState("");
  const [folder, setFolder] = useState("");
  const [forceProgram, setForceProgram] = useState(true);
  const [busy, setBusy] = useState(false);

  const route = detectRoute(prompt, forceProgram);
  const isProgramPipeline = route.empId === "taeo";

  const pickFolder = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string" && selected) {
      setFolder(selected);
    }
  };

  const submit = async () => {
    const trimmed = prompt.trim();
    if (!trimmed) {
      toast("만들고 싶은 프로그램이나 업무를 한 줄로 입력해 주세요.", "info");
      return;
    }
    setBusy(true);
    try {
      const task = await api.createAutoTask(trimmed, folder || undefined, forceProgram);
      setPrompt("");
      await refresh();
      closeNewTask();
      onStarted?.();
      navigate("work", task.id);
      toast(`‘${task.title}’ 자동 업무에 착수했습니다!`, "success");
    } catch (e) {
      toast(errorText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className={`auto-box ${compact ? "auto-box--compact" : ""}`}>
      <div className="auto-box__head">
        <div className="auto-box__title">
          <span className="auto-box__badge">⚡ 한줄 자동업무</span>
          <h3>한 줄만 적으면 리서치부터 기획·개발·착수까지 알아서 움직입니다</h3>
        </div>
        <label className="auto-box__mode-toggle" title="체크하면 어떤 문장이든 프로그램 제작 SOP 파이프라인으로 실행합니다">
          <input
            type="checkbox"
            checked={forceProgram}
            onChange={(e) => setForceProgram(e.target.checked)}
          />
          <span>프로그램 제작 SOP 고정</span>
        </label>
      </div>

      <div className="auto-box__input-row">
        <input
          className="auto-box__input"
          value={prompt}
          disabled={busy}
          placeholder="예: 메모 프로그램 만들고 싶어. / 뽀모도로 타이머 만들어줘"
          onChange={(e) => setPrompt(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.nativeEvent.isComposing) {
              e.preventDefault();
              submit();
            }
          }}
        />
        <button
          type="button"
          className="btn btn--primary auto-box__submit"
          disabled={busy || !prompt.trim()}
          onClick={submit}
        >
          <Icon name="spark" size={16} />
          {busy ? "착수 중…" : "바로 착수"}
        </button>
      </div>

      <div className="auto-box__presets">
        <span className="auto-box__presets-label">예시 클릭:</span>
        {PRESETS.map((p) => (
          <button
            key={p}
            type="button"
            className="chip auto-box__preset-chip"
            onClick={() => setPrompt(p)}
          >
            {p}
          </button>
        ))}
      </div>

      <div className="auto-box__pipeline">
        <div className="auto-box__pipeline-head">
          <span className="auto-box__route-label">
            자동 배정 파이프라인: <strong>{route.label}</strong>
          </span>
          <div className="auto-box__folder-ctrl">
            {folder ? (
              <>
                <span className="auto-box__folder-tag" title={folder}>
                  📁 {folder.split(/[\\/]/).pop() || folder}
                </span>
                <button type="button" className="link-btn" onClick={() => setFolder("")}>
                  해제
                </button>
              </>
            ) : (
              <span className="auto-box__folder-auto">
                📁 폴더 자동 생성 (내 문서/일손_프로젝트)
              </span>
            )}
            <button type="button" className="btn btn--ghost btn--sm" onClick={pickFolder}>
              <Icon name="folder" size={14} /> {folder ? "폴더 변경" : "폴더 직접 지정"}
            </button>
          </div>
        </div>

        {isProgramPipeline ? (
          <div className="auto-box__sop-steps">
            {SOP_AGENTS.map((item, idx) => {
              const emp = employee(item.id);
              return (
                <div key={item.id} className="auto-box__sop-step">
                  <Avatar employee={emp} size={22} />
                  <span>
                    <strong>{item.step}</strong>
                    <small>{item.role}</small>
                  </span>
                  {idx < SOP_AGENTS.length - 1 && <i className="auto-box__sop-arrow">→</i>}
                </div>
              );
            })}
          </div>
        ) : (
          <div className="auto-box__sop-steps">
            <div className="auto-box__sop-step">
              <Avatar employee={employee(route.empId)} size={24} />
              <span>
                <strong>{employee(route.empId)?.name}</strong>
                <small>{employee(route.empId)?.steps.map((s) => s.label).join(" → ")}</small>
              </span>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
