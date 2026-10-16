import { useStore } from "../store";
import { Avatar, Modal } from "./ui";

export function ResumeDialog() {
  const { resumeOf, showResume, employee, newTask } = useStore();
  const e = resumeOf ? employee(resumeOf) : undefined;
  if (!e) return null;
  const r = e.resume;
  return (
    <Modal
      title={`${e.name}의 입사지원서`}
      description="이름과 이력은 역할을 쉽게 기억하도록 만든 가상의 AI 직원 설정입니다."
      onClose={() => showResume(null)}
      footer={
        <button
          className="btn btn--primary"
          onClick={() => {
            showResume(null);
            newTask(e.id);
          }}
        >
          {e.name}에게 업무 맡기기
        </button>
      }
    >
      <article className="resume">
        <header className="resume__head">
          <span>일손 AI 직원 채용 서류</span>
          <h3>입 사 지 원 서</h3>
        </header>
        <section className="resume__identity">
          <Avatar employee={e} size={96} />
          <dl>
            <div><dt>성명</dt><dd>{e.name}</dd></div>
            <div><dt>지원 분야</dt><dd>{e.role}</dd></div>
            <div><dt>사번</dt><dd>{r.employeeNo}</dd></div>
            <div><dt>경력</dt><dd>{r.experience}</dd></div>
            <div><dt>소속</dt><dd>{e.team}</dd></div>
            <div><dt>출신</dt><dd>{r.hometown}</dd></div>
            <div><dt>전 직장</dt><dd>{r.formerRole}</dd></div>
          </dl>
        </section>
        <blockquote className="resume__motto">“{e.motto}”</blockquote>
        <section>
          <h4>자기소개</h4>
          <p>{r.intro}</p>
        </section>
        <div className="resume__cols">
          <section>
            <h4>경력 사항</h4>
            <ol className="resume__career">
              {r.career.map((c) => (
                <li key={c.period}>
                  <span>{c.period}</span>
                  <strong>{c.org}</strong>
                  <small>{c.note}</small>
                </li>
              ))}
            </ol>
          </section>
          <section>
            <h4>보유 기술</h4>
            <ul className="resume__skills">
              {r.skills.map((s) => (
                <li key={s.label}>
                  <span>{s.label}</span>
                  <i aria-label={`${s.score}점`}>
                    {[1, 2, 3, 4, 5].map((n) => (
                      <b key={n} className={n <= s.score ? "on" : ""} />
                    ))}
                  </i>
                </li>
              ))}
            </ul>
          </section>
        </div>
        <section className="resume__ref">
          <h4>추천사</h4>
          <p>“{r.reference.quote}”</p>
          <small>— {r.reference.from}</small>
        </section>
        <section className="resume__note">
          <h4>면접 메모</h4>
          <p>{r.interviewNote}</p>
        </section>
        <footer className="resume__sign">위 기재 사항은 사실과 다름없음을 확인합니다. · 지원자 {e.name}</footer>
      </article>
    </Modal>
  );
}
