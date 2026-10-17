# 일손 (ilson) 디자인 규율 — DESIGN.md

외부 UI 프레임워크(Tailwind·shadcn)를 쓰지 않고 `src/styles.css` 단일 토큰 체계와 이 문서의 규율만으로 일관된 화면을 유지합니다. 새 컴포넌트를 만들거나 기존 화면을 고칠 때 아래 9개 항목을 체크리스트로 확인합니다.

---

## 1. 토큰 단일 소스 (Single Source of Tokens)

색·간격·반경·그림자·모션·글꼴은 오직 `src/styles.css`의 `:root`에서만 정의합니다.

- **간격 (4pt 배수)**: `--space-1` (4px) · `--space-2` (8px) · `--space-3` (12px) · `--space-4` (16px) · `--space-5` (20px) · `--space-6` (24px) · `--space-7` (28px) · `--space-8` (32px)
- **반경**: `--radius-sm` (8px) · `--radius` (12px) · `--radius-lg` (16px)
- **모션**: `--dur-fast` (`120ms`) · `--dur` (`200ms`) · `--ease` (`cubic-bezier(0.22, 1, 0.36, 1)`)
- **글꼴**:
  - `--font-sans`: `"Pretendard Variable", "Pretendard", "Malgun Gothic", system-ui, sans-serif`
  - `--font-mono`: `ui-monospace, "Cascadia Mono", Consolas, monospace`

## 2. 상태 세트 하나 (Unified Status Tokens)

업무 배지·직원 아바타 점·사무실 발밑 상태 링·사용량 및 예산 게이지가 모두 **같은 변수**를 씁니다.

| 토큰 | 의미 | 대표 색상 |
|---|---|---|
| `--status-running` | 진행 중 (일하는 중) | 파랑 (`#1d4ed8` / `#2563eb`) |
| `--status-waiting` | 사장님 차례 (승인·핸드오프 대기) | 주황 (`#d97706`) |
| `--status-paused` | 멈춤 (일시정지·예산 상한 대기) | 앰버 (`#b45309`) |
| `--status-failed` | 문제 발생·실패 | 빨강 (`#b91c1c`) |
| `--status-done` | 완료 | 초록 (`#1f7a4c`) |
| `--status-idle` | 대기 중 | 회색 (`#8a857c`) |
| `--status-over` | 예산·한도 초과 (80%↑ 주황, 100% 빨강) | `#d97706` → `#b91c1c` |

기존 `--ok / --warn / --bad / --info`는 위 상태 토큰의 별칭으로 유지합니다.

## 3. 기계값은 Monospace

토큰 수, 금액(₩·$), 시각·소요 시간, 커밋 해시, 업무 ID, 모델 ID, CLI 플래그·환경 변수는 항상 `font-family: var(--font-mono)`와 `font-variant-numeric: tabular-nums`로 표시합니다.

## 4. 용어 하나 (Consistent Vocabulary)

| 쓰는 말 | 쓰지 않는 말 |
|---|---|
| **업무** | 작업, 태스크, 잡(Job) |
| **직원** | 에이전트, 봇, 워커(화면 노출 시) |
| **구독 엔진** | 프로바이더, 백엔드, LLM |
| **환산 비용 / 실제 결제** | 예상 요금, API 청구액 |
| **사장님 차례** | 승인 대기, 펜딩 게이트 |

## 5. 버튼은 동작명

버튼 라벨은 클릭했을 때 일어나는 구체적인 동작을 적습니다.
- 좋은 예: `병합하기`, `실행했어요 · 다음으로`, `업무 맡기기`, `지금 확인`, `환율 새로 고침`
- 금지: `확인`, `OK`, `예`, `Submit`

## 6. 빈 상태는 다음 할 일을 안내

목록이나 표가 비어 있을 때 단순히 "없음"만 적지 않고, 다음에 무엇을 하면 채워지는지 한 문장으로 안내합니다.
- 예: *"첫 업무를 맡기면 진행 상황과 결과물이 여기에 쌓입니다."*
- 예: *"자동 모드로 업무를 실행하면 단계별 토큰과 환산 비용이 여기에 기록됩니다."*

## 7. 대기 상태의 테이크오버 (Takeover on Waiting)

사장님 차례(`awaiting_approval`)인 업무 카드는 카드 전체에 주황 테두리(`--status-waiting`)와 연한 배경을 주고, 주변의 부차적인 강조를 줄여 사장님이 무엇을 결정해야 하는지 한눈에 보이게 합니다.

## 8. 접근성 (Accessibility)

- `@media (prefers-reduced-motion: reduce)` 환경에서는 사무실 캐릭터 걷기·깜빡임 애니메이션을 끄고 즉시 위치 이동·고정 표시로 바꿉니다.
- 텍스트 없는 아이콘 버튼에는 반드시 `aria-label`을 붙입니다.
- 키보드 포커스 링(`:focus-visible`)을 제거하지 않습니다.

## 9. 아트·에셋 크레딧 명시

앱에 포함되는 폰트·아이콘·사운드·그래픽 에셋(CC0 포함)의 출처와 라이선스는 도움말(`HelpPage`)의 **크레딧** 섹션에 명시합니다.
