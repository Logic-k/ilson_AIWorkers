import { open } from "@tauri-apps/plugin-dialog";
import type { Field, Section } from "../api";
import { Icon } from "./ui";

export type Values = Record<string, unknown>;

export interface ItemRow {
  name: string;
  spec: string;
  qty: number | string;
  unitPrice: number | string;
}

const today = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};

export function defaultValues(form: Section[]): Values {
  const v: Values = {};
  for (const s of form)
    for (const f of s.fields) {
      if (f.defaultToday) v[f.id] = today();
      else if (f.default !== undefined) v[f.id] = structuredClone(f.default);
      else v[f.id] = f.kind === "checkbox" ? false : f.kind === "items" ? [] : "";
    }
  return v;
}

export function isVisible(f: Field, values: Values) {
  return !f.visibleWhen || f.visibleWhen.oneOf.includes(String(values[f.visibleWhen.field] ?? ""));
}

/** 서버 검증과 같은 규칙을 화면에서도 먼저 알려 준다. */
export function missingFields(form: Section[], values: Values): string[] {
  const out: string[] = [];
  for (const s of form)
    for (const f of s.fields) {
      if (!f.required || !isVisible(f, values)) continue;
      const v = values[f.id];
      const filled = f.kind === "items" ? Array.isArray(v) && v.some((r: ItemRow) => String(r.name ?? "").trim()) : String(v ?? "").trim() !== "";
      if (!filled) out.push(f.label);
    }
  return out;
}

function ItemsEditor({ rows, onChange }: { rows: ItemRow[]; onChange: (rows: ItemRow[]) => void }) {
  const set = (i: number, key: keyof ItemRow, value: string) => onChange(rows.map((r, j) => (j === i ? { ...r, [key]: value } : r)));
  return (
    <div className="items">
      <div className="items__head">
        <span>항목</span>
        <span>내용</span>
        <span>수량</span>
        <span>단가(원)</span>
        <span>금액</span>
        <span />
      </div>
      {rows.map((r, i) => (
        <div className="items__row" key={i}>
          <input value={r.name} placeholder="예: 기본 디자인" maxLength={40} onChange={(e) => set(i, "name", e.target.value)} aria-label="항목" />
          <input value={r.spec} placeholder="예: 시안 1종" maxLength={80} onChange={(e) => set(i, "spec", e.target.value)} aria-label="내용" />
          <input value={r.qty} inputMode="decimal" onChange={(e) => set(i, "qty", e.target.value)} aria-label="수량" />
          <input value={r.unitPrice} inputMode="numeric" onChange={(e) => set(i, "unitPrice", e.target.value.replace(/[^\d]/g, ""))} aria-label="단가" />
          <span className="items__amount">{(Number(r.qty || 0) * Number(r.unitPrice || 0)).toLocaleString("ko-KR")}</span>
          <button type="button" className="icon-btn" onClick={() => onChange(rows.filter((_, j) => j !== i))} aria-label="항목 삭제">
            <Icon name="close" size={16} />
          </button>
        </div>
      ))}
      <button type="button" className="btn btn--ghost btn--sm" onClick={() => onChange([...rows, { name: "", spec: "", qty: 1, unitPrice: 0 }])}>
        <Icon name="plus" size={16} /> 항목 추가
      </button>
    </div>
  );
}

function FieldInput({ field, value, onChange }: { field: Field; value: unknown; onChange: (v: unknown) => void }) {
  const str = value === undefined || value === null ? "" : String(value);
  switch (field.kind) {
    case "textarea":
      return <textarea value={str} rows={3} maxLength={4000} placeholder={field.placeholder} onChange={(e) => onChange(e.target.value)} />;
    case "number":
      return <input type="number" value={str} min={field.min} max={field.max} onChange={(e) => onChange(e.target.value)} />;
    case "date":
      return <input type="date" value={str} onChange={(e) => onChange(e.target.value)} />;
    case "select": {
      const hint = field.choices?.find((c) => c.value === str)?.hint;
      return (
        <>
          <select value={str} onChange={(e) => onChange(e.target.value)}>
            {field.choices?.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
          {hint && <small className="field__hint">{hint}</small>}
        </>
      );
    }
    case "choice":
      if (field.variant === "cards")
        return (
          <div className="choice-cards" role="radiogroup">
            {field.choices?.map((c) => (
              <button type="button" key={c.value} role="radio" aria-checked={str === c.value} className={`choice-card ${str === c.value ? "is-on" : ""}`} onClick={() => onChange(c.value)}>
                <strong>{c.label}</strong>
                {c.hint && <span>{c.hint}</span>}
                {c.example && (
                  <ol className="choice-card__example">
                    {c.example.map((l) => (
                      <li key={l}>{l}</li>
                    ))}
                  </ol>
                )}
              </button>
            ))}
          </div>
        );
      return (
        <div className="chips" role="radiogroup">
          {field.choices?.map((c) => (
            <button type="button" key={c.value} role="radio" aria-checked={str === c.value} className={`chip ${str === c.value ? "is-on" : ""}`} onClick={() => onChange(c.value)}>
              {c.label}
            </button>
          ))}
        </div>
      );
    case "checkbox":
      return (
        <label className="check">
          <input type="checkbox" checked={Boolean(value)} onChange={(e) => onChange(e.target.checked)} />
          <span>{field.label}</span>
        </label>
      );
    case "folder":
      return (
        <div className="folder-pick">
          <input value={str} readOnly placeholder="폴더를 골라 주세요" />
          <button
            type="button"
            className="btn btn--ghost"
            onClick={async () => {
              const picked = await open({ directory: true, multiple: false, title: field.label });
              if (typeof picked === "string") onChange(picked);
            }}
          >
            <Icon name="folder" size={16} /> 폴더 고르기
          </button>
        </div>
      );
    case "items":
      return <ItemsEditor rows={(value as ItemRow[]) ?? []} onChange={onChange} />;
    default:
      return <input value={str} maxLength={field.maxLength ?? 200} placeholder={field.placeholder} onChange={(e) => onChange(e.target.value)} />;
  }
}

export function TaskForm({ form, values, onChange }: { form: Section[]; values: Values; onChange: (v: Values) => void }) {
  const set = (id: string, v: unknown) => onChange({ ...values, [id]: v });
  return (
    <>
      {form.map((section) => {
        const fields = section.fields.filter((f) => isVisible(f, values));
        if (!fields.length) return null;
        const body = (
          <div className="form-grid">
            {fields.map((f) => (
              <div key={f.id} className={`field ${["textarea", "choice", "items", "folder"].includes(f.kind) ? "field--wide" : ""}`}>
                {f.kind !== "checkbox" && (
                  <label className="field__label">
                    {f.label}
                    {f.required && <em>필수</em>}
                  </label>
                )}
                <FieldInput field={f} value={values[f.id]} onChange={(v) => set(f.id, v)} />
                {f.hint && f.kind !== "select" && <small className="field__hint">{f.hint}</small>}
              </div>
            ))}
          </div>
        );
        return section.advanced ? (
          <details key={section.title} className="form-section form-section--advanced">
            <summary>{section.title}</summary>
            {body}
          </details>
        ) : (
          <section key={section.title} className="form-section">
            <h3>{section.title}</h3>
            {section.description && <p className="form-section__desc">{section.description}</p>}
            {body}
          </section>
        );
      })}
    </>
  );
}
