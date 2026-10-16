import { invoke } from "@tauri-apps/api/core";

export type TaskStatus =
  | "queued"
  | "awaiting_approval"
  | "running"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export type EmployeeStatus = "ready" | "setup_required" | "busy" | "guarded";
export type Mode = "ai" | "local" | "ai_local";

export interface Choice {
  value: string;
  label: string;
  hint?: string;
  example?: string[];
}

export interface Field {
  id: string;
  kind: "text" | "textarea" | "number" | "date" | "select" | "choice" | "checkbox" | "folder" | "items";
  label: string;
  required?: boolean;
  placeholder?: string;
  hint?: string;
  default?: unknown;
  defaultToday?: boolean;
  choices?: Choice[];
  variant?: "chips" | "cards";
  min?: number;
  max?: number;
  maxLength?: number;
  visibleWhen?: { field: string; oneOf: string[] };
}

export interface Section {
  title: string;
  description?: string;
  advanced?: boolean;
  fields: Field[];
}

export interface Resume {
  employeeNo: string;
  experience: string;
  hometown: string;
  formerRole: string;
  intro: string;
  career: { period: string; org: string; note: string }[];
  skills: { label: string; score: number }[];
  reference: { quote: string; from: string };
  interviewNote: string;
}

export interface Employee {
  id: string;
  name: string;
  role: string;
  team: string;
  mode: Mode;
  hue: number;
  summary: string;
  motto: string;
  capabilities: string[];
  inputSummary: string;
  outputSummary: string;
  costSummary: string;
  notice?: string;
  resume: Resume;
  steps: { key: string; label: string; gate: string | null; actor: string }[];
  form: Section[];
  availability: { status: EmployeeStatus; missing: string[]; guard?: string };
  /** AI를 쓰는 직원의 담당 구독 엔진 */
  engine?: { id: EngineId; label: string; auto: boolean };
}

export type EngineId = "agy" | "claude" | "codex" | "kiro";

export interface Gate {
  kind: "apply" | "merge" | "handoff";
  key?: string;
  folder?: string | null;
  agent?: string | null;
  command?: string | null;
  prompt?: string | null;
  engine?: EngineId | null;
  title: string;
  summary: string;
  details: string[];
  estimateKrw?: number | null;
  maxKrw?: number | null;
}

export interface StepRow {
  idx: number;
  key: string;
  label: string;
  status: "pending" | "running" | "done" | "failed";
  note?: string | null;
  startedAt?: string | null;
  finishedAt?: string | null;
}

export interface EventRow {
  at: string;
  level: "info" | "warn" | "error" | "success";
  message: string;
}

export interface Task {
  id: string;
  employeeId: string;
  title: string;
  status: TaskStatus;
  input: Record<string, unknown>;
  progress: number;
  currentStep?: string | null;
  currentActor?: string | null;
  pendingGate?: Gate | null;
  approvedGates: string[];
  costUsd: number;
  result?: Deliverable | null;
  error?: string | null;
  createdAt: string;
  updatedAt: string;
  steps: StepRow[];
  events: EventRow[];
}

export interface Source {
  title: string;
  url: string;
}

export interface QuoteItem {
  name: string;
  spec: string;
  qty: number;
  unitPrice: number;
  amount: number;
}

export interface Quote {
  number: string;
  date: string;
  validUntil: string;
  validDays: number;
  delivery: string;
  projectName: string;
  client: { name: string; contact: string };
  supplier: { name: string; owner: string; bizRegNo: string; address: string; phone: string; email: string };
  items: QuoteItem[];
  vatMode: "separate" | "included" | "exempt";
  supply: number;
  vat: number;
  total: number;
  totalKorean: string;
  notes: string[];
  message: string;
}

export type Deliverable =
  | {
      kind: "document";
      title: string;
      markdown: string;
      meta: { label: string; value: string }[];
      checks?: { label: string; value: string; ok: boolean }[] | null;
      sources: Source[];
      truncated?: boolean;
    }
  | { kind: "quote"; title: string; quote: Quote }
  | {
      kind: "code";
      title: string;
      markdown: string;
      sections: string[];
      merged?: boolean | null;
      mergeCommit?: string | null;
      files?: { path: string; added: number; removed: number }[] | null;
      added?: number | null;
      removed?: number | null;
      commits?: string[] | null;
      verdict?: string | null;
      workspace?: { repo: string; branch: string; baseBranch: string; path: string } | null;
    }
  | {
      kind: "organize";
      title: string;
      markdown: string;
      folder: string;
      moves: { from: string; to: string; label: string; status: string; reason?: string }[];
      undone: boolean;
    };

