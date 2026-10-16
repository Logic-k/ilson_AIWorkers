import { useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorText, type Deliverable, type Task } from "../api";
import { useStore } from "../store";
import { Icon, Markdown, markdownToHtml } from "./ui";
import { QuoteSheet } from "./QuoteSheet";

const safeName = (s: string) => s.replace(/[\\/:*?"<>|]/g, " ").trim().slice(0, 80) || "결과";

function htmlDocument(title: string, body: string, css = "") {
  return `<!doctype html><html lang="ko"><head><meta charset="utf-8"><title>${title.replace(/</g, "&lt;")}</title><style>
body{font-family:"Pretendard","Apple SD Gothic Neo","Malgun Gothic",sans-serif;max-width:760px;margin:40px auto;padding:0 20px;line-height:1.75;color:#1b1d1f}
table{border-collapse:collapse;width:100%}th,td{border:1px solid #ddd;padding:6px 10px;text-align:left}blockquote{border-left:3px solid #d9792b;margin:0;padding:4px 14px;color:#555}
${css}</style></head><body>${body}</body></html>`;
}

/** 블로그 편집기에는 '본문'만 붙여 넣는 경우가 많아 따로 꺼낸다. */
function blogBody(md: string): string | null {
  const m = md.match(/## 본문\s*\n([\s\S]*?)(\n## |$)/);
  return m ? m[1].trim() : null;
}

async function copyRich(md: string) {
  const html = markdownToHtml(md);
  await navigator.clipboard.write([
    new ClipboardItem({
      "text/html": new Blob([html], { type: "text/html" }),
      "text/plain": new Blob([md], { type: "text/plain" }),
    }),
  ]);
}

function quoteCss(): string {
  const out: string[] = [];
  for (const sheet of Array.from(document.styleSheets)) {
    try {
      for (const rule of Array.from(sheet.cssRules)) {
        const text = rule.cssText;
        if (text.includes("quote-sheet") || text.startsWith(":root")) out.push(text);
      }
    } catch {
      /* 다른 출처 스타일시트는 건너뛴다 */
    }
  }
  return out.join("\n");
}

export function DeliverableView({ task, result }: { task: Task; result: Deliverable }) {
  const { toast } = useStore();
  const sheetRef = useRef<HTMLDivElement>(null);
  const [undoing, setUndoing] = useState(false);

  const saveFile = async (ext: "md" | "html" | "txt", content: () => string) => {
    const path = await save({ defaultPath: `${safeName(result.title)}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    if (!path) return;
    try {
      await api.saveTextFile(path, content());
      toast("파일로 저장했습니다.", "success");
    } catch (e) {
      toast(errorText(e), "error");
    }
  };

  if (result.kind === "quote") {
    return (
      <div className="deliverable">
        <div className="deliverable__actions">
          <button className="btn btn--primary btn--sm" onClick={() => window.print()}>
            <Icon name="print" size={16} /> 인쇄 · PDF로 저장
          </button>
          <button
            className="btn btn--ghost btn--sm"
            onClick={() => saveFile("html", () => htmlDocument(result.title, sheetRef.current?.innerHTML ?? "", quoteCss()))}
          >
            <Icon name="download" size={16} /> HTML로 저장
          </button>
          <small>인쇄 창에서 ‘PDF로 저장’을 고르면 PDF 파일이 됩니다.</small>
        </div>
        <div className="print-root" ref={sheetRef}>
          <QuoteSheet quote={result.quote} />
        </div>
      </div>
    );
  }

  if (result.kind === "organize") {
    return (
      <div className="deliverable">
        <div className="deliverable__actions">
          <button className="btn btn--ghost btn--sm" onClick={() => revealItemInDir(result.folder)}>
            <Icon name="folder" size={16} /> 탐색기에서 보기
          </button>
          {!result.undone ? (
            <button
              className="btn btn--ghost btn--sm"
              disabled={undoing}
              onClick={async () => {
                setUndoing(true);
                try {
                  toast(await api.undoOrganize(task.id), "success");
                } catch (e) {
                  toast(errorText(e), "error");
                } finally {
                  setUndoing(false);
                }
              }}
            >
              <Icon name="undo" size={16} /> 원래대로 되돌리기
            </button>
          ) : (
            <span className="badge badge--neutral">
              <i />
              되돌림 완료
            </span>
          )}
        </div>
        <Markdown text={result.markdown} />
        <details className="moves">
          <summary>옮긴 파일 {result.moves.filter((m) => m.status === "moved").length}개 전체 보기</summary>
          <ul>
            {result.moves.map((m) => (
              <li key={m.from} className={m.status === "moved" ? "" : "is-skipped"}>
                {m.label}
                {m.reason && <small> — {m.reason}</small>}
              </li>
            ))}
          </ul>
        </details>
      </div>
    );
  }

  if (result.kind === "code") {
    const files = result.files ?? [];
    return (
      <div className="deliverable">
        <div className="deliverable__actions">
          {result.merged ? (
            <span className="badge badge--positive">
              <i />
              {result.workspace?.baseBranch ?? "원래 브랜치"}에 병합됨 · {result.mergeCommit}
            </span>
          ) : (
            <span className="badge badge--neutral">
              <i />
              병합할 변경 없음
            </span>
          )}
          {result.verdict && <span className={`badge badge--${result.verdict === "통과" ? "positive" : "warning"}`}><i />리뷰 {result.verdict}</span>}
          {result.workspace?.repo && (
            <>
              <button className="btn btn--ghost btn--sm" onClick={() => revealItemInDir(result.workspace!.repo)}>
                <Icon name="folder" size={16} /> 저장소 열기
              </button>
              <button className="btn btn--ghost btn--sm" onClick={() => api.openInAntigravity(result.workspace!.repo).catch((e) => toast(errorText(e), "error"))}>
                <Icon name="external" size={16} /> Antigravity로 열기
              </button>
            </>
          )}
          <button className="btn btn--ghost btn--sm" onClick={() => saveFile("md", () => result.markdown)}>
            <Icon name="download" size={16} /> 보고서 저장
          </button>
        </div>
        {files.length > 0 && (
          <div className="code-summary">
            <strong>
              바뀐 파일 {files.length}개 · <span className="diff__add">+{result.added ?? 0}</span> <span className="diff__del">−{result.removed ?? 0}</span>
            </strong>
            <ul className="diff__files">
              {files.map((f) => (
                <li key={f.path}>
                  <code>{f.path}</code>
                  <span className="diff__add">+{f.added}</span>
                  <span className="diff__del">−{f.removed}</span>
                </li>
              ))}
            </ul>
            {(result.commits ?? []).length > 0 && (
              <details>
                <summary>커밋 {(result.commits ?? []).length}개</summary>
                <ul className="commits">
                  {(result.commits ?? []).map((c) => (
                    <li key={c}>
                      <code>{c}</code>
                    </li>
                  ))}
                </ul>
              </details>
            )}
          </div>
        )}
        <Markdown text={result.markdown} />
      </div>
    );
  }

  const body = task.employeeId === "harin" ? blogBody(result.markdown) : null;
  return (
    <div className="deliverable">
      <div className="deliverable__actions">
        <button
          className="btn btn--primary btn--sm"
          onClick={async () => {
            await copyRich(body ?? result.markdown);
            toast(body ? "본문을 서식과 함께 복사했습니다. 블로그 편집기에 붙여 넣으세요." : "서식과 함께 복사했습니다.", "success");
          }}
        >
          <Icon name="copy" size={16} /> {body ? "본문 서식 복사" : "서식 복사"}
        </button>
        <button
          className="btn btn--ghost btn--sm"
          onClick={async () => {
            await navigator.clipboard.writeText(result.markdown);
            toast("전체 내용을 텍스트로 복사했습니다.", "success");
          }}
        >
          <Icon name="doc" size={16} /> 텍스트 복사
        </button>
        <button className="btn btn--ghost btn--sm" onClick={() => saveFile("md", () => result.markdown)}>
          <Icon name="download" size={16} /> .md 저장
        </button>
        <button className="btn btn--ghost btn--sm" onClick={() => saveFile("html", () => htmlDocument(result.title, markdownToHtml(result.markdown)))}>
          <Icon name="download" size={16} /> .html 저장
        </button>
      </div>
      {result.meta?.length > 0 && (
        <dl className="meta-grid">
          {result.meta.map((m) => (
            <div key={m.label}>
              <dt>{m.label}</dt>
              <dd>{m.value}</dd>
            </div>
          ))}
        </dl>
      )}
      {result.checks && result.checks.length > 0 && (
        <ul className="checks">
          {result.checks.map((c) => (
            <li key={c.label} className={c.ok ? "is-ok" : "is-warn"}>
              <Icon name={c.ok ? "check" : "help"} size={16} />
              <span>{c.label}</span>
              <em>{c.value}</em>
            </li>
          ))}
        </ul>
      )}
      {result.truncated && <p className="callout callout--warning">답변이 길이 한도에 닿아 끝부분이 잘렸을 수 있습니다.</p>}
      <Markdown text={result.markdown} />
    </div>
  );
}
