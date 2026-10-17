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
