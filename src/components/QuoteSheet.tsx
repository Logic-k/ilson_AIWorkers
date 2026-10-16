import type { Quote, Settings } from "../api";
import type { ItemRow, Values } from "./TaskForm";

const n = (v: number) => Math.round(v).toLocaleString("ko-KR");

function koreanAmount(value: number): string {
  if (value <= 0) return "영";
  const digits = ["", "일", "이", "삼", "사", "오", "육", "칠", "팔", "구"];
  const small = ["천", "백", "십", ""];
  const big = ["", "만", "억", "조"];
  const parts: string[] = [];
  let rest = Math.floor(value);
  for (let b = 0; rest > 0; b++, rest = Math.floor(rest / 10000)) {
    const chunk = rest % 10000;
    if (!chunk) continue;
    const ds = [Math.floor(chunk / 1000), Math.floor(chunk / 100) % 10, Math.floor(chunk / 10) % 10, chunk % 10];
    parts.unshift(ds.map((d, i) => (d ? digits[d] + small[i] : "")).join("") + big[b]);
  }
  return parts.join("");
}

/** 견적 폼을 채우는 동안 보여 줄 미리보기. 실제 견적서는 Rust 쪽 계산 결과를 쓴다. */
export function previewQuote(values: Values, s: Settings): Quote {
  const rows = ((values.items as ItemRow[]) ?? []).filter((r) => String(r.name).trim() || Number(r.unitPrice));
  const items = rows.map((r) => {
    const qty = Number(r.qty || 1);
    const unitPrice = Number(r.unitPrice || 0);
    return { name: String(r.name), spec: String(r.spec), qty, unitPrice, amount: Math.round(qty * unitPrice) };
  });
  const sum = items.reduce((a, b) => a + b.amount, 0);
  const vatMode = (values.vatMode as Quote["vatMode"]) || "separate";
  let supply = sum,
    vat = Math.round(sum * 0.1),
    total = sum + vat;
  if (vatMode === "included") {
    supply = Math.round(sum / 1.1);
    vat = sum - supply;
    total = sum;
  } else if (vatMode === "exempt") {
    vat = 0;
    total = sum;
  }
  const date = String(values.quoteDate || new Date().toISOString().slice(0, 10));
  const valid = new Date(date);
  valid.setDate(valid.getDate() + Number(values.validDays || 30));
  return {
    number: "미리보기",
    date,
    validUntil: isNaN(valid.getTime()) ? "" : valid.toISOString().slice(0, 10),
    validDays: Number(values.validDays || 30),
    delivery: String(values.delivery ?? ""),
    projectName: String(values.projectName ?? ""),
    client: { name: String(values.clientName ?? ""), contact: String(values.clientContact ?? "") },
    supplier: { name: s.businessName, owner: s.ownerName, bizRegNo: s.bizRegNo, address: s.address, phone: s.phone, email: s.email },
    items,
    vatMode,
    supply,
    vat,
    total,
    totalKorean: `일금 ${koreanAmount(total)}원정`,
    notes: String(values.notes ?? "")
      .split("\n")
      .map((l) => l.replace(/^[\s\-·*•]+/, "").trim())
      .filter(Boolean),
    message: String(values.message ?? ""),
  };
}

const vatCaption = { separate: "부가세 별도 (10% 가산)", included: "부가세 포함 금액", exempt: "면세" };

export function QuoteSheet({ quote: q, compact }: { quote: Quote; compact?: boolean }) {
  return (
    <article className={`quote-sheet ${compact ? "quote-sheet--compact" : ""}`}>
      <header className="quote-sheet__head">
        <span>QUOTATION</span>
        <h1>견 적 서</h1>
        <dl>
          <div>
            <dt>문서 번호</dt>
            <dd>{q.number}</dd>
          </div>
          <div>
            <dt>견적일</dt>
            <dd>{q.date}</dd>
          </div>
          <div>
            <dt>유효 기간</dt>
            <dd>{q.validUntil ? `${q.validUntil}까지 (${q.validDays}일)` : "-"}</dd>
          </div>
        </dl>
      </header>

      <section className="quote-sheet__parties">
        <div>
          <h4>받는 곳</h4>
          <strong>{q.client.name || "받는 곳"} 귀하</strong>
          {q.client.contact && <p>{q.client.contact}</p>}
          {q.projectName && <p>건명: {q.projectName}</p>}
          {q.delivery && <p>납기: {q.delivery}</p>}
        </div>
        <div>
          <h4>공급자</h4>
          <strong>{q.supplier.name || "상호 미입력"}</strong>
          {q.supplier.owner && <p>대표 {q.supplier.owner}</p>}
          {q.supplier.bizRegNo && <p>사업자등록번호 {q.supplier.bizRegNo}</p>}
          {q.supplier.address && <p>{q.supplier.address}</p>}
          {(q.supplier.phone || q.supplier.email) && <p>{[q.supplier.phone, q.supplier.email].filter(Boolean).join(" · ")}</p>}
        </div>
      </section>

      <div className="quote-sheet__total">
        <span>합계 금액</span>
        <strong>{q.totalKorean}</strong>
        <em>₩{n(q.total)}</em>
      </div>

      <table className="quote-sheet__table">
        <caption>견적 내역 · {vatCaption[q.vatMode]}</caption>
        <thead>
          <tr>
            <th>No</th>
            <th>항목</th>
            <th>내용</th>
            <th>수량</th>
            <th>단가</th>
            <th>금액</th>
          </tr>
        </thead>
        <tbody>
          {q.items.length === 0 && (
            <tr>
              <td colSpan={6} className="is-empty">
                항목을 입력하면 여기에 표시됩니다.
              </td>
            </tr>
          )}
          {q.items.map((it, i) => (
            <tr key={i}>
              <td>{i + 1}</td>
              <td>{it.name || "-"}</td>
              <td>{it.spec || "-"}</td>
              <td className="num">{it.qty.toLocaleString("ko-KR")}</td>
              <td className="num">{n(it.unitPrice)}</td>
              <td className="num">{n(it.amount)}</td>
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr>
            <td colSpan={5}>공급가액</td>
            <td className="num">{n(q.supply)}</td>
          </tr>
          <tr>
            <td colSpan={5}>부가세</td>
            <td className="num">{n(q.vat)}</td>
          </tr>
          <tr className="is-total">
            <td colSpan={5}>총 견적 금액</td>
            <td className="num">{n(q.total)}</td>
          </tr>
        </tfoot>
      </table>

      <section className="quote-sheet__notes">
        <h4>유의사항</h4>
        <ul>
          {(q.notes.length ? q.notes : ["본 견적은 유효 기간 내 계약 시 적용됩니다.", "범위 변경 시 금액이 조정될 수 있습니다."]).map((l) => (
            <li key={l}>{l}</li>
          ))}
        </ul>
      </section>
      {q.message && <p className="quote-sheet__message">{q.message}</p>}
      <footer className="quote-sheet__sign">
        <span>{q.date}</span>
        <strong>{q.supplier.name || "공급자"}</strong>
        {q.supplier.owner && <span>대표 {q.supplier.owner} (인)</span>}
      </footer>
    </article>
  );
}
