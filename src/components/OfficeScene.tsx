import { useEffect, useMemo, useRef, useState } from "react";
import { engineHue, engineShort, isDev, type Activity, type Employee, type Task } from "../api";

// ───────── 건물 단면도 좌표 (논리 해상도 1200×720) ─────────
const W = 1200;
const H = 720;
type Floor = 1 | 2 | 3;
const FEET: Record<Floor, number> = { 3: 236, 2: 466, 1: 696 };
const FLOOR_TOP: Record<Floor, number> = { 3: 24, 2: 254, 1: 484 };
/** 계단은 지그재그: 1층→2층은 왼쪽 아래에서 오른쪽 위로, 2층→3층은 다시 왼쪽 위로 */
const STAIR_X: Record<Floor, number> = { 1: 1098, 2: 1178, 3: 1098 };

const DEV_DESKS: Record<string, number> = { taeo: 96, yuna: 246, geonwoo: 396, sohee: 546, minhyuk: 696, daeun: 846 };
const BIZ_DESKS: Record<string, number> = { harin: 86, jiho: 218, seojin: 350, minjae: 482, doyun: 614, sua: 746, yena: 878 };
const SPOTS = {
  coffee: { floor: 1 as Floor, x: 300 },
  sofa: { floor: 1 as Floor, x: 130 },
  meeting: { floor: 1 as Floor, x: 520 },
  boss: { floor: 1 as Floor, x: 880 },
  tray: { floor: 1 as Floor, x: 975 },
  door: { floor: 1 as Floor, x: 1050 },
  whiteboard: { floor: 3 as Floor, x: 985 },
  printer: { floor: 2 as Floor, x: 1010 },
};

interface Spot {
  floor: Floor;
  x: number;
}

export function deskOf(id: string): Spot {
  return isDev(id) ? { floor: 3, x: DEV_DESKS[id] ?? 100 } : { floor: 2, x: BIZ_DESKS[id] ?? 100 };
}

type Pose = "idle" | "typing" | "reading" | "running" | "waiting" | "confused" | "meeting" | "coffee" | "handing" | "cheer" | "paused";

interface View {
  pose: Pose;
  chip?: { text: string; tone: "work" | "wait" | "bad" | "done" | "idle" };
  bubble?: string;
  carrying?: boolean;
}

interface Sim {
  x: number;
  y: number;
  path: { x: number; y: number; slow?: boolean }[];
  goalKey: string;
  facing: 1 | -1;
  walking: boolean;
  errand?: { to: string; label: string; phase: "go" | "hand"; until: number };
  wanderUntil: number;
  wander?: Spot & { key: string };
  cheerUntil: number;
}

const speed = { walk: 120, stair: 85 };

/** 화면을 오가도 직원 위치·동작을 이어 가도록 장면 밖에 둔다. */
const SIMS: Record<string, Sim> = {};
const PREV_ACTOR: Record<string, string | null | undefined> = {};
const PREV_STATUS: Record<string, string> = {};

function floorOfY(y: number): Floor {
  return y < 350 ? 3 : y < 580 ? 2 : 1;
}

/** 같은 층이면 바로, 다른 층이면 계단 지그재그를 거쳐 간다. */
function route(from: { x: number; y: number }, to: Spot): Sim["path"] {
  const f1 = floorOfY(from.y);
  const pts: Sim["path"] = [];
  if (f1 !== to.floor) {
    pts.push({ x: STAIR_X[f1], y: FEET[f1] });
    const dir = to.floor > f1 ? 1 : -1;
    for (let f = f1 + dir; dir > 0 ? f <= to.floor : f >= to.floor; f += dir) {
      pts.push({ x: STAIR_X[f as Floor], y: FEET[f as Floor], slow: true });
    }
  }
  pts.push({ x: to.x, y: FEET[to.floor] });
  return pts;
}

const short = (s: string, n = 16) => (s.length > n ? s.slice(0, n - 1) + "…" : s);

function deskProp(id: string) {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0;
  return h % 5;
}

