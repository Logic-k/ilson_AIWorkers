import { useStore } from "../store";

const faqs = [
  ["API 키가 필요한가요?", "아니요. 이 앱은 API 키를 받지도, 쓰지도 않습니다. OpenCode 오픈소스 엔진을 사용하며, 기본 탑재된 DeepSeek Flash 무료 모델을 통해 별도 API 키나 결제 없이도 즉시 업무를 자동화합니다. 견적서와 파일 정리는 AI 없이 이 PC에서 처리합니다."],
  ["안전 모드와 자동 모드는 무엇이 다른가요?", "안전 모드(기본)에서는 앱이 지시서와 에이전트 정의를 준비하고 사장님이 터미널에서 OpenCode를 직접 실행합니다. 자동 모드에서는 사장님이 ‘자동 실행 허락’을 한 경우 앱이 OpenCode를 백그라운드에서 직접 실행해, 직원들이 단계마다 일을 주고받으며 끝까지 진행합니다."],
  ["안전 모드에서는 어떻게 실행하나요?", "업무 화면의 실행 카드에서 ‘터미널 열기’를 누르고 ‘터미널 명령 복사’로 OpenCode 실행 명령을 붙여 넣으면 됩니다. 그다음 ‘실행했어요’를 누르면 앱이 보고서 파일이 생길 때까지 지켜보다가 다음 단계로 넘어갑니다."],
  ["직원마다 모델을 바꿀 수 있나요?", "네. 연결 및 설정에서 기본 DeepSeek 무료 모델 외에도 OpenCode에서 지원하는 다양한 모델을 지정하여 사용할 수 있습니다."],
  ["YOLO 모드는 무엇인가요?", "자동 모드에서 파일을 고치는 단계의 모든 확인을 생략하는 모드입니다. 작업은 별도 작업 공간(git worktree)에서 하고 병합은 여전히 사장님 확인을 받지만, 명령이 샌드박스 없이 실행될 수 있습니다."],
  ["코드는 언제 내 저장소에 들어가나요?", "개발팀은 업무마다 앱 데이터 폴더의 별도 작업 공간(git worktree)에서 일합니다. 사장님이 변경 내용을 보고 ‘병합하기’를 눌러야만 원래 브랜치에 합쳐지고, ‘작업 공간 지우기’를 누르면 흔적 없이 정리됩니다."],
  ["비용이 드나요?", "기본 모델(DeepSeek V4.1 Flash Free)은 완전 무료(0원)입니다. OpenCode에 유료 공급자 API 키나 OpenCode Zen 크레딧을 연동하여 사용하는 경우에만 해당 서비스의 요금이 발생합니다."],
  ["OpenCode는 무엇인가요?", "OpenCode는 MIT 라이선스 오픈소스 코딩 에이전트 CLI입니다. 로그인이나 유료 결제 없이도 DeepSeek V4.1 Flash 무료 모델로 즉시 코딩 및 텍스트 작업을 수행할 수 있습니다. 일손은 OpenCode 실행 시 --pure 격리 옵션을 사용해 플러그인 충돌 없이 안전하게 동작하며, 안전 모드에서는 .opencode/agents/ilson-*.md 에이전트 정의와 실행 지시서를 준비해 드립니다."],
  ["블로그에 자동으로 올려 주나요?", "아니요. 자동 발행·자동 이웃 신청은 플랫폼 약관과 계정 정지 위험이 있어 하지 않습니다. 초안의 ‘본문 서식 복사’를 눌러 블로그 편집기에 붙여 넣으세요."],
  ["중간에 앱을 꺼도 되나요?", "됩니다. 단계마다 결과를 저장해 두기 때문에, 다시 켜면 업무가 ‘멈춤’으로 보이고 이어하기를 누르면 끝낸 단계 다음부터 진행합니다."],
];

export function HelpPage() {
  const { boot } = useStore();
  return (
    <div className="page help">
      <section className="card">
        <span className="eyebrow">일하는 방식</span>
        <h2>맡기면, 사장님은 실행과 결정만 하시면 됩니다.</h2>
        <ol className="how">
          <li>
            <strong>01 · 맡길 일 고르기</strong>
            <p>직원을 고르고 양식을 채웁니다. 최근 설정을 불러와 반복 업무를 빠르게 맡길 수 있습니다.</p>
          </li>
          <li>
            <strong>02 · 직원이 처리</strong>
            <p>단계별로 일하고, 끝낸 단계는 바로 저장합니다. 진행 상황은 업무 화면에서 실시간으로 보입니다.</p>
          </li>
          <li>
            <strong>03 · 사장님이 결정</strong>
            <p>비용이 드는 단계, 파일을 바꾸는 단계 앞에서 멈춰 확인을 받습니다. 결과물은 복사·저장·인쇄할 수 있습니다.</p>
          </li>
        </ol>
      </section>
      <section className="principles">
        <article className="card">
          <span className="eyebrow">준비물</span>
          <h3>필요한 것은 미리 알려 드립니다</h3>
          <p>API 키나 사업장 정보가 필요한 직원은 ‘설정 필요’로 표시하고, 준비가 끝나기 전에는 업무를 받지 않습니다.</p>
        </article>
        <article className="card">
          <span className="eyebrow">병합</span>
          <h3>내 저장소는 확인 후에</h3>
          <p>코드는 별도 작업 공간에서 만들고, 변경 내용을 보여 드린 뒤 사장님이 병합을 눌러야만 반영합니다.</p>
        </article>
        <article className="card">
          <span className="eyebrow">실수</span>
          <h3>멈춰도 다시 이어갑니다</h3>
          <p>어디까지 했는지 기록해 두어 처음부터 다시 하지 않습니다. 반복되는 오류는 자동으로 막아 둡니다.</p>
        </article>
      </section>
      <section className="card">
        <h3>자주 묻는 질문</h3>
        <div className="faq">
          {faqs.map(([q, a]) => (
            <details key={q}>
              <summary>{q}</summary>
              <p>{a}</p>
            </details>
          ))}
        </div>
      </section>

      <section className="card">
        <span className="eyebrow">크레딧 & 라이선스</span>
        <h3>앱에 사용된 오픈소스 및 에셋</h3>
        <ul style={{ paddingLeft: 20, margin: "10px 0", display: "grid", gap: 6, fontSize: 13, color: "var(--ink-2)" }}>
          <li>
            <strong>Pretendard Variable</strong> — 본문 및 제목 글꼴 (SIL Open Font License 1.1)
          </li>
          <li>
            <strong>Lucide Icons</strong> — 인터페이스 아이콘 (ISC License)
          </li>
          <li>
            <strong>OpenCode</strong> — MIT 라이선스 오픈소스 CLI 엔진
          </li>
          <li>
            <strong>models.dev</strong> — 표준 LLM 토큰 단가 기준 데이터
          </li>
          <li>
            <strong>한국수출입은행</strong> — 원/달러 기준 환율 제공 API
          </li>
        </ul>
      </section>

      <section className="card card--quiet">
        <h3>이 앱에 대해</h3>
        <p>
          일손 v{boot?.version} — ‘이름 있는 AI 직원에게 업무를 맡기는’ 방식을 참고해 처음부터 새로 만든 AI 코딩 회사 데스크톱 앱입니다. OpenCode 오픈소스 엔진을 기반으로 동작하는 독립 앱이며, 직원 이름과 이력은 모두 가상입니다.
        </p>
      </section>
    </div>
  );
}