export interface Settings {
  model: string;
  engineMode: "safe" | "auto";
  autoAckAt?: string | null;
  autoAcks: Record<string, string>;
  assignments: Record<string, EngineId>;
  models: Record<string, string>;
  yolo: boolean;
  businessName: string;
  ownerName: string;
  bizRegNo: string;
  industry: string;
  region: string;
  phone: string;
  email: string;
  address: string;
  brandTone: string;
  brandKeywords: string;
  bannedWords: string;
  sampleText: string;
}

export interface ModelInfo {
  id: string;
  label: string;
}

export interface EngineStatus {
  id: EngineId;
  label: string;
  plan: string;
  canAuto: boolean;
  login?: string | null;
  installed: boolean;
  path?: string | null;
  version?: string | null;
  signedIn: boolean;
  models: ModelInfo[];
  message?: string | null;
}

export interface Activity {
  taskId: string;
  employeeId: string;
  kind: "typing" | "reading" | "running" | "thinking" | "talking";
  label: string;
  engine?: EngineId;
  at: string;
}

export interface DiffSummary {
  files: { path: string; added: number; removed: number }[];
  added: number;
  removed: number;
  patch: string;
  truncated: boolean;
}

export interface Bootstrap {
  employees: Employee[];
  settings: Settings;
  engines: EngineStatus[];
  /** 엔진별 권한 단계 → 실제 CLI 옵션 (YOLO 키워드 표) */
  levels: { engine: EngineId; readOnly: string; edit: string; sandbox: string; yolo: string }[];
  dataDir: string;
  version: string;
}

export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  listTasks: () => invoke<Task[]>("list_tasks"),
  getTask: (id: string) => invoke<Task>("get_task", { id }),
  createTask: (employeeId: string, title: string, input: Record<string, unknown>) =>
    invoke<Task>("create_task", { employeeId, title, input }),
  approveTask: (id: string) => invoke<void>("approve_task", { id }),
  declineTask: (id: string) => invoke<void>("decline_task", { id }),
  pauseTask: (id: string) => invoke<void>("pause_task", { id }),
  resumeTask: (id: string) => invoke<void>("resume_task", { id }),
  deleteTask: (id: string) => invoke<void>("delete_task", { id }),
  undoOrganize: (id: string) => invoke<string>("undo_organize", { id }),
  getRecentInput: (employeeId: string) => invoke<Record<string, unknown> | null>("get_recent_input", { employeeId }),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  engineStatus: () => invoke<EngineStatus[]>("engine_status"),
  openLoginTerminal: (engine: EngineId) => invoke<void>("open_login_terminal", { engine }),
  openTerminal: (folder: string) => invoke<void>("open_terminal", { folder }),
  activities: () => invoke<Activity[]>("activities"),
  taskDiff: (id: string) => invoke<DiffSummary>("task_diff", { id }),
  discardTask: (id: string) => invoke<void>("discard_task", { id }),
  taskWorkspacePath: (id: string) => invoke<string>("task_workspace_path", { id }),
  openInAntigravity: (folder: string) => invoke<void>("open_in_antigravity", { folder }),
  finishExternal: (id: string) => invoke<void>("finish_external", { id }),
  acknowledgeGuard: (employeeId: string) => invoke<void>("acknowledge_guard", { employeeId }),
  saveTextFile: (path: string, contents: string) => invoke<void>("save_text_file", { path, contents }),
};

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

export const DEV_TEAM = ["taeo", "yuna", "geonwoo", "sohee", "minhyuk", "daeun"];
export const isDev = (id: string) => DEV_TEAM.includes(id);

export const won = (n: number) => `${Math.round(n).toLocaleString("ko-KR")}원`;

export function relativeTime(iso: string): string {
  const diff = (Date.now() - new Date(iso).getTime()) / 1000;
  if (diff < 45) return "방금";
  if (diff < 3600) return `${Math.floor(diff / 60)}분 전`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}시간 전`;
  if (diff < 172800) return "어제";
  if (diff < 604800) return `${Math.floor(diff / 86400)}일 전`;
  return new Date(iso).toLocaleDateString("ko-KR");
}

export const modeLabel: Record<Mode, string> = {
  ai: "AI (구독)",
  ai_local: "AI + 이 PC",
  local: "이 PC",
};

export const engineShort: Record<EngineId, string> = { agy: "AG", claude: "CC", codex: "CX", kiro: "KR" };
export const engineHue: Record<EngineId, number> = { agy: 210, claude: 22, codex: 160, kiro: 275 };