interface Props {
  employees: Employee[];
  tasks: Task[];
  activities: Record<string, Activity>;
  selected?: string | null;
  onSelect: (id: string) => void;
  onBoss: () => void;
  onTaskClick?: (taskId: string) => void;
}

export function OfficeScene({ employees, tasks, activities, selected, onSelect, onBoss, onTaskClick }: Props) {
  const sims = useRef<Record<string, Sim>>(SIMS);
  const refs = useRef<Record<string, SVGGElement | null>>({});
  const prevActor = useRef(PREV_ACTOR);
  const prevStatus = useRef(PREV_STATUS);
  const [views, setViews] = useState<Record<string, View>>({});
  const [hour, setHour] = useState(new Date().getHours());
  const inputs = useRef({ employees, tasks, activities });
  inputs.current = { employees, tasks, activities };

  const names = useMemo(() => Object.fromEntries(employees.map((e) => [e.id, e.name])), [employees]);

  // 처음엔 모두 문에서 출근해 자리로 간다.
  useEffect(() => {
    employees.forEach((e, i) => {
      if (sims.current[e.id]) return;
      sims.current[e.id] = {
        x: SPOTS.door.x + (i % 3) * 6,
        y: FEET[1],
        path: [],
        goalKey: "",
        facing: -1,
        walking: false,
        wanderUntil: Date.now() + 4000 + i * 900,
        cheerUntil: 0,
      };
    });
  }, [employees]);

  // 핸드오프·완료 감지: 업무의 담당 직원이 바뀌면 이전 담당이 서류를 들고 간다.
  useEffect(() => {
    const now = Date.now();
    for (const t of tasks) {
      const before = prevActor.current[t.id];
      const after = t.currentActor ?? null;
      if (before && after && before !== after && sims.current[before] && t.status === "running") {
        sims.current[before].errand = { to: after, label: `${names[after] ?? ""} 님, ${t.currentStep ?? "다음 단계"} 부탁해요`, phase: "go", until: 0 };
      }
      prevActor.current[t.id] = after;
      if (prevStatus.current[t.id] && prevStatus.current[t.id] !== "completed" && t.status === "completed" && sims.current[t.employeeId]) {
        sims.current[t.employeeId].cheerUntil = now + 7000;
      }
      prevStatus.current[t.id] = t.status;
    }
  }, [tasks, names]);

  // 생각하기: 0.5초마다 각 직원이 어디서 무엇을 할지 정한다.
  useEffect(() => {
    const think = () => {
      const { employees, tasks, activities } = inputs.current;
      const now = Date.now();
      const next: Record<string, View> = {};
      const approvals = tasks.filter((t) => t.status === "awaiting_approval").sort((a, b) => a.updatedAt.localeCompare(b.updatedAt));
      const meeting = new Set<string>();
      for (const t of tasks) {
        if (t.status === "running" && t.employeeId === "taeo" && t.currentActor === "taeo" && t.currentStep === "기획·설계") {
          meeting.add("taeo");
          const devs = String(t.input.devs ?? "both");
          if (devs !== "backend") meeting.add("yuna");
          if (devs !== "frontend") meeting.add("geonwoo");
        }
      }
      const runningIds = new Set(tasks.filter((t) => t.status === "running").map((t) => t.id));
      let meetIdx = 0;
      employees.forEach((e, i) => {
        const s = sims.current[e.id];
        if (!s) return;
        let goal: Spot = deskOf(e.id);
        let key = `desk`;
        let view: View = { pose: "idle" };
        const mine = tasks.filter((t) => t.employeeId === e.id || t.currentActor === e.id);
        const qIdx = approvals.findIndex((t) => t.employeeId === e.id);
        const act0 = activities[e.id];
        const liveAct = act0 && runningIds.has(act0.taskId) && now - new Date(act0.at).getTime() < 120_000 ? act0 : undefined;
        const running = tasks.find((t) => t.status === "running" && t.currentActor === e.id) ?? (liveAct ? tasks.find((t) => t.id === liveAct.taskId) : undefined);
        const failed = mine.find((t) => t.status === "failed" && now - new Date(t.updatedAt).getTime() < 20 * 60_000);
        const paused = mine.find((t) => t.status === "paused" && (t.currentActor ?? t.employeeId) === e.id);
        const queued = tasks.find((t) => t.status === "queued" && t.employeeId === e.id);
        const act = activities[e.id];
        if (s.errand) {
          const target = deskOf(s.errand.to);
          goal = { floor: target.floor, x: target.x + (target.x > 600 ? -34 : 34) };
          key = `errand:${s.errand.to}`;
          view = { pose: s.errand.phase === "hand" ? "handing" : "idle", carrying: true, bubble: s.errand.label, chip: { text: "📁 전달", tone: "work" } };
        } else if (qIdx >= 0) {
          goal = { floor: 1, x: SPOTS.boss.x - 46 - qIdx * 40 };
          key = `boss:${qIdx}`;
          const g = approvals[qIdx].pendingGate;
          view = { pose: "waiting", chip: { text: "!", tone: "wait" }, bubble: g?.kind === "merge" ? "병합 확인 부탁드려요" : g?.kind === "handoff" ? "Antigravity에서 불러 주세요" : "확인 부탁드려요" };
        } else if (meeting.has(e.id)) {
          goal = { floor: 1, x: SPOTS.meeting.x - 60 + meetIdx++ * 60 };
          key = `meeting:${meetIdx}`;
          view = { pose: "meeting", chip: { text: "회의", tone: "work" }, bubble: e.id === "taeo" ? "구조부터 정할게요" : undefined };
        } else if (running && act && act.taskId === running.id && act.label.startsWith("Antigravity에서 시작")) {
          view = { pose: "idle", chip: { text: "▶ 대기", tone: "idle" }, bubble: "Antigravity에서 불러 주세요" };
        } else if (running) {
          const kind = act && act.taskId === running.id ? act.kind : "thinking";
          const pose: Pose = kind === "typing" ? "typing" : kind === "running" ? "running" : kind === "reading" ? "reading" : "typing";
          view = { pose, chip: { text: "작업 중", tone: "work" }, bubble: short(act && act.taskId === running.id ? act.label : running.currentStep ?? "작업 중", 18) };
        } else if (failed) {
          view = { pose: "confused", chip: { text: "⚠", tone: "bad" }, bubble: "문제가 생겼어요" };
        } else if (now < s.cheerUntil) {
          if (now < s.cheerUntil - 3500) {
            goal = { floor: 1, x: SPOTS.tray.x - 8 - (i % 3) * 10 };
            key = "tray";
            view = { pose: "idle", carrying: true, chip: { text: "✓ 완료", tone: "done" }, bubble: "결과 올려 둘게요" };
          } else {
            view = { pose: "cheer", chip: { text: "✓ 완료", tone: "done" } };
          }
        } else if (paused) {
          view = { pose: "paused", chip: { text: "⏸", tone: "idle" }, bubble: "잠시 멈춤" };
        } else if (queued) {
          view = { pose: "idle", chip: { text: "대기", tone: "idle" }, bubble: "곧 시작할게요" };
        } else {
          // 한가할 때: 가끔 커피·라운지·화이트보드에 다녀온다.
          if (now > s.wanderUntil) {
            const r = Math.random();
            if (s.wander) {
              s.wander = undefined;
              s.wanderUntil = now + 15000 + Math.random() * 30000;
            } else if (r < 0.5) {
              const opts = [
                { ...SPOTS.coffee, key: "coffee" },
                { ...SPOTS.sofa, key: "sofa" },
                isDev(e.id) ? { ...SPOTS.whiteboard, key: "whiteboard" } : { ...SPOTS.printer, key: "printer" },
              ];
              const pick = opts[Math.floor(Math.random() * opts.length)];
              s.wander = { ...pick, x: pick.x + (i % 4) * 14 - 20 };
              s.wanderUntil = now + 9000 + Math.random() * 9000;
            } else {
              s.wanderUntil = now + 10000 + Math.random() * 25000;
            }
          }
          if (s.wander) {
            goal = s.wander;
            key = `wander:${s.wander.key}`;
            view = { pose: s.wander.key === "coffee" ? "coffee" : "idle", bubble: s.wander.key === "coffee" && !s.walking ? "☕" : undefined };
          }
        }
        if (key !== s.goalKey) {
          s.goalKey = key;
          s.path = route(s, goal);
        }
        if (s.errand && s.errand.phase === "go" && !s.path.length) {
          s.errand.phase = "hand";
          s.errand.until = now + 1800;
        } else if (s.errand && s.errand.phase === "hand" && now > s.errand.until) {
          s.errand = undefined;
        }
        next[e.id] = view;
      });
      setViews(next);
      setHour(new Date().getHours());
    };
    think();
    const id = window.setInterval(think, 500);
    return () => window.clearInterval(id);
  }, []);

  // 걷기: 매 프레임 위치만 바꾼다 (React 렌더 없이 transform 직접 갱신).
  useEffect(() => {
    let raf = 0;
    let last = performance.now();
    const step = (t: number) => {
      const dt = Math.min(0.05, (t - last) / 1000);
      last = t;
      for (const [id, s] of Object.entries(sims.current)) {
        const wp = s.path[0];
        if (wp) {
          const dx = wp.x - s.x;
          const dy = wp.y - s.y;
          const dist = Math.hypot(dx, dy);
          const v = (wp.slow ? speed.stair : speed.walk) * dt;
          if (dist <= v) {
            s.x = wp.x;
            s.y = wp.y;
            s.path.shift();
          } else {
            s.x += (dx / dist) * v;
            s.y += (dy / dist) * v;
          }
          if (Math.abs(dx) > 0.5) s.facing = dx > 0 ? 1 : -1;
        }
        const walking = s.path.length > 0;
        const el = refs.current[id];
        if (el) {
          el.setAttribute("transform", `translate(${s.x.toFixed(1)},${s.y.toFixed(1)})`);
          el.classList.toggle("is-walking", walking);
          const body = el.firstElementChild as SVGGElement | null;
          body?.setAttribute("transform", `scale(${s.facing},1)`);
        }
        s.walking = walking;
      }
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, []);

  const night = hour < 6 || hour >= 19;
  const dusk = hour >= 17 && hour < 19;
  const sky = night ? "#1d2747" : dusk ? "#f5b98a" : "#bfe1f7";
  const pending = tasks.filter((t) => t.status === "awaiting_approval").length;
  const deskState = (id: string) => {
    const v = views[id];
    if (!v) return "";
    const s = sims.current[id];
    const atDesk = s && !s.walking && s.goalKey === "desk";
    if (!atDesk) return "";
    return v.pose === "running" ? "is-run" : v.pose === "confused" ? "is-bad" : ["typing", "reading"].includes(v.pose) ? "is-on" : "";
  };

  return (
    <svg className={`office ${night ? "is-night" : ""}`} viewBox={`0 0 ${W} ${H}`} role="img" aria-label="AI 직원 사무실">
      <defs>
        <linearGradient id="wall3" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#e4f2f1" />
          <stop offset="1" stopColor="#d5e9e7" />
        </linearGradient>
        <linearGradient id="wall2" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#f7efe3" />
          <stop offset="1" stopColor="#efe3d1" />
        </linearGradient>
        <linearGradient id="wall1" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#eceff5" />
          <stop offset="1" stopColor="#dfe4ee" />
        </linearGradient>
      </defs>

      {/* 건물 외곽과 층 */}
      <rect x="0" y="0" width={W} height={H} rx="18" fill="#2a2f38" />
      {([3, 2, 1] as Floor[]).map((f) => (
        <g key={f}>
          <rect x="14" y={FLOOR_TOP[f]} width="1050" height="212" fill={`url(#wall${f})`} />
          {[0, 1, 2, 3].map((k) => (
            <rect key={k} x={150 + k * 250} y={FLOOR_TOP[f] + 22} width="120" height="62" rx="6" fill={sky} className="window" />
          ))}
          <rect x="14" y={FEET[f]} width="1050" height="10" fill="#b9a68c" />
          <rect x="14" y={FEET[f] + 10} width="1050" height="8" fill="#3a3f49" />
        </g>
      ))}
      {night && [0, 1, 2, 3].map((k) => <circle key={k} cx={180 + k * 250} cy={FLOOR_TOP[3] + 40} r="2" fill="#fff" opacity=".8" />)}

      {/* 층 이름 */}
      <text x="28" y={FLOOR_TOP[3] + 20} className="office__floor">3F 개발실</text>
      <text x="28" y={FLOOR_TOP[2] + 20} className="office__floor">2F 사무팀 · 콘텐츠 · 리서치 · 영업 · 경영지원</text>
      <text x="28" y={FLOOR_TOP[1] + 20} className="office__floor">1F 로비</text>

      {/* 계단 */}
      <rect x="1064" y="24" width="122" height="672" fill="#cfd5df" />
      {([1, 2] as Floor[]).map((f) => {
        const top = (f + 1) as Floor;
        return <line key={f} x1={STAIR_X[f]} y1={FEET[f]} x2={STAIR_X[top]} y2={FEET[top]} stroke="#8b94a3" strokeWidth="8" strokeLinecap="round" />;
      })}
      <text x="1125" y="44" textAnchor="middle" className="office__label">계단</text>

      {/* 3F: 화이트보드 */}
      <rect x="940" y={FLOOR_TOP[3] + 96} width="92" height="62" rx="4" fill="#fff" stroke="#9aa5b4" />
      <path d={`M952 ${FLOOR_TOP[3] + 116} h40 M952 ${FLOOR_TOP[3] + 128} h60 M952 ${FLOOR_TOP[3] + 140} h30`} stroke="#4e79c2" strokeWidth="3" strokeLinecap="round" />
      {/* 2F: 프린터 */}
      <rect x="990" y={FEET[2] - 44} width="44" height="44" rx="5" fill="#d9dde4" stroke="#9aa5b4" />
      <rect x="998" y={FEET[2] - 52} width="28" height="10" rx="2" fill="#fff" stroke="#9aa5b4" />

      {/* 1F: 라운지·회의실·사장석 */}
      <rect x="70" y={FEET[1] - 34} width="150" height="34" rx="12" fill="#c4734a" />
      <rect x="64" y={FEET[1] - 56} width="162" height="26" rx="10" fill="#d98a5f" />
      <rect x="282" y={FEET[1] - 66} width="38" height="66" rx="5" fill="#59606c" />
      <circle cx="301" cy={FEET[1] - 46} r="6" fill="#e9a03b" />
      <text x="150" y={FLOOR_TOP[1] + 118} textAnchor="middle" className="office__label">라운지</text>
      <rect x="380" y={FLOOR_TOP[1] + 30} width="290" height="182" fill="#ffffff" opacity=".35" stroke="#a9b5c6" strokeDasharray="4 4" />
      <rect x="430" y={FEET[1] - 34} width="180" height="10" rx="3" fill="#7d6450" />
      <rect x="512" y={FEET[1] - 24} width="16" height="24" fill="#6a5443" />
      <text x="525" y={FLOOR_TOP[1] + 50} textAnchor="middle" className="office__label">회의실</text>
      <g className="office__boss" onClick={onBoss} role="button" aria-label="사장석: 결재함 열기">
        <rect x="830" y={FEET[1] - 38} width="120" height="12" rx="3" fill="#5b4a3d" />
        <rect x="840" y={FEET[1] - 26} width="10" height="26" fill="#4b3d32" />
        <rect x="930" y={FEET[1] - 26} width="10" height="26" fill="#4b3d32" />
        <rect x="868" y={FEET[1] - 64} width="34" height="24" rx="3" fill="#2f3a4f" />
        <rect x="955" y={FEET[1] - 48} width="40" height="10" rx="2" fill="#e8d7b8" stroke="#a88f68" />
        <text x="975" y={FEET[1] - 54} textAnchor="middle" className="office__label">결과함</text>
        <text x="890" y={FLOOR_TOP[1] + 50} textAnchor="middle" className="office__label">사장석</text>
        <g transform={`translate(890, ${FEET[1] - 96})`}>
          <rect x="-32" y="-14" width="64" height="24" rx="12" fill={pending ? "#d4742a" : "#7c8696"} />
          <text y="3" textAnchor="middle" className="office__boss-count">결재 {pending}</text>
        </g>
      </g>
      <rect x="1030" y={FEET[1] - 92} width="30" height="92" rx="3" fill="#8a6c52" />
      <circle cx="1036" cy={FEET[1] - 46} r="2.5" fill="#e9c46a" />

      {/* 직원들 (책상 뒤에 그려 앉은 것처럼 보이게) */}
      {employees.map((e) => {
        const v = views[e.id] ?? { pose: "idle" };
        const shirt = `hsl(${e.hue} 55% 52%)`;
        const hair = `hsl(${(e.hue + 200) % 360} 25% 22%)`;
        return (
          <g
            key={e.id}
            ref={(el) => {
              refs.current[e.id] = el;
            }}
            className={`person pose-${v.pose} ${selected === e.id ? "is-selected" : ""}`}
            onClick={() => onSelect(e.id)}
            role="button"
            aria-label={`${e.name} ${e.role}`}
          >
            {/* 발밑 상태 링 */}
            <ellipse
              cx="0"
              cy="0"
              rx="16"
              ry="6"
              className={`office-ring office-ring--${
                v.chip?.tone === "work" ? "running" :
                v.chip?.tone === "wait" ? "waiting" :
                v.chip?.tone === "bad" ? "failed" :
                v.chip?.tone === "done" ? "done" : "idle"
              }`}
            />
            <g className="person__body">
              <rect className="leg leg--l" x="-8" y="-17" width="6" height="17" rx="2" fill="#39414f" />
              <rect className="leg leg--r" x="2" y="-17" width="6" height="17" rx="2" fill="#39414f" />
              <rect x="-12" y="-42" width="24" height="27" rx="9" fill={shirt} />
              <rect className="arm" x="6" y="-38" width="6" height="18" rx="3" fill={shirt} />
              <circle cx="0" cy="-55" r="14" fill="#f4d3b3" />
              <path d="M-14 -57 a14 14 0 0 1 28 0 q-6 -6 -14 -4 q-8 -2 -14 4z" fill={hair} />
              <circle cx="5" cy="-55" r="1.8" fill="#2b2b2b" />
              <circle cx="11" cy="-50" r="2.6" fill="#f2a7a0" opacity=".6" />
              {v.carrying && <rect x="10" y="-36" width="14" height="11" rx="1.5" fill="#f3c66b" stroke="#c08d2c" className="folder" />}
              {v.pose === "coffee" && <rect x="10" y="-34" width="7" height="9" rx="1.5" fill="#fff" stroke="#999" />}
            </g>
            <text y="16" textAnchor="middle" className="person__name">{e.name}</text>
            {e.engine && (
              <g transform="translate(17,10)" className="person__engine">
                <rect x="-9" y="-6" width="18" height="11" rx="3" fill={`hsl(${engineHue[e.engine.id]} 55% 45%)`} />
                <text y="2.5" textAnchor="middle">{engineShort[e.engine.id]}</text>
              </g>
            )}
            {v.chip && (
              <g
                transform="translate(0,-84)"
                className={`chip chip--${v.chip.tone}`}
                onClick={(ev) => {
                  const t = tasks.find((x) => ["running", "awaiting_approval", "queued", "paused", "failed"].includes(x.status) && (x.currentActor === e.id || x.employeeId === e.id));
                  if (t && onTaskClick) {
                    ev.stopPropagation();
                    onTaskClick(t.id);
                  }
                }}
              >
                <rect x="-24" y="-11" width="48" height="18" rx="9" />
                <text y="2" textAnchor="middle">{v.chip.text}</text>
              </g>
            )}
            {v.bubble && (
              <g
                transform="translate(0,-108)"
                className="bubble"
                onClick={(ev) => {
                  const t = tasks.find((x) => ["running", "awaiting_approval", "queued", "paused", "failed"].includes(x.status) && (x.currentActor === e.id || x.employeeId === e.id));
                  if (t && onTaskClick) {
                    ev.stopPropagation();
                    onTaskClick(t.id);
                  }
                }}
              >
                <rect x={-Math.max(30, v.bubble.length * 6.6) / 2 - 8} y="-14" width={Math.max(30, v.bubble.length * 6.6) + 16} height="22" rx="8" />
                <text y="2" textAnchor="middle">{v.bubble}</text>
              </g>
            )}
          </g>
        );
      })}

      {/* 책상 (앞쪽 레이어) */}
      {employees.map((e) => {
        const d = deskOf(e.id);
        const y = FEET[d.floor];
        return (
          <g key={`desk-${e.id}`} className={`desk ${deskState(e.id)}`} pointerEvents="none">
            <rect x={d.x - 38} y={y - 31} width="76" height="11" rx="2" fill="#a07d5c" />
            <rect x={d.x - 34} y={y - 20} width="6" height="20" fill="#86684c" />
            <rect x={d.x + 28} y={y - 20} width="6" height="20" fill="#86684c" />
            <rect x={d.x + 6} y={y - 56} width="28" height="20" rx="2" fill="#2b313c" />
            <rect className="screen" x={d.x + 8} y={y - 54} width="24" height="16" rx="1" fill="#46516a" />
            <rect x={d.x + 17} y={y - 36} width="6" height="6" fill="#2b313c" />
            <text x={d.x} y={y - 22.5} textAnchor="middle" className="desk__tag">{e.role.replace(" 개발자", "").replace(" 엔지니어", "")}</text>
            {/* 고유 책상 소품 */}
            {deskProp(e.id) === 0 && (
              <g className="desk-prop">
                <rect x={d.x - 24} y={y - 39} width="7" height="8" rx="1.5" fill="#fdfcf8" stroke="#b3ae9f" strokeWidth="0.8" />
                <rect x={d.x - 26} y={y - 36} width="3" height="4" rx="1" fill="none" stroke="#b3ae9f" strokeWidth="0.8" />
              </g>
            )}
            {deskProp(e.id) === 1 && (
              <g className="desk-prop">
                <rect x={d.x - 25} y={y - 38} width="8" height="7" rx="1.5" fill="#c4734a" />
                <circle cx={d.x - 21} cy={y - 42} r="5" fill="#3ba569" />
              </g>
            )}
            {deskProp(e.id) === 2 && (
              <g className="desk-prop">
                <rect x={d.x - 26} y={y - 35} width="12" height="4" rx="0.8" fill="#2d5d9e" />
                <rect x={d.x - 25} y={y - 39} width="10" height="4" rx="0.8" fill="#d4742a" />
              </g>
            )}
            {deskProp(e.id) === 3 && (
              <g className="desk-prop">
                <rect x={d.x - 22} y={y - 40} width="6" height="9" rx="1" fill="#7c8696" />
                <line x1={d.x - 20} y1={y - 45} x2={d.x - 20} y2={y - 40} stroke="#2b2b2b" strokeWidth="1.2" />
                <line x1={d.x - 18} y1={y - 44} x2={d.x - 17} y2={y - 40} stroke="#d4493d" strokeWidth="1.2" />
              </g>
            )}
            {deskProp(e.id) === 4 && (
              <g className="desk-prop">
                <rect x={d.x - 25} y={y - 36} width="11" height="5" rx="1" fill="#e9eaec" stroke="#9aa5b4" strokeWidth="0.6" />
                <rect x={d.x - 23} y={y - 35} width="7" height="3" fill="#3ba569" opacity="0.6" />
              </g>
            )}
          </g>
        );
      })}
    </svg>
  );
}
