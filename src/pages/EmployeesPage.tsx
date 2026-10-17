import { useEffect, useMemo, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  api,
  errorText,
  isDev,
  modeLabel,
  relativeTime,
  shortModelLabel,
  type SkillCatalogItem,
} from "../api";
import { useStore } from "../store";
import { Avatar, Empty, Icon, Modal, StatusBadge } from "../components/ui";
import { GuardAck } from "../components/NewTaskDialog";

const filters: { id: "all" | "dev" | "office"; label: string }[] = [
  { id: "all", label: "전체" },
  { id: "dev", label: "개발팀" },
  { id: "office", label: "사무팀" },
];

export function EmployeesPage() {
  const { boot, tasks, newTask, showResume, navigate, refresh, toast } = useStore();
  const [filter, setFilter] = useState<"all" | "dev" | "office">("all");
  const [q, setQ] = useState("");
  const [selected, setSelected] = useState("taeo");

  // 고용·스킬 모달 상태
  const [hireOpen, setHireOpen] = useState(false);
  const [hireTab, setHireTab] = useState<"catalog" | "github" | "custom">("catalog");
  const [catalog, setCatalog] = useState<SkillCatalogItem[]>([]);
  const [githubUrl, setGithubUrl] = useState("");
  const [importingUrl, setImportingUrl] = useState(false);
  const [savingHire, setSavingHire] = useState(false);

  // 새 직원 입력 폼
  const [empName, setEmpName] = useState("");
  const [empRole, setEmpRole] = useState("");
  const [empFloor, setEmpFloor] = useState<"dev" | "office">("dev");
  const [empTeam, setEmpTeam] = useState("개발팀");
  const empHue = 215;
  const [empSummary, setEmpSummary] = useState("");
  const [empMotto, setEmpMotto] = useState("장착된 전문 스킬로 빈틈없이 완수합니다.");
  const [empCaps, setEmpCaps] = useState("전문 스킬 수행, 품질 검증, 실무 최적화");
  const [empSkillMd, setEmpSkillMd] = useState("");
  const [empSourceUrl, setEmpSourceUrl] = useState("");

  // 선택된 직원의 스킬 직접 편집기 상태
  const [skillEditorOpen, setSkillEditorOpen] = useState(false);
  const [skillDraft, setSkillDraft] = useState("");
  const [savingSkill, setSavingSkill] = useState(false);

  useEffect(() => {
    if (hireOpen && catalog.length === 0) {
      api.listSkillCatalog().then(setCatalog).catch(() => undefined);
    }
  }, [hireOpen, catalog.length]);

  const list = useMemo(
    () =>
      (boot?.employees ?? []).filter(
        (e) =>
          (filter === "all" || (filter === "dev") === isDev(e.id)) &&
          (!q || [e.name, e.role, e.team, e.summary, ...e.capabilities].join(" ").includes(q.trim())),
      ),
    [boot, filter, q],
  );
  if (!boot) return null;
  const e = list.find((x) => x.id === selected) ?? list[0];
  const lastTask = e && tasks.find((t) => t.employeeId === e.id);

  const selectedEngineId = e?.engine?.id;
  const selectedModel = boot.settings.models[selectedEngineId ?? ""] || (selectedEngineId === "opencode" ? "deepseek-v4.1-flash-free" : "기본 모델");
  const selectedIsFree = selectedModel.endsWith("-free") || selectedEngineId === "opencode";
  const activeSkillMd = e?.equippedSkill || e?.skillMarkdown || "";

  const handleHireFromCatalog = async (item: SkillCatalogItem) => {
    setSavingHire(true);
    try {
      const created = await api.hireEmployee({
        id: item.id,
        name: item.name,
        role: item.role,
        team: item.team,
        floor: item.floor,
        hue: item.hue,
        summary: item.summary,
        motto: item.motto,
        capabilities: item.capabilities,
        skillMarkdown: item.skillMarkdown,
        sourceUrl: item.sourceUrl,
      });
      await refresh();
      setSelected(created.id);
      setHireOpen(false);
      toast(`${item.name}(${item.role}) 직원을 채용했습니다! 사무실과 업무 목록에 즉시 배치됩니다.`, "success");
    } catch (err) {
      toast(errorText(err), "error");
    } finally {
      setSavingHire(false);
    }
  };

  const handleEquipSkillToCurrent = async (skillMd: string, label: string) => {
    if (!e) return;
    setSavingHire(true);
    try {
      await api.saveEmployeeSkill(e.id, skillMd);
      await refresh();
      setHireOpen(false);
      toast(`${e.name} 직원에게 '${label}' 스킬을 장착했습니다!`, "success");
    } catch (err) {
      toast(errorText(err), "error");
    } finally {
      setSavingHire(false);
    }
  };

  const handleImportGithub = async () => {
    const url = githubUrl.trim();
    if (!url) {
      toast("GitHub SKILL.md 또는 마크다운 주소를 입력해 주세요.", "error");
      return;
    }
    setImportingUrl(true);
    try {
      const imported = await api.importSkillFromUrl(url);
      setEmpName(imported.name.slice(0, 12) || "새 직원");
      setEmpRole(imported.role || "전문 AI 에이전트");
      setEmpSummary(imported.summary);
      setEmpSkillMd(imported.skillMarkdown);
      setEmpSourceUrl(imported.sourceUrl);
      setHireTab("custom");
      toast("GitHub에서 스킬 문서를 불러왔습니다. 이름과 부서를 확인한 뒤 채용을 완료해 주세요!", "success");
    } catch (err) {
      toast(errorText(err), "error");
    } finally {
      setImportingUrl(false);
    }
  };

  const handleCreateCustomEmployee = async () => {
    if (!empName.trim() || !empRole.trim()) {
      toast("직원 이름과 담당 역할을 입력해 주세요.", "error");
      return;
    }
    setSavingHire(true);
    try {
      const caps = empCaps
        .split(",")
        .map((c) => c.trim())
        .filter(Boolean);
      const created = await api.hireEmployee({
        name: empName.trim(),
        role: empRole.trim(),
        team: empTeam.trim() || (empFloor === "dev" ? "개발팀" : "전문지원팀"),
        floor: empFloor,
        hue: empHue,
        summary: empSummary.trim() || `${empRole.trim()} 전문 스킬을 수행합니다.`,
        motto: empMotto.trim() || "장착된 전문 스킬로 빈틈없이 완수합니다.",
        capabilities: caps.length ? caps : ["전문 스킬 수행"],
        skillMarkdown: empSkillMd.trim(),
        sourceUrl: empSourceUrl.trim() || undefined,
      });
      await refresh();
      setSelected(created.id);
      setHireOpen(false);
      toast(`${created.name}(${created.role}) 직원을 새로 고용했습니다!`, "success");
    } catch (err) {
      toast(errorText(err), "error");
    } finally {
      setSavingHire(false);
    }
  };

  const handleSaveEmployeeSkill = async () => {
    if (!e) return;
    setSavingSkill(true);
    try {
      await api.saveEmployeeSkill(e.id, skillDraft);
      await refresh();
      setSkillEditorOpen(false);
      toast(skillDraft.trim() ? `${e.name} 직원의 전문 스킬(SKILL.md)을 저장했습니다.` : `${e.name} 직원의 추가 스킬을 해제했습니다.`, "success");
    } catch (err) {
      toast(errorText(err), "error");
    } finally {
      setSavingSkill(false);
    }
  };

  return (
    <div className="page">
      <div className="toolbar">
        <label className="search">
          <Icon name="search" size={16} />
          <input value={q} onChange={(ev) => setQ(ev.target.value)} placeholder="이름, 역할, 할 수 있는 일로 찾기" />
        </label>
        <div className="chips">
          {filters.map((f) => (
            <button key={f.id} className={`chip ${filter === f.id ? "is-on" : ""}`} onClick={() => setFilter(f.id)}>
              {f.label}
            </button>
          ))}
        </div>
        <button
          type="button"
          className="btn btn--primary btn--sm"
          style={{ marginLeft: "auto" }}
          onClick={() => {
            setHireTab("catalog");
            setHireOpen(true);
          }}
        >
          <Icon name="plus" size={15} /> AI 직원 추가 고용 · 스킬(SKILL.md) 가져오기
        </button>
      </div>
      <div className="master-detail">
        <aside className="master">
          <span className="master__count">{list.length}명</span>
          {list.map((x) => {
            const hasSkill = !!(x.equippedSkill || x.skillMarkdown);
            return (
              <button key={x.id} className={`emp-item ${e?.id === x.id ? "is-on" : ""}`} onClick={() => setSelected(x.id)}>
                <Avatar employee={x} size={42} showStatus />
                <span>
                  <strong>
                    {x.name} {x.custom && <small className="emp-custom-badge">신규 영입</small>}
                  </strong>
                  <small>{x.role}</small>
                  <span className="emp-item__tags">
                    <StatusBadge employee={x.availability.status} />
                    <em>{modeLabel[x.mode]}</em>
                    {hasSkill && <span className="model-chip model-chip--free">⚡ SKILL</span>}
                  </span>
                </span>
              </button>
            );
          })}
          {!list.length && <Empty title="조건에 맞는 직원이 없어요" />}
        </aside>
        {e && (
          <article className="detail emp-detail">
            <header className="emp-detail__head">
              <Avatar employee={e} size={84} showStatus />
              <div>
                <div className="emp-detail__badges">
                  <StatusBadge employee={e.availability.status} />
                  <span className="tag">{modeLabel[e.mode]}</span>
                  {e.custom && <span className="tag" style={{ background: "rgba(37,99,235,0.12)", color: "#1d4ed8", fontWeight: 700 }}>추가 고용 직원</span>}
                  {activeSkillMd && <span className="model-chip model-chip--free">⚡ 전문 스킬(SKILL.md) 장착됨</span>}
                  {e.engine && (
                    <span className={`model-chip ${selectedIsFree ? "model-chip--free" : ""}`}>
                      {e.engine.label} · {shortModelLabel(selectedModel)}
                    </span>
                  )}
                </div>
                <h2>{e.name}</h2>
                <p>
                  {e.role} · {e.team}
                </p>
              </div>
            </header>
            <p className="emp-detail__summary">{e.summary}</p>
            <blockquote className="motto">“{e.motto}”</blockquote>
            <dl className="info-grid">
              <div>
                <dt>받는 정보</dt>
                <dd>{e.inputSummary}</dd>
              </div>
              <div>
                <dt>만드는 결과</dt>
                <dd>{e.outputSummary}</dd>
              </div>
              <div>
                <dt>비용 기준</dt>
                <dd>{selectedIsFree ? `실제 0원 (무료 모델) · ${e.costSummary}` : e.costSummary}</dd>
              </div>
              <div>
                <dt>최근 업무</dt>
                <dd>{lastTask ? `${relativeTime(lastTask.updatedAt)} · ${lastTask.title}` : "아직 없음"}</dd>
              </div>
            </dl>
            <section>
              <h4>할 수 있는 일</h4>
              <div className="tags">
                {e.capabilities.map((c) => (
                  <span key={c} className="tag">
                    {c}
                  </span>
                ))}
              </div>
            </section>

            {/* ───────── 장착된 전문 스킬 (SKILL.md) 섹션 ───────── */}
            <section className="emp-skill-box">
              <div className="emp-skill-box__head">
                <div>
                  <h4>⚡ 전문 스킬 지침 (SKILL.md)</h4>
                  <small>
                    {activeSkillMd
                      ? "이 직원에게 업무를 맡기면 아래 전문 스킬 지침이 OpenCode 에이전트 프롬프트에 자동 주입됩니다."
                      : "GitHub에서 찾은 SKILL.md나 나만의 작업 규칙을 이 직원에게 장착할 수 있습니다."}
                  </small>
                </div>
                <div style={{ display: "flex", gap: 6 }}>
                  <button
                    type="button"
                    className="btn btn--ghost btn--sm"
                    onClick={() => {
                      setSkillDraft(activeSkillMd);
                      setSkillEditorOpen((v) => !v);
                    }}
                  >
                    <Icon name="doc" size={14} /> {skillEditorOpen ? "편집 닫기" : activeSkillMd ? "스킬 보기·편집" : "스킬 장착하기"}
                  </button>
                  <button
                    type="button"
                    className="btn btn--ghost btn--sm"
                    onClick={() => {
                      setHireTab("catalog");
                      setHireOpen(true);
                    }}
                  >
                    <Icon name="spark" size={14} /> 카탈로그·GitHub에서 가져오기
                  </button>
                </div>
              </div>
              {e.sourceUrl && (
                <p style={{ fontSize: 12, color: "var(--muted)", marginTop: 4 }}>
                  출처: <code>{e.sourceUrl}</code>
                </p>
              )}
              {skillEditorOpen && (
                <div className="emp-skill-box__editor">
                  <textarea
                    rows={8}
                    value={skillDraft}
                    onChange={(ev) => setSkillDraft(ev.target.value)}
                    placeholder="# 전문 스킬 지침 (SKILL.md)\n- 작업 원칙, 코딩 컨벤션, 체크리스트 등을 마크다운으로 적어 주세요."
                  />
                  <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 8 }}>
                    {activeSkillMd && (
                      <button
                        type="button"
                        className="btn btn--ghost btn--sm"
                        disabled={savingSkill}
                        onClick={() => {
                          setSkillDraft("");
                        }}
                      >
                        비우기
                      </button>
                    )}
                    <button
                      type="button"
                      className="btn btn--primary btn--sm"
                      disabled={savingSkill}
                      onClick={handleSaveEmployeeSkill}
                    >
                      {savingSkill ? "저장 중…" : "스킬 저장"}
                    </button>
                  </div>
                </div>
              )}
            </section>

            <section>
              <h4>업무 흐름</h4>
              <ol className="flow">
                {e.steps.map((s) => (
                  <li key={s.key}>
                    {s.label}
                    {s.gate && <em>{s.gate === "cost" ? "비용 확인" : "변경 확인"}</em>}
                  </li>
                ))}
              </ol>
            </section>
            {e.availability.status === "setup_required" && (
              <div className="callout callout--warning">
                <strong>먼저 준비해 주세요: {e.availability.missing.join(", ")}</strong>
                <button className="btn btn--sm" onClick={() => navigate("connections")}>
                  연결 및 설정으로
                </button>
              </div>
            )}
            {e.availability.status === "guarded" && (
              <div className="callout callout--critical">
                <strong>같은 문제로 두 번 실패해 잠시 멈춰 두었습니다.</strong>
                <p>{e.availability.guard}</p>
                <GuardAck employeeId={e.id} />
              </div>
            )}
            {e.notice && <p className="preflight__notice">{e.notice}</p>}
            <footer className="detail__actions">
              {e.custom && (
                <button
                  className="btn btn--ghost btn--danger"
                  onClick={async () => {
                    const ok = await ask(`${e.name} 직원을 직원 목록에서 해고(제거)할까요?`, {
                      title: "직원 해고",
                      kind: "warning",
                      okLabel: "해고",
                      cancelLabel: "취소",
                    });
                    if (ok) {
                      try {
                        await api.dismissEmployee(e.id);
                        await refresh();
                        setSelected("taeo");
                        toast(`${e.name} 직원을 목록에서 제거했습니다.`, "info");
                      } catch (err) {
                        toast(errorText(err), "error");
                      }
                    }
                  }}
                >
                  <Icon name="trash" size={16} /> 직원 해고
                </button>
              )}
              <button className="btn btn--ghost" onClick={() => showResume(e.id)}>
                입사지원서 보기
              </button>
              <button className="btn btn--primary" onClick={() => newTask(e.id)}>
                <Icon name="plus" size={16} /> 업무 맡기기
              </button>
            </footer>
          </article>
        )}
      </div>

      {/* ───────── AI 직원 추가 고용 & 오픈소스 스킬(SKILL.md) 영입 모달 ───────── */}
      {hireOpen && (
        <Modal
          wide
          title="➕ AI 직원 추가 고용 & 오픈소스 스킬(SKILL.md) 가져오기"
          description="검증된 오픈소스 에이전트 스킬 카탈로그나 GitHub URL에서 새로운 전문 직원을 영입하거나, 기존 직원에게 스킬을 장착합니다."
          onClose={() => setHireOpen(false)}
        >
          <div className="tabs" style={{ marginBottom: 16 }}>
            <button
              type="button"
              className={`tab ${hireTab === "catalog" ? "is-on" : ""}`}
              onClick={() => setHireTab("catalog")}
            >
              🌟 오픈소스 추천 스킬 카탈로그 ({catalog.length}종)
            </button>
            <button
              type="button"
              className={`tab ${hireTab === "github" ? "is-on" : ""}`}
              onClick={() => setHireTab("github")}
            >
              🔗 GitHub URL (SKILL.md)로 가져오기
            </button>
            <button
              type="button"
              className={`tab ${hireTab === "custom" ? "is-on" : ""}`}
              onClick={() => setHireTab("custom")}
            >
              ✍️ 직접 AI 직원 & 스킬 만들기
            </button>
          </div>

          {hireTab === "catalog" && (
            <div className="skill-catalog">
              <p className="field__hint" style={{ marginBottom: 12 }}>
                글로벌 오픈소스 프로젝트(<code>msitarzewski/agency-agents</code>, <code>anthropics/skills</code>)를 한국어 실무 환경에 맞게 다듬은 전문가 프리셋입니다. 새 직원으로 바로 채용하거나 현재 선택된 직원(<strong>{e?.name}</strong>)에게 장착할 수 있습니다.
              </p>
              <div className="skill-catalog__list">
                {catalog.map((item) => {
                  const alreadyHired = boot.employees.some((emp) => emp.id === item.id);
                  return (
                    <article key={item.id} className="skill-catalog__card">
                      <div className="skill-catalog__top">
                        <div>
                          <span className="badge badge--info" style={{ marginRight: 6 }}>
                            {item.floor === "dev" ? "3F 개발실 (코드 작성·수정)" : "2F 사무팀 (기획·문서·분석)"}
                          </span>
                          <strong>{item.name} · {item.role}</strong>
                        </div>
                        <small style={{ color: "var(--muted)" }}>{item.team}</small>
                      </div>
                      <p style={{ margin: "8px 0", fontSize: 13.5 }}>{item.summary}</p>
                      <div className="tags" style={{ marginBottom: 10 }}>
                        {item.capabilities.map((c) => (
                          <span key={c} className="tag">{c}</span>
                        ))}
                      </div>
                      <details style={{ marginBottom: 10, fontSize: 12 }}>
                        <summary style={{ cursor: "pointer", color: "var(--muted)" }}>장착된 SKILL.md 전문 지침 미리보기</summary>
                        <pre className="skill-catalog__pre">{item.skillMarkdown}</pre>
                      </details>
                      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                        <button
                          type="button"
                          className="btn btn--primary btn--sm"
                          disabled={savingHire}
                          onClick={() => handleHireFromCatalog(item)}
                        >
                          <Icon name="plus" size={14} /> {alreadyHired ? `${item.name} 프로필 업데이트` : `${item.name} 직원으로 바로 채용`}
                        </button>
                        {e && (
                          <button
                            type="button"
                            className="btn btn--ghost btn--sm"
                            disabled={savingHire}
                            onClick={() => handleEquipSkillToCurrent(item.skillMarkdown, `${item.name} (${item.role})`)}
                          >
                            ⚡ 현재 직원({e.name})에게 이 스킬 장착
                          </button>
                        )}
                      </div>
                    </article>
                  );
                })}
              </div>
            </div>
          )}

          {hireTab === "github" && (
            <div className="form-grid">
              <div className="field field--wide">
                <label className="field__label">GitHub SKILL.md 또는 마크다운(.md) URL</label>
                <p className="field__hint">
                  GitHub의 일반 파일 주소(<code>https://github.com/.../blob/main/.../SKILL.md</code>)나 Raw 주소를 붙여 넣으면 자동으로 Raw 변환 후 제목·역할·지침을 추출합니다.
                </p>
                <div style={{ display: "flex", gap: 8, marginTop: 6 }}>
                  <input
                    value={githubUrl}
                    onChange={(ev) => setGithubUrl(ev.target.value)}
                    placeholder="예: https://github.com/msitarzewski/agency-agents/blob/main/design/design-ui-designer.md"
                  />
                  <button
                    type="button"
                    className="btn btn--primary"
                    disabled={importingUrl}
                    onClick={handleImportGithub}
                  >
                    {importingUrl ? "불러오는 중…" : "스킬 불러오기"}
                  </button>
                </div>
              </div>
              <div className="field field--wide">
                <label className="field__label">자주 쓰는 오픈소스 스킬 저장소 예시 (클릭 시 주소 자동 입력)</label>
                <div className="chips" style={{ marginTop: 6 }}>
                  {[
                    {
                      label: "UI Designer (agency-agents)",
                      url: "https://raw.githubusercontent.com/msitarzewski/agency-agents/main/design/design-ui-designer.md",
                    },
                    {
                      label: "Frontend Developer (agency-agents)",
                      url: "https://raw.githubusercontent.com/msitarzewski/agency-agents/main/engineering/engineering-frontend-developer.md",
                    },
                    {
                      label: "Backend Architect (agency-agents)",
                      url: "https://raw.githubusercontent.com/msitarzewski/agency-agents/main/engineering/engineering-backend-architect.md",
                    },
                    {
                      label: "Security Engineer (agency-agents)",
                      url: "https://raw.githubusercontent.com/msitarzewski/agency-agents/main/engineering/engineering-security-engineer.md",
                    },
                  ].map((ex) => (
                    <button key={ex.label} type="button" className="chip" onClick={() => setGithubUrl(ex.url)}>
                      {ex.label}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}

          {hireTab === "custom" && (
            <div className="form-grid">
              <div className="field">
                <label className="field__label">직원 이름 *</label>
                <input value={empName} onChange={(ev) => setEmpName(ev.target.value)} placeholder="예: 하준, 시우, 나윤" />
              </div>
              <div className="field">
                <label className="field__label">담당 역할 (직무) *</label>
                <input value={empRole} onChange={(ev) => setEmpRole(ev.target.value)} placeholder="예: UI/UX 디자이너, 보안 엔지니어, 상세페이지 카피라이터" />
              </div>
              <div className="field">
                <label className="field__label">배치할 층·작업 방식 *</label>
                <select
                  value={empFloor}
                  onChange={(ev) => {
                    const next = ev.target.value as "dev" | "office";
                    setEmpFloor(next);
                    setEmpTeam(next === "dev" ? "개발팀" : "전문지원팀");
                  }}
                >
                  <option value="dev">3F 개발실 (코드·파일 직접 생성 및 수정 가능)</option>
                  <option value="office">2F 사무팀 (리서치·기획서·카피·보고서 작성)</option>
                </select>
              </div>
              <div className="field">
                <label className="field__label">소속 팀 이름</label>
                <input value={empTeam} onChange={(ev) => setEmpTeam(ev.target.value)} placeholder="예: 개발팀, 디자인팀, 마케팅팀" />
              </div>
              <div className="field field--wide">
                <label className="field__label">한 줄 소개</label>
                <input value={empSummary} onChange={(ev) => setEmpSummary(ev.target.value)} placeholder="이 직원이 어떤 전문성을 가지고 어떻게 일하는지 적어 주세요." />
              </div>
              <div className="field">
                <label className="field__label">좌우명 (한마디)</label>
                <input value={empMotto} onChange={(ev) => setEmpMotto(ev.target.value)} />
              </div>
              <div className="field">
                <label className="field__label">할 수 있는 일 (쉼표로 구분)</label>
                <input value={empCaps} onChange={(ev) => setEmpCaps(ev.target.value)} placeholder="예: 디자인 시스템 설계, 다크모드 구현, 반응형 점검" />
              </div>
              <div className="field field--wide">
                <label className="field__label">전문 스킬 지침 (SKILL.md 마크다운)</label>
                <textarea
                  rows={8}
                  value={empSkillMd}
                  onChange={(ev) => setEmpSkillMd(ev.target.value)}
                  placeholder="# 전문 스킬 지침 (SKILL.md)\n이 직원에게 업무를 맡길 때 OpenCode 에이전트가 반드시 준수할 작업 규칙·프레임워크·체크리스트를 마크다운으로 적어 주세요."
                />
              </div>
              <div className="field field--wide" style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
                {e && empSkillMd.trim() && (
                  <button
                    type="button"
                    className="btn btn--ghost"
                    disabled={savingHire}
                    onClick={() => handleEquipSkillToCurrent(empSkillMd, empRole || empName || "커스텀 스킬")}
                  >
                    현재 직원({e.name})에게만 스킬 장착
                  </button>
                )}
                <button
                  type="button"
                  className="btn btn--primary"
                  disabled={savingHire}
                  onClick={handleCreateCustomEmployee}
                >
                  <Icon name="plus" size={16} /> {savingHire ? "채용 처리 중…" : "새 AI 직원으로 정식 채용"}
                </button>
              </div>
            </div>
          )}
        </Modal>
      )}
    </div>
  );
}
