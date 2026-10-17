import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, type Activity, type Bootstrap, type Employee, type Task } from "./api";

export type Route = "office" | "home" | "employees" | "work" | "usage" | "connections" | "help";

interface Toast {
  id: number;
  text: string;
  tone: "info" | "error" | "success";
}

interface Store {
  boot: Bootstrap | null;
  tasks: Task[];
  route: Route;
  selectedTaskId: string | null;
  lastEvent: { id: string; at: number } | null;
  navigate: (route: Route, taskId?: string) => void;
  refresh: () => Promise<void>;
  employee: (id: string) => Employee | undefined;
  newTask: (employeeId?: string, prefill?: Record<string, unknown>) => void;
  newTaskState: { open: boolean; employeeId?: string; prefill?: Record<string, unknown> };
  closeNewTask: () => void;
  resumeOf: string | null;
  showResume: (employeeId: string | null) => void;
  toast: (text: string, tone?: Toast["tone"]) => void;
  toasts: Toast[];
  /** 직원별 지금 하는 일 */
  activities: Record<string, Activity>;
  /** 실시간 업무 채널 (최근 활동) */
  feed: Activity[];
}

const Ctx = createContext<Store | null>(null);

function routeFromHash(): { route: Route; taskId: string | null } {
  const [path, query] = window.location.hash.replace(/^#\/?/, "").split("?");
  const route = (["office", "home", "employees", "work", "usage", "connections", "help"] as Route[]).find((r) => r === path) ?? "office";
  const taskId = new URLSearchParams(query ?? "").get("task");
  return { route, taskId };
}

export function StoreProvider({ children }: { children: ReactNode }) {
  const [boot, setBoot] = useState<Bootstrap | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const initial = routeFromHash();
  const [route, setRoute] = useState<Route>(initial.route);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(initial.taskId);
  const [lastEvent, setLastEvent] = useState<Store["lastEvent"]>(null);
  const [newTaskState, setNewTaskState] = useState<Store["newTaskState"]>({ open: false });
  const [resumeOf, setResumeOf] = useState<string | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [activities, setActivities] = useState<Record<string, Activity>>({});
  const [feed, setFeed] = useState<Activity[]>([]);
  const timer = useRef<number | undefined>(undefined);

  const toast = useCallback((text: string, tone: Toast["tone"] = "info") => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t, { id, text, tone }]);
    window.setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), tone === "error" ? 7000 : 3800);
  }, []);

  const refresh = useCallback(async () => {
    try {
      const [b, t] = await Promise.all([api.bootstrap(), api.listTasks()]);
      setBoot(b);
      setTasks(t);
    } catch (e) {
      toast(errorText(e), "error");
    }
  }, [toast]);

  useEffect(() => {
    refresh();
    const un = listen<string>("task-updated", (e) => {
      setLastEvent({ id: e.payload, at: Date.now() });
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(refresh, 120);
    });
    api.activities().then((list) => setActivities(Object.fromEntries(list.map((a) => [a.employeeId, a])))).catch(() => undefined);
    const unAct = listen<Activity>("activity", (e) => {
      const a = e.payload;
      setActivities((m) => ({ ...m, [a.employeeId]: a }));
      setFeed((f) => (f[0]?.employeeId === a.employeeId && f[0]?.label === a.label ? f : [a, ...f].slice(0, 40)));
    });
    const unEngine = listen("engine-updated", () => refresh());
    const onHash = () => {
      const r = routeFromHash();
      setRoute(r.route);
      if (r.taskId) setSelectedTaskId(r.taskId);
    };
    window.addEventListener("hashchange", onHash);
    return () => {
      un.then((f) => f());
      unAct.then((f) => f());
      unEngine.then((f) => f());
      window.removeEventListener("hashchange", onHash);
    };
  }, [refresh]);

  const navigate = useCallback((r: Route, taskId?: string) => {
    window.location.hash = `#/${r}${taskId ? `?task=${taskId}` : ""}`;
    setRoute(r);
    if (taskId) setSelectedTaskId(taskId);
  }, []);

  const employee = useCallback((id: string) => boot?.employees.find((e) => e.id === id), [boot]);

  const value: Store = {
    boot,
    tasks,
    route,
    selectedTaskId,
    lastEvent,
    navigate,
    refresh,
    employee,
    newTask: (employeeId, prefill) => setNewTaskState({ open: true, employeeId, prefill }),
    newTaskState,
    closeNewTask: () => setNewTaskState({ open: false }),
    resumeOf,
    showResume: setResumeOf,
    toast,
    toasts,
    activities,
    feed,
  };
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useStore(): Store {
  const s = useContext(Ctx);
  if (!s) throw new Error("StoreProvider missing");
  return s;
}
