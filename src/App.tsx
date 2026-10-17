import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { StoreProvider, useStore, type Route } from "./store";
import { Icon } from "./components/ui";
import { NewTaskDialog } from "./components/NewTaskDialog";
import { ResumeDialog } from "./components/ResumeDialog";
import { HomePage } from "./pages/HomePage";
import { OfficePage } from "./pages/OfficePage";
import { EmployeesPage } from "./pages/EmployeesPage";
import { WorkPage } from "./pages/WorkPage";
import { ConnectionsPage } from "./pages/ConnectionsPage";
import { HelpPage } from "./pages/HelpPage";
import { UsagePage } from "./pages/UsagePage";

const nav: { id: Route; label: string; description: string; icon: string }[] = [
  { id: "office", label: "사무실", description: "직원들이 일하는 모습을 실시간으로", icon: "office" },
  { id: "home", label: "홈", description: "확인할 일과 진행 중인 업무", icon: "home" },
  { id: "employees", label: "AI 직원", description: "직원 소개와 업무 맡기기", icon: "people" },
  { id: "work", label: "업무", description: "전체 업무와 결과물", icon: "work" },
  { id: "usage", label: "사용량·예산", description: "토큰 사용량, 환산 비용, 구독 한도 및 일일 예산", icon: "gauge" },
  { id: "connections", label: "연결 및 설정", description: "OpenCode 연결, 실행 방식, 사업장 정보", icon: "link" },
  { id: "help", label: "도움말", description: "일하는 방식과 자주 묻는 질문", icon: "help" },
];

function UpdaterWidget() {
  const { toast } = useStore();
  const [update, setUpdate] = useState<Update | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);

  const checkForUpdate = async (silent = false) => {
    setChecking(true);
    try {
      const found = await check();
      if (found) {
        setUpdate(found);
        if (!silent) {
          toast(`새 버전(v${found.version})이 있습니다!`, "info");
        }
      } else if (!silent) {
        toast("현재 최신 버전을 사용 중입니다.", "success");
      }
    } catch {
      if (!silent) {
        toast("아직 배포된 업데이트가 없거나 확인하지 못했습니다.", "info");
      }
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => {
    checkForUpdate(true);
  }, []);

  const installUpdate = async () => {
    if (!update) return;
    setInstalling(true);
    try {
      toast(`v${update.version} 무설치 업데이트를 내려받는 중…`, "info");
      await update.downloadAndInstall();
      await relaunch();
    } catch (e) {
      toast(`업데이트 실패: ${e instanceof Error ? e.message : String(e)}`, "error");
      setInstalling(false);
    }
  };

  if (update) {
    return (
      <button
        type="button"
        className="btn btn--primary btn--sm"
        style={{ width: "100%", justifyContent: "center", marginBottom: 6 }}
        disabled={installing}
        onClick={installUpdate}
      >
        <Icon name="spark" size={14} />
        {installing ? "업데이트 적용 중…" : `⬆️ v${update.version} 무설치 업데이트`}
      </button>
    );
  }

  return (
    <button
      type="button"
      className="link-btn"
      style={{ fontSize: 11.5, marginBottom: 4 }}
      disabled={checking}
      onClick={() => checkForUpdate(false)}
    >
      <Icon name="refresh" size={12} /> {checking ? "확인 중…" : "업데이트 확인"}
    </button>
  );
}

function Shell() {
  const { route, navigate, tasks, newTask, toasts, boot } = useStore();
  const current = nav.find((n) => n.id === route)!;
  const attention = tasks.filter((t) => t.status === "awaiting_approval").length;
  const running = tasks.filter((t) => t.status === "running").length;
  const page = {
    office: <OfficePage />,
    home: <HomePage />,
    employees: <EmployeesPage />,
    work: <WorkPage />,
    usage: <UsagePage />,
    connections: <ConnectionsPage />,
    help: <HelpPage />,
  }[route];

  return (
    <div className="shell">
      <aside className="side">
        <div className="brand">
          <span className="brand__mark">일</span>
          <span className="brand__text">
            <strong>일손</strong>
            <small>AI 코딩 회사</small>
          </span>
        </div>
        <nav className="nav">
          {nav.map((n) => (
            <button key={n.id} className={route === n.id ? "is-on" : ""} onClick={() => navigate(n.id)} aria-current={route === n.id ? "page" : undefined}>
              <Icon name={n.icon} />
              <span>{n.label}</span>
              {n.id === "work" && attention > 0 && <b className="nav__count">{attention}</b>}
            </button>
          ))}
        </nav>
        <div className="side__foot">
          {running > 0 && (
            <p className="side__live">
              <i /> {running}건 처리 중
            </p>
          )}
          <UpdaterWidget />
          <small>v{boot?.version ?? ""} · OpenCode 자동화</small>
        </div>
      </aside>
      <main className="main">
        <header className="topbar">
          <div>
            <h1>{current.label}</h1>
            <p>{current.description}</p>
          </div>
          <button className="btn btn--primary" onClick={() => newTask()}>
            <Icon name="plus" size={16} /> 새 업무
          </button>
        </header>
        <div className="content">{boot ? page : <div className="loading">사무소 문을 여는 중…</div>}</div>
      </main>
      <NewTaskDialog />
      <ResumeDialog />
      <div className="toasts" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast toast--${t.tone}`}>
            {t.text}
          </div>
        ))}
      </div>
    </div>
  );
}

export default function App() {
  return (
    <StoreProvider>
      <Shell />
    </StoreProvider>
  );
}
