import { useEffect, useMemo, useRef, type ReactNode, type MouseEvent } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Employee, EmployeeStatus, TaskStatus } from "../api";

import * as LucideIcons from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { fmtTokens, shortModelLabel } from "../api";

// ───────── 아이콘 (lucide-react 매핑, 호출부 호환 유지) ─────────
const iconMap: Record<string, LucideIcon> = {
  office: LucideIcons.Building2,
  home: LucideIcons.Home,
  people: LucideIcons.Users,
  work: LucideIcons.Briefcase,
  link: LucideIcons.Link2,
  help: LucideIcons.HelpCircle,
  plus: LucideIcons.Plus,
  chevron: LucideIcons.ChevronRight,
  close: LucideIcons.X,
  check: LucideIcons.Check,
  pause: LucideIcons.Pause,
  play: LucideIcons.Play,
  trash: LucideIcons.Trash2,
  copy: LucideIcons.Copy,
  download: LucideIcons.Download,
  print: LucideIcons.Printer,
  folder: LucideIcons.Folder,
  undo: LucideIcons.Undo2,
  search: LucideIcons.Search,
  key: LucideIcons.KeyRound,
  refresh: LucideIcons.RotateCw,
  external: LucideIcons.ExternalLink,
  doc: LucideIcons.FileText,
  spark: LucideIcons.Sparkles,
  gauge: LucideIcons.Gauge,
  coins: LucideIcons.Coins,
  cpu: LucideIcons.Cpu,
  shield: LucideIcons.ShieldAlert,
};

export function Icon({ name, size = 18 }: { name: string; size?: number }) {
  const Comp = iconMap[name] || LucideIcons.HelpCircle;
  return <Comp className="icon" size={size} strokeWidth={1.8} aria-hidden="true" />;
}

export function TokenChip({
  model,
  input,
  output,
  krw,
  free,
}: {
  model?: string | null;
  input?: number;
  output?: number;
  krw?: number;
  free?: boolean;
}) {
  const label = shortModelLabel(model);
  const parts: string[] = [label];
  if (input !== undefined && input > 0) parts.push(`입력 ${fmtTokens(input)}`);
  if (output !== undefined && output > 0) parts.push(`출력 ${fmtTokens(output)}`);
  if (free) {
    parts.push("실제 0원");
  } else if (krw !== undefined && krw > 0) {
    parts.push(`환산 ₩${krw.toLocaleString("ko-KR")}`);
  }
  return (
    <span className="token-chip" title={model || ""}>
      <Icon name="coins" size={13} />
      <span>{parts.join(" · ")}</span>
    </span>
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
