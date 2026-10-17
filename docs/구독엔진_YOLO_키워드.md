# 구독 엔진 연결과 YOLO 키워드 조사

- 조사일: 2026-10-14 · 대상 PC: Windows 11
- 질문: API 키 없이 **구독만으로** 여러 코딩 에이전트를 한 폴더·한 업무에서 직원처럼 돌릴 수 있는가? 확인 없이 끝까지 돌리는 "YOLO" 옵션은 무엇인가?

## 1. 결론

**가능합니다.** 이 PC에 설치된 구독형 CLI 중 Antigravity(`agy`)와 Codex(`codex`)는 사장님 구독 로그인 상태로 헤드리스 실행과 실시간 JSON 출력을 지원하고, 앱 코드 경로로 실제 실행까지 확인했습니다. Claude Code(`claude`)도 같은 방식을 지원하지만 이 PC의 CLI가 로그인되어 있지 않아 실행 검증은 하지 못했습니다(출력 해석은 공식 형식으로 단위 테스트).

일손은 업무마다 git 작업 공간(worktree) 하나를 만들고, 단계마다 **담당 직원의 구독 엔진**으로 실행해 다음 직원에게 넘깁니다. 예: 설계는 태오(Claude Code) → 화면은 유나(Antigravity) → 서버는 건우(Codex) → 테스트는 소희(Codex) → 리뷰는 민혁(Claude Code). 모든 단계가 같은 폴더의 커밋을 이어받습니다.

## 2. 엔진별 정리

| 엔진 | 구독 | 헤드리스 실행 | 실시간 출력 | 로그인 확인(사용량 없음) | 앱 자동 실행 | 이 PC 상태 |
|---|---|---|---|---|---|---|
| Antigravity CLI `agy` 1.3.4 | Google AI (무료·Pro·Ultra) | `agy -p "<지시>"` | `--output-format stream-json` (init / step_update / result) | `agy models` | 지원 · **검증함** | 로그인됨 |
| Claude Code `claude` 2.1.280 | Claude Pro/Max | `claude -p` (지시는 표준입력) | `--output-format stream-json --verbose` (system / assistant / user / result) | `claude auth status` (JSON) | 지원 · 형식 테스트만 | **로그인 안 됨** |
| Codex CLI `codex` 0.160.0 | ChatGPT Plus/Pro | `codex exec -` (지시는 표준입력) | `--json` (thread.started / item.* / turn.completed) | `codex login status` | 지원 · **검증함** | ChatGPT 로그인됨 |
| OpenCode `opencode` 1.18.32 | MIT 오픈소스 (DeepSeek 무료 탑재) | `opencode run --pure -f json` (지시는 표준입력) | `--format json` (step_start / message / step_finish) | `opencode models` | 지원 · **검증함** | DeepSeek 무료 사용 가능 |
| Kiro CLI `kiro-cli` 2.24.1 | Kiro | `kiro-cli chat --no-interactive` | `--output-format stream-json` (ACP 이벤트) | `kiro-cli whoami` | 안전 모드만 | 로그인됨, **실행 시 계정 일시 정지 응답** |
| Gemini CLI `gemini` 0.42.0 | — | `gemini -p` | `-o stream-json` | — | 미지원 | 개인 구글 계정 로그인이 2026-06-18에 끝나 API 키 없이는 쓸 수 없음 |

### API 키를 절대 쓰지 않게 하는 장치

- 실행할 때 자식 프로세스 환경에서 `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `GOOGLE_APPLICATION_CREDENTIALS`, `AGY_ADC_AUTH`, `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `OPENAI_API_KEY`, `CODEX_API_KEY` 등을 지웁니다.
- 로그인 방식이 API 키로 확인되면(`claude auth status`의 authMethod, `codex login status`의 "API key") 그 엔진을 "로그인 필요"로 막습니다.
- 앱은 로그인 정보·토큰 파일을 읽지 않습니다. 각 CLI가 사용자 로그인으로 직접 동작합니다.

## 3. YOLO 키워드 (권한 단계별 실제 옵션)

| 단계 | 의미 | Antigravity `agy` | Claude Code `claude` | Codex `codex exec` | OpenCode `opencode` |
|---|---|---|---|---|---|
| 읽기 | 설계·리뷰. 고치기·명령 거부 | (옵션 없음: 헤드리스가 자동 거부) | `--permission-mode dontAsk --allowedTools Read,Grep,Glob,LS,WebSearch,WebFetch` | `-s read-only` | `OPENCODE_PERMISSION='{"edit":"deny","bash":"deny"}'` |
| 편집 | 파일 수정만 자동 승인 | `--mode accept-edits` | `--permission-mode acceptEdits --permission-prompts none` | `-s workspace-write` | `OPENCODE_PERMISSION='{"edit":"allow","bash":"deny"}'` |
| 샌드박스 실행 | 수정 + 샌드박스 안 명령 | `--mode accept-edits --sandbox --dangerously-skip-permissions` | `--permission-mode acceptEdits --permission-prompts none --allowedTools Bash` (Windows 샌드박스 없음) | `-s workspace-write` | `OPENCODE_PERMISSION='{"edit":"allow","bash":"ask"}'` |
| **YOLO** | 모든 확인 생략 | `--dangerously-skip-permissions` | `--dangerously-skip-permissions` (= `--permission-mode bypassPermissions`) | `--dangerously-bypass-approvals-and-sandbox` | `OPENCODE_PERMISSION='{"edit":"allow","bash":"allow"}'` |

참고로 다른 CLI의 YOLO 키워드:

