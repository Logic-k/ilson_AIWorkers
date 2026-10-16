import { useEffect, useMemo, useRef, type ReactNode, type MouseEvent } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Employee, EmployeeStatus, TaskStatus } from "../api";

// ───────── 아이콘 (선 아이콘, currentColor) ─────────
const paths: Record<string, string> = {
  office: "M4 21V5a1 1 0 0 1 1-1h9a1 1 0 0 1 1 1v16M15 9h4a1 1 0 0 1 1 1v11M8 8h3M8 12h3M8 16h3M3 21h18",
  home: "M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z",
  people: "M16 19v-1a4 4 0 0 0-4-4H7a4 4 0 0 0-4 4v1M9.5 10a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7M21 19v-1a4 4 0 0 0-3-3.87M15.5 3.13a3.5 3.5 0 0 1 0 6.75",
  work: "M9 4h6a1 1 0 0 1 1 1v2H8V5a1 1 0 0 1 1-1M4 7h16a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1M3 12h18",
  link: "M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1",
  help: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17h.01",
  plus: "M12 5v14M5 12h14",
  chevron: "m9 5 7 7-7 7",
  close: "M6 6l12 12M18 6 6 18",
  check: "m5 12 5 5 9-10",
  pause: "M8 5v14M16 5v14",
  play: "M7 5l12 7-12 7z",
  trash: "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
  copy: "M8 8h11v12H8zM5 16V4h11",
  download: "M12 4v11m0 0-4-4m4 4 4-4M5 20h14",
  print: "M7 9V3h10v6M7 17H5a1 1 0 0 1-1-1v-5a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v5a1 1 0 0 1-1 1h-2M7 14h10v7H7z",
  folder: "M3 6a1 1 0 0 1 1-1h5l2 2h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z",
  undo: "M9 14 4 9l5-5M4 9h11a5 5 0 0 1 0 10h-3",
  search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14M21 21l-4.3-4.3",
  key: "M15 7a4 4 0 1 1-3.87 5H9l-2 2-2-2 2-2h4.13A4 4 0 0 1 15 7M16 9h.01",
  refresh: "M20 11a8 8 0 0 0-14.9-4M4 4v4h4M4 13a8 8 0 0 0 14.9 4M20 20v-4h-4",
  external: "M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5",
  doc: "M7 3h7l5 5v12a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1M14 3v5h5M9 13h6M9 17h6",
  spark: "M12 3v4M12 17v4M3 12h4M17 12h4M6 6l2.5 2.5M15.5 15.5 18 18M6 18l2.5-2.5M15.5 8.5 18 6",
};

export function Icon({ name, size = 18 }: { name: keyof typeof paths | string; size?: number }) {
  return (
    <svg className="icon" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={paths[name] ?? ""} />
    </svg>
  );
}

// ───────── 아바타: 사진 대신 직원 색과 이름 첫 글자 ─────────
export function Avatar({ employee, size = 40, showStatus = false }: { employee?: Employee; size?: number; showStatus?: boolean }) {
  if (!employee) return <span className="avatar" style={{ width: size, height: size }} />;
  const h = employee.hue;
  return (
    <span
      className="avatar"
      style={{
        width: size,
        height: size,
        fontSize: size * 0.42,
        background: `linear-gradient(145deg, hsl(${h} 62% 72%), hsl(${(h + 28) % 360} 52% 50%))`,
      }}
      aria-label={`${employee.name} 프로필`}
    >
      {employee.name.slice(0, 1)}
      {showStatus && <i className={`avatar__dot avatar__dot--${employee.availability.status}`} />}
    </span>
  );
}

// ───────── 상태 배지 ─────────
const taskLabels: Record<TaskStatus, [string, string]> = {
  queued: ["대기", "neutral"],
  awaiting_approval: ["확인 필요", "warning"],
  running: ["실행 중", "info"],
  paused: ["멈춤", "neutral"],
  completed: ["완료", "positive"],
  failed: ["실패", "critical"],
  cancelled: ["취소됨", "neutral"],
};
const employeeLabels: Record<EmployeeStatus, [string, string]> = {
  ready: ["바로 사용", "positive"],
  setup_required: ["설정 필요", "warning"],
  busy: ["업무 중", "info"],
  guarded: ["점검 필요", "critical"],
};

export function StatusBadge({ task, employee }: { task?: TaskStatus; employee?: EmployeeStatus }) {
  const [label, tone] = task ? taskLabels[task] : employee ? employeeLabels[employee] : ["", "neutral"];
  return (
    <span className={`badge badge--${tone}`}>
      <i />
      {label}
    </span>
  );
}

// ───────── 모달 ─────────
export function Modal({
  title,
  description,
  onClose,
  children,
  footer,
  wide,
}: {
  title: ReactNode;
  description?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
}) {
  const panel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    panel.current?.focus();
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`modal ${wide ? "modal--wide" : ""}`} role="dialog" aria-modal="true" tabIndex={-1} ref={panel}>
        <header className="modal__head">
          <div>
            <h2>{title}</h2>
            {description && <p>{description}</p>}
          </div>
          <button className="icon-btn" onClick={onClose} aria-label="닫기">
            <Icon name="close" />
          </button>
        </header>
        <div className="modal__body">{children}</div>
        {footer && <footer className="modal__foot">{footer}</footer>}
      </div>
    </div>
  );
}

// ───────── 마크다운 (AI 결과는 반드시 정화해서 그린다) ─────────
export function markdownToHtml(md: string): string {
  return DOMPurify.sanitize(marked.parse(md, { async: false, gfm: true, breaks: false }) as string);
}

export function Markdown({ text }: { text: string }) {
  const html = useMemo(() => markdownToHtml(text), [text]);
  const onClick = (e: MouseEvent<HTMLDivElement>) => {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute("href") ?? "";
    if (/^https?:\/\//.test(href)) openUrl(href);
  };
  return <div className="markdown" onClick={onClick} dangerouslySetInnerHTML={{ __html: html }} />;
}

export function Empty({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="empty">
      <strong>{title}</strong>
      {children && <p>{children}</p>}
    </div>
  );
}
