import { useStore } from "../store";

const faqs = [
  ["API 키가 필요한가요?", "아니요. 이 앱은 API 키를 받지도, 쓰지도 않습니다. Antigravity·Claude Code·Codex·Kiro CLI에 사장님 구독 계정으로 로그인해 두면 그 구독 안에서 일합니다. API 키로 로그인된 CLI는 쓰지 않고, 실행할 때 API 키 환경 변수도 지웁니다. 견적서와 파일 정리는 AI 없이 이 PC에서 처리합니다."],
  ["안전 모드와 자동 모드는 무엇이 다른가요?", "안전 모드(기본)에서는 앱이 지시서와 에이전트 정의를 준비하고 사장님이 각 CLI에서 직접 실행합니다. 자동 모드에서는 사장님이 엔진별로 ‘자동 실행 허락’을 한 엔진만 앱이 직접 실행해, 직원들이 단계마다 일을 주고받으며 끝까지 진행합니다. Google·Anthropic 약관상 앱이 구독 CLI를 감싸 쓰는 방식에는 계정 위험이 있어 엔진마다 따로 동의를 받습니다."],
  ["안전 모드에서는 어떻게 실행하나요?", "업무 화면의 실행 카드에서 Antigravity 직원이면 ‘Antigravity 열기’, Claude Code·Codex·Kiro 직원이면 ‘터미널 열기’를 누르고 ‘터미널 명령 복사’로 붙여 넣으면 됩니다. 그다음 ‘실행했어요’를 누르면 앱이 보고서 파일이 생길 때까지 지켜보다가 다음 단계로 넘어갑니다."],
  ["직원마다 다른 구독을 쓸 수 있나요?", "네. 연결 및 설정의 ‘직원별 담당 구독’에서 고르면 됩니다. 예를 들어 설계는 Claude Code, 화면은 Antigravity, 테스트는 Codex로 맡기면 같은 작업 공간에서 차례로 일하고 커밋을 이어받습니다."],
  ["YOLO 모드는 무엇인가요?", "자동 모드에서 파일을 고치는 단계의 모든 확인을 생략하는 모드입니다(agy·claude는 --dangerously-skip-permissions, codex는 --dangerously-bypass-approvals-and-sandbox). 작업은 별도 작업 공간에서 하고 병합은 여전히 사장님 확인을 받지만, 명령이 샌드박스 없이 실행될 수 있습니다."],
  ["코드는 언제 내 저장소에 들어가나요?", "개발팀은 업무마다 앱 데이터 폴더의 별도 작업 공간(git worktree)에서 일합니다. 사장님이 변경 내용을 보고 ‘병합하기’를 눌러야만 원래 브랜치에 합쳐지고, ‘작업 공간 지우기’를 누르면 흔적 없이 정리됩니다."],
  ["구독 한도를 넘으면 돈이 나가나요?", "Antigravity 설정에서 AI 크레딧 초과 사용을 켜 두었다면 크레딧이 쓰일 수 있습니다. 구독 범위 안에서만 쓰려면 Antigravity의 ‘AI Credit Overages’를 Never로, CLI의 Use AI Credits를 끈 상태로 두세요."],
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
      <section className="card card--quiet">
        <h3>이 앱에 대해</h3>
        <p>
          일손 v{boot?.version} — ‘이름 있는 AI 직원에게 업무를 맡기는’ 방식을 참고해 처음부터 새로 만든 AI 코딩 회사 데스크톱 앱입니다. Google Antigravity와 제휴하지 않은 독립 앱입니다. 특정 서비스와 제휴하거나 그 코드·이미지를 사용하지 않았으며, 직원 이름과 이력은 모두 가상입니다.
        </p>
      </section>
    </div>
  );
}