| CLI | YOLO / 자동 승인 키워드 |
|---|---|
| Gemini CLI | `-y` / `--yolo`, `--approval-mode yolo` (단계: `default`, `auto_edit`, `yolo`, `plan`) |
| Kiro CLI | `-a` / `--trust-all-tools`, 일부만: `--trust-tools=fs_read,fs_write` |
| OpenCode | `opencode run --auto` |
| Codex (대화형) | `codex --yolo`는 `--dangerously-bypass-approvals-and-sandbox`의 별칭 (exec 도움말에는 정식 이름만 표시) |
| Claude Code (대화형) | `Shift+Tab`으로 권한 모드 순환, 시작 시 `--permission-mode` |

### 앱이 고르는 방식

- 설계(태오)·리뷰(민혁)는 항상 **읽기**.
- 개발·테스트는 업무 양식의 "명령 실행 허용"을 켜면 **샌드박스 실행**, 끄면 **편집**.
- 설정의 **YOLO 모드**를 켜면 파일을 고치는 모든 단계를 **YOLO**로 실행합니다. 이때도 작업은 별도 worktree에서 하고, 원래 저장소 병합은 사장님 확인을 받습니다.

## 4. 직원들이 일을 주고받는 방식

- **자동 모드:** 사장님이 "자동 실행 허락"한 엔진만 앱이 직접 실행합니다. 팀 업무는 설계 → 화면 → 서버 → 테스트 → 리뷰 → (필요하면) 리뷰 반영 → 병합 확인 순서로 진행되고, 각 단계는 담당 직원의 엔진으로 돌아갑니다. 앞 단계의 설계서·리뷰·바뀐 파일 목록이 다음 직원의 지시에 들어가고, 단계마다 커밋해 같은 작업 공간을 이어받습니다. 사무실 화면에서는 단계가 바뀔 때 앞 직원이 서류를 들고 다음 직원 자리로 걸어갑니다.
- 허락하지 않은 엔진을 쓰는 직원 차례가 오면 그 단계만 **안전 모드**로 바뀌어, 사장님께 실행을 부탁하는 카드를 띄웁니다.
- **안전 모드(기본):** 앱이 지시서(`.ilson/TASK.md`)와 에이전트 정의를 준비하면 사장님이 각 CLI에서 직접 실행합니다. Antigravity는 `.agents/agents/ilson-*.md` 사용자 정의 에이전트, Claude Code는 `.claude/agents/ilson-*.md` 서브에이전트를 써서 리더가 팀원을 부릅니다. Codex·Kiro는 서브에이전트가 없어 한 직원이 역할을 이어서 맡습니다. 직원은 `.ilson/status-이름.txt`와 `.ilson/report-이름.md`를 남기고, 앱은 이 파일과 git 변경을 지켜보다 다음 단계로 넘깁니다.
- "한 세션"의 뜻: 각 CLI의 대화 세션은 단계마다 새로 시작합니다. 대신 같은 작업 공간·커밋·보고서로 맥락을 이어 갑니다(Orca가 여러 에이전트를 같은 worktree에서 돌리는 방식과 같습니다).

## 5. 약관 주의 (자동 실행)

- **Antigravity:** 공식 FAQ는 `agy -p`를 직접 스크립팅·CI용으로 설명하지만, 개인(소비자) 요금제에서 agy를 다른 **애플리케이션 안에 감싸는 것**은 "not supported"라고 적고 있습니다. 추가 약관 6조는 서드파티 소프트웨어를 통한 접근을 정지 사유로 둡니다.
- **Claude Code:** Anthropic은 승인 없이 제3자 제품이 claude.ai 로그인·사용 한도를 제공하는 것을 허용하지 않습니다. 사장님 PC의 Claude Code를 실행만 하는 형태도 회색 지대입니다.
- **Codex:** ChatGPT 로그인으로 `codex exec` 비대화 실행을 공식 지원합니다. 계정 공유·재판매는 금지입니다.
- 그래서 앱은 **엔진마다 따로** 위험 안내에 동의해야 자동 실행을 켭니다. 기본값은 안전 모드입니다.
- 이번 조사 중 Kiro CLI로 최소 시험을 한 번 보냈을 때 서버가 "User ID가 비정상 활동으로 일시 정지됐다"고 응답했습니다. 이 한 번의 요청이 원인일 가능성은 낮지만, Kiro 계정 상태를 직접 확인하시기 바랍니다.

## 6. 이번에 검증한 것

| 항목 | 결과 |
|---|---|
| agy 헤드리스 `stream-json` 이벤트와 파일 편집 도구 이벤트 | 확인 (실제 실행) |
| agy 사용자 정의 에이전트 + 서브에이전트 리뷰 | 확인. `tools`의 `command_status`는 에이전트 등록부에 없는 이름이라 제외함 |
| Codex `exec --json` 이벤트, 앱 코드 경로(`.cmd` + 표준입력 + 한글·특수문자) | 확인 (실제 실행) |
| agy 앱 코드 경로 | 확인 (실제 실행) |
| 안전 모드: Antigravity(유나) → 병합, Codex(건우) → 병합을 같은 저장소에서 연속 진행 | 확인. 커밋이 직원 이름과 함께 남음 |
| 엔진별 출력 해석기와 YOLO 옵션 | 단위 테스트 6개 통과 |
| 자동 모드 다중 엔진 팀 파이프라인 | **미검증.** 엔진별 자동 실행 동의는 사장님이 직접 해야 해서 대신 켜지 않음 |
| Claude Code 실제 실행 | **미검증** (CLI 로그인 필요) |
