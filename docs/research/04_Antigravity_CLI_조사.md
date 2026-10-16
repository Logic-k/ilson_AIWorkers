# Google Antigravity CLI (`agy`) 조사 보고서 — 구독 전용 헤드리스 오케스트레이션 관점

- 조사일: 2026-10-14 / 대상: `agy` 1.3.4 (Windows, `C:\Users\kimmi\AppData\Local\agy\bin\agy.EXE`)
- 표기: **[확인]** 공식 문서·로컬 명령 출력·로컬 changelog로 직접 확인 / **[추론]** 근거로부터의 해석 / **[미확인]** 근거 없음 또는 검증 불가
- 로컬 근거: `agy --help`, `agy help <sub>`, `agy mcp add --help`, `agy models`, `agy agents`, `agy changelog`(1.0.0~1.3.4 전체, 이하 "CL x.y.z"), 바이너리 문자열 검사(환경변수명 존재 여부만), `~/.gemini/antigravity-cli/settings.json`·`~/.gemini/config/config.json`(토큰 없음 확인 후 열람). 자격증명 파일(`oauth_creds.json`, `google_accounts.json` 등)은 이름만 확인했고 열지 않음. `-p` 실행은 지시에 따라 하지 않음.
- 주의: 공식 문서 일부는 바이너리보다 오래됨(예: 헤드리스 문서의 `--print-timeout` 기본값 5m ↔ 실제 1.3.4는 `0s`=무제한). 충돌 시 로컬 help/changelog를 우선함.

---

## 0. 결정에 중요한 요약 (TL;DR)

1. **약관/정책 리스크가 최대 변수.** 공식 FAQ는 헤드리스(`agy -p`)를 "직접 스크립팅·CI용"으로 규정하고, `agy`를 *서드파티 코딩 에이전트나 애플리케이션 안에 감싸는 것*과 *계정 로테이션 자동화*는 소비자 플랜에서 "is not supported under consumer plans."라고 명시함 [확인]. 추가 약관 6조는 서드파티 소프트웨어로 서비스에 접근하는 행위를 계약 위반으로 규정 [확인]. 우리 앱(로컬 데스크톱 앱이 `agy`를 "AI 직원"으로 오케스트레이션)은 이 문구에 걸릴 소지가 있음 [추론]. 반면 포럼 답변(구글 직원 여부 불명)은 "공식 바이너리를 stdin/stdout 파이프로 호출"은 지원, "OAuth 토큰 추출/백엔드 직접 호출"은 비지원이라고 설명 [확인: 포럼, 비공식].
2. **크레딧 과금 차단**: CLI 설정 키 `useG1Credits`(기본 `false`, `/config`의 "Use AI Credits") [확인]. 단, CLI가 전역 사용자 설정에서 `useG1Credits=true`를 **상속**하는 코드 경로가 있음(바이너리 문자열 `applyUserSettings: inherited useG1Credits=true from %s`, CL 1.0.8) [확인/추론] → CLI `settings.json`에 `"useG1Credits": false`를 명시 + 데스크톱 앱의 **AI Credit Overages = Never** 둘 다 설정 권장.
3. **구독 전용 보장 env 제거 목록**: `GEMINI_API_KEY`, `GOOGLE_GEMINI_BASE_URL`, `AGY_ADC_AUTH`, `GOOGLE_CLOUD_QUOTA_PROJECT`, `GOOGLE_CLOUD_LOCATION`, `GOOGLE_APPLICATION_CREDENTIALS`, (방어적) `GOOGLE_API_KEY`, `GOOGLE_GENAI_USE_VERTEXAI`, `GOOGLE_VERTEX_BASE_URL`, `GOOGLE_CLOUD_PROJECT`, 그리고 계정 선택용 `AGY_ACCOUNT`. 추가로 settings의 `modelProvider` 키 부재와 `--project`/Business 프로젝트 미설정을 확인해야 함.
4. **권한**: 규칙 문법 `action(target)`, 리스트 `allow/ask/deny`, 우선순위 Deny > Ask > Allow [확인]. 헤드리스는 1.1.5부터 저장된 `settings.json` 정책을 따르고, 승인 불가 도구는 soft-deny(종료코드 0, `denied_actions`에 기록) [확인]. **워크스페이스 안의 `.agents/settings.json` 같은 권한 파일은 문서·바이너리 어디에도 없음** [확인: 바이너리 문자열 0건]. 프로젝트별 권한은 `~/.gemini/config/projects/<id>.json`(사용자 홈 아래) 쪽이며 전역보다 우선 [확인: CL 1.0.12]. 저장소 안에서 권한을 제어하려면 `.agents/hooks.json`의 `PreToolUse` 훅(`allow/auto_approve/deny/ask` 결정 반환)이 현실적 대안 [확인: hooks 문서, 헤드리스 동작은 미확인].
5. **커스텀 에이전트**: `.agents/agents/<name>.md` 또는 `.agents/agents/<name>/agent.md`(워크스페이스), `~/.gemini/config/agents/`(전역). YAML frontmatter + 본문=시스템 프롬프트. `--agent <name>`이 없는 이름이면 헤드리스는 종료코드 1 (CL 1.3.4) [확인].

---

## 1. 구독 플랜과 쿼터

### 1.1 어떤 플랜에 포함되나
| 플랜 | 내용 | 근거 |
|---|---|---|
| 개인 무료(Google 계정) | Plans 문서는 "Pro/Ultra가 아닌 사용자"에게 주간 갱신 쿼터 + 주간 rate limit을 준다고 기술 | [확인] https://antigravity.google/docs/plans/ |
| Google AI Pro | 5시간마다 갱신되는 높은 쿼터, 주간 한도까지. AI 크레딧 overage 사용 가능 | [확인] 같은 페이지 |
| Google AI Ultra ($100 / $200) | Pro 대비 5배 / 20배 사용량(Gemini 앱·Antigravity), 최고 주간 한도, 서드파티 모델 접근 | [확인] https://blog.google/products-and-platforms/products/google-one/google-ai-subscriptions (2026-05-19), https://support.google.com/googleone/answer/16286513 |
| Google AI Plus | 공식 문서에 Antigravity 언급 없음 | [미확인] |
| Workspace(비즈니스) 계정 | FAQ: 현재 *개인* Google 계정 대상이며 Workspace 계정이 안 되면 @gmail.com 사용 권장 | [확인] https://antigravity.google/docs/faq/ |
| 팀/기업 | Gemini Enterprise(Google Cloud 약관). "Business account" 로그인 → 라이선스(Standard/Plus 좌석, 좌석당 풀링 크레딧 월 $10/$15) 또는 Cloud 프로젝트 종량제. ADC 지원 | [확인] https://antigravity.google/docs/enterprise/ |
| BYOK/BYO endpoint로 한도 증설 | 미지원 | [확인] Plans 문서 |

- 바이너리 내부 tier 열거형: `FREE_TIER`, `G1_PRO_TIER`, `G1_ULTRA_TIER`, `AGY_BUSINESS_PAYGO_TIER`, `ENTERPRISE_AGENT_TIER`, `CS_STANDARD_TIER`, `AIDA_TIER` [확인: 문자열 존재 / 의미는 추론].
- 참고: 이 PC에서 `agy models` 출력 = `gemini-3.8-flash-{high,medium,low}`, `claude-sonnet-4-6`, `claude-opus-4-6-thinking`, `gpt-oss-120b-medium` [확인: 로컬]. Plans 문서는 서드파티 모델을 Ultra 특전으로 적지만 계정 tier를 확인하지 않았으므로 관계는 [미확인].

### 1.2 쿼터 동작
- 한도는 프롬프트 개수가 아니라 에이전트 작업량 기준, **모델별로 추적**, 용량에 따라 변동 가능 [확인: Plans].
- 블로그: 일일 프롬프트 한도 → 5시간 갱신 compute 기반 + 주간 상한, 상한 도달 시 작은 모델로 이동 [확인: Google blog 2026-05-19].
- "사용자별" 쿼터라는 표현은 문서에 명시돼 있지 않으나 개인 계정 단위로 서술됨 [추론]. 따라서 여러 "AI 직원"을 병렬로 돌리면 **같은 계정의 같은 모델 쿼터를 나눠 씀** [추론]. 포럼 답변도 헤드리스가 대화형과 같은 계정 한도를 소비한다고 설명 [확인: 비공식 포럼].
- 쿼터 소진 시 동작(CL): 개인 쿼터 도달은 재시도 없이 즉시 보고(1.3.3); 일일/결제 상한이거나 서버 retry-delay가 30초 초과면 즉시 중단(1.2.13); 크레딧 부족 시 "Your AI credits balance is too low to continue." 즉시 표시(1.2.15); 재시도 대기 최대 30초(1.3.4) [확인].

### 1.3 AI 크레딧(유료 overage)
- Pro/Ultra만 사용 가능. one.google.com/ai/credits에서 구매, 일회성 프로모션 크레딧도 포함. 기본 쿼터 소진 후 Gemini Enterprise 소비 단가로 차감 [확인: Plans].
- 데스크톱 앱 설정 **AI Credit Overages**: `Never`(크레딧 자동 사용 안 함, 갱신 대기 — "Baseline model quota reached" 알림) / `Always`(자동 전환) [확인: Plans, Google One 도움말].
- **CLI 설정 키 `useG1Credits`** (`~/.gemini/antigravity-cli/settings.json`, `/config` 또는 `/settings`에서 "Use AI Credits"/"Use G1 Credits" 토글). 레퍼런스 기본값 `false`, "external builds only" [확인: https://antigravity.google/docs/cli/credits/, https://antigravity.google/docs/cli/reference/, https://antigravity.google/docs/settings/].
- 상속 주의: CL 1.0.8 "전역 사용자 설정의 `use_ai_credits`를 시작 시 상속", 바이너리 문자열 `applyUserSettings: inherited useG1Credits=true from %s` [확인]. 명시적 `false`가 상속된 `true`를 이기는지는 [미확인] → 둘 다 꺼두고, 실행 전 `-p "/credits"`로 상태를 확인하는 방어 로직 권장 [추론].
- ADC/Cloud 프로젝트 로그인에는 "Use AI Credits" 설정이 적용되지 않음(CL 1.1.11) [확인].
- 현재 로컬 CLI `settings.json`에는 `useG1Credits` 키가 없음(키: `colorScheme`, `model`, `trustedWorkspaces`) [확인: 로컬].

### 1.4 CLI에서 잔여 쿼터 조회
- `agy -p "/usage"`(별칭 `/quota`), `/credits`, `/model`, `/effort`, `/skills`(CL 1.1.11), `/permissions`, `/hooks`, `/help`, `/changelog`, `/config`(CL 1.1.12)는 print 모드에서 **에이전트 턴 없이, 쿼터 소모 없이, 대화 기록도 남기지 않고** 한 줄당 TSV 레코드를 출력하며 `--output-format json|stream-json`이면 구조화 payload를 냄 [확인: CL].
- `/usage` 리셋 시각: 터미널 출력은 로컬 시간대, 파이프/`--json` 출력은 UTC(CL 1.2.15) [확인]. 1.3.4 help에는 전역 `--json` 플래그가 없으므로 실무적으로는 `agy -p "/usage" --output-format json` 사용 [추론]. JSON 필드 스키마는 문서에 없음 [미확인] — 실제 출력으로 스키마 고정 필요.
- stream-json **입력** 세션 안에 `/usage`·`/model`을 보내면 ERROR 결과 + 종료코드 2로 세션 종료. 별도 `agy -p` 프로세스로 실행해야 함 [확인: 헤드리스 문서].
- 공식 `/usage` 문서는 대화형 TUI 패널(Model Quotas)만 설명 [확인: https://antigravity.google/docs/cli/commands/usage/].

---

## 2. 인증 모드와 제거해야 할 환경변수

### 2.1 인증 모드
| 모드 | 활성화 방법 | 과금 | 근거 |
|---|---|---|---|
| Google 계정(기본) | 첫 실행 시 브라우저 OAuth. 토큰은 OS 키링(Windows: Credential Manager)에 저장, 이후 무음 로그인. SSH는 URL+코드 붙여넣기 | 구독 쿼터(+선택적 크레딧) | [확인] https://antigravity.google/docs/cli/install/, troubleshooting |
| Gemini API 키 | settings.json `"modelProvider": "gemini"` **와** `GEMINI_API_KEY` 둘 다 필요. env만으로는 효과 없음. `modelProvider=gemini`인데 키가 없으면 시작 실패. 헤더에 "Gemini API key" 표시, 계정 세션 없음 | Gemini API 종량제 | [확인] install 문서, CL 1.1.13 |
| ADC | `gcloud auth application-default login` 후 `AGY_ADC_AUTH=true`. 프로젝트: ADC 파일 `quota_project_id` → `GOOGLE_CLOUD_QUOTA_PROJECT` → 메타데이터 서버. 위치: `GOOGLE_CLOUD_LOCATION`(기본 global) | Cloud 프로젝트 | [확인] https://antigravity.google/docs/enterprise/ |
| Business(Gemini Enterprise) | `/login` → Business account → Google Cloud SSO 또는 WIF, 라이선스 선택 또는 `agy --project PROJECT_ID`. 프로젝트/위치가 CLI settings.json에 저장 | 좌석 라이선스 또는 종량제 | [확인] enterprise 문서 |

- 헤드리스 인증: 캐시된 자격증명 사용. 미인증 상태로 비대화형 실행 시 `authentication required` 오류로 종료(멈추지 않음) [확인: https://antigravity.google/docs/cli/headless/]. 만료 시 "Authentication expired. Please log back in." (CL 1.3.3) [확인].
- 2.0/IDE는 `~/.gemini/config/config.json`의 `userSettings.enableAdc: true`로도 ADC 활성화 [확인: enterprise 문서]. 로컬 config.json에는 해당 키 없음 [확인: 로컬].
- 포럼 답변: 디스크의 OAuth 토큰 추출은 비지원 [확인: 비공식]. 우리 앱은 토큰을 절대 읽지 않고 공식 바이너리만 호출해야 함.

### 2.2 자식 프로세스에서 제거할 env 목록
| 변수 | 효과 | 상태 |
|---|---|---|
| `GEMINI_API_KEY` | API 키 자격증명(단 `modelProvider` 필요) | [확인] 문서 |
| `GOOGLE_GEMINI_BASE_URL` | Gemini 호환 커스텀 엔드포인트로 요청 전송 | [확인] 문서 |
| `AGY_ADC_AUTH` | ADC 로그인 활성화 | [확인] 문서, CL 1.3.3 |
| `GOOGLE_CLOUD_QUOTA_PROJECT` | ADC 프로젝트 결정 | [확인] 문서 |
| `GOOGLE_CLOUD_LOCATION` | ADC 위치 | [확인] 문서 |
| `GOOGLE_APPLICATION_CREDENTIALS` | 표준 ADC 탐색 순서의 첫 단계(바이너리에 문자열 존재) | [추론] |
| `GOOGLE_API_KEY` | 문서상 "읽지 않음"이지만 바이너리에 문자열 존재 → 방어적 제거 | [확인 문서/방어] |
| `GOOGLE_GENAI_USE_VERTEXAI`, `GOOGLE_VERTEX_BASE_URL`, `GOOGLE_CLOUD_PROJECT` | 바이너리에 존재, 문서화 안 됨(Vertex 경로 가능성) | [추론] 방어적 제거 |
| `AGY_ACCOUNT` | 여러 로그인 계정 중 선택(오류 문자열 `AGY_ACCOUNT=%q matches no signed-in account`) | [확인 문자열/의미 추론]. FAQ가 계정 로테이션 자동화를 비지원으로 명시하므로 앱이 설정하지 말 것 |

- env 외 점검 항목: CLI settings.json의 `modelProvider` 키 부재, Business 프로젝트/위치 미저장, `--project` 미사용(단 Antigravity "프로젝트" 개념의 `--project`는 별개 — §4 참조. Business 문서의 `--project PROJECT_ID`는 GCP 프로젝트이므로 혼동 주의) [추론].
- 권장 구현: 블랙리스트보다 **화이트리스트**(PATH, SystemRoot, USERPROFILE, APPDATA, LOCALAPPDATA, TEMP/TMP, HOMEDRIVE/HOMEPATH, COMSPEC 등만 전달) [추론]. 운영 안정성용으로 `AGY_CLI_DISABLE_AUTO_UPDATE=true` 추가 고려 [확인: troubleshooting 문서].
- 기타 무해한 env(참고): `AGY_CLI_HIDE_LOGO`, `AGY_CLI_HIDE_ACCOUNT_INFO`, `AGY_CLI_DISABLE_ESCAPE_SEQUENCE_OPTIMIZATIONS`, `AGY_CLI_CMD_OUTPUT_PERCENTAGE`, `AGY_CLI_DISABLE_LATEX`, `AGY_CLI_MODEL_API_MAX_RETRIES`(바이너리), `PAGER/GIT_PAGER/JJ_PAGER`(에이전트가 `cat`으로 설정) [확인: CL].
- 런타임 검증 아이디어: `agy -p "/help"` 또는 `/config`의 구조화 출력, 또는 배너의 계정 표시("Gemini API key" 여부)로 모드 확인 [추론].

---

## 3. 헤드리스 자동화

### 3.1 플래그 (로컬 `agy --help` 1.3.4 기준 [확인] + 문서)
| 플래그 | 의미 | 비고 |
|---|---|---|
| `-p/--print/--prompt` | 단일 프롬프트 비대화형 실행 | `-i/--prompt-interactive`와 동시 사용 시 종료코드 2 (CL 1.3.4) |
| `--output-format text\|json\|stream-json` | 기본 text | stdout=응답, stderr=오류·진행·권한 알림 |
| `--input-format text\|stream-json` | stream-json은 stdin NDJSON 1줄당 1턴, `--output-format stream-json` 필수 | 1.1.15 추가 |
| `--json-schema` | 스키마 문자열 또는 파일 경로. stream-json에선 최종 result에만 | 1.2.14부터 루트가 `"type":"object"`가 아니거나 원시 타입명·평문이면 시작 시 종료코드 1 (문서의 "string/number 허용" 서술은 구버전) |
| `--mode accept-edits\|plan` | accept-edits: 파일 읽기/생성/치환 자동 승인(서브에이전트 상속). plan: `/plan` 접두, 읽기 전용 도구로 조사·계획 | 헤드리스에서 계획 승인은 자동 진행(CL 1.1.28). 설정 키 `agentMode` |
| `--dangerously-skip-permissions` | 모든 도구 권한 자동 승인. init의 `permission_mode`가 `always-proceed` | 1.3.2부터 `/plan` 계획 자동 승인은 하지 않음(`artifactReviewPolicy`로 별도). deny 규칙을 무시하는지 [미확인] |
| `--sandbox` | 터미널 샌드박스 강제 활성(설정 덮어씀) | §3.4 |
| `--agent <name>` | 세션 에이전트 선택 | 미존재 시 헤드리스 종료코드 1 + 목록 출력 (CL 1.3.4) |
| `--conversation <id>` / `-c/--continue` | ID로 재개 / 최근 대화 재개 | 매 호출은 새 프로세스. `--continue`는 현재 워크스페이스(상·하위 포함)의 최근 비어있지 않은 대화로 fallback (CL 1.2.1) |
| `--add-dir <dir>` | 워크스페이스에 디렉터리 추가(반복 가능) | [확인: help] |
| `--print-timeout` | 0=턴 완료까지 대기(기본 `0s`) | 1.2.6에 5분→무제한으로 변경. 만료 시 부분 출력 + stderr 경고 + **성공 종료**(CL 1.1.28). 백그라운드 작업 대기는 이 기한까지, 최대 30분 (CL 1.2.9) |
| `--model`, `--effort low\|medium\|high\|xhigh\|max` | 모델 고정/추론 강도 | 알 수 없는 모델은 폴백 없이 ERROR, 비0 종료 |
| `--project`, `--new-project` | Antigravity 프로젝트 지정/생성(기본 `default-cli-project`) | §4 |
| `--disable-slash-commands` | print 모드에서 슬래시/스킬 확장 비활성 | |
| `--log-file` | CLI 로그 경로 지정 | 기본 `~/.gemini/antigravity-cli/cli.log`(로컬에서 `log/cli-*.log` 심볼릭 링크 확인) |
| `--remote-control` | 원격 연결 생성 | ADC에선 미지원 |

### 3.2 JSON 봉투 / result 필드 [확인: 헤드리스 문서 + CL 1.1.27]
`conversation_id`, `status`, `response`, `error`(실패 시), `duration_seconds`, `num_turns`, `usage{input_tokens, output_tokens, thinking_tokens, cache_read_tokens, total_tokens}`, `structured_output`·`json_schema`(`--json-schema` 시), `denied_actions`(헤드리스에서 거부된 도구 동작 목록, CL 1.1.27). 오류 시 JSON에 부분 응답 포함(CL 1.2.10).

### 3.3 stream-json 이벤트 [확인: 헤드리스 문서]
각 줄은 `event` 필드를 가진 객체. 순서: `init` 1회 → `step_update` 0..N → `result` 1회(입력 스트림 모드에선 턴마다 result).
- **`init`**: `cwd`, `tools`(문자열 배열), `permission_mode`(`request-review` 기본 / `always-proceed`), `model`·`agent`(플래그로 지정 시), `json_schema`.
- **`step_update`**: `conversation_id`, `step_index`(0부터), `state`=`ACTIVE|DONE`, `step_type`=**`user_input` | `agent_response` | `tool` | `checkpoint`**, `tool_name`(tool), `text_delta`, `duration_seconds`, `usage`, `tool_info{name, parameters, output, error{type, message}}`, `subagent_info{subagents[{type_name, role, conversation_id, log_uri, workspace_uris}]}`. 긴 agent_response는 ACTIVE 여러 번 후 DONE.
  - `step_type`은 "closed vocabulary"(CL 1.1.8) [확인]. `state`에 `ERROR` 등이 나오는지는 문서에 없음. CL 1.2.4가 transcript.jsonl의 "terminal ERROR steps"를 언급 → 스트림에도 나올 가능성 [추론]. 미지의 값은 무시하도록 파서 작성 권장.
- **`result`**: §3.2와 동일 형태. 내용 텍스트는 `select(.event=="step_update") | .step_update.text_delta` 형태로 이벤트 이름 키 아래에 payload가 중첩됨(jq 예시 기준) [확인].
- **입력 메시지(stream-json 입력)**: `{"event":"user","message":{"content": "..." 또는 [{"type":"text","text":"..."}]}}`. text 외 블록 → ERROR(종료 1). 미지 event명 → 건너뛰고 stderr 경고. `control_request/control_response` → ERROR(종료 2). CLI 처리 슬래시 명령 → ERROR(종료 2). event 누락/잘못된 JSON → ERROR(종료 1). stdin 닫으면 현재 턴 완료 후 종료 0. `response`는 턴별, `num_turns/usage/duration_seconds`는 누적 [확인].

### 3.4 status 값과 종료코드
- **status**: `SUCCESS`, `ERROR`, `CANCELED`, `INTERRUPTED`(SIGINT 등), `INVALID`, `WAITING`, `RUNNING`(종결 상태 미도달) [확인: 헤드리스 문서]. "CANCELLED"(LL 두 개)가 아니라 `CANCELED` 철자.
- **종료코드** [확인: 문서 + CL]:
  - `0`: 성공. 권한 soft-deny, 무해한 도구 오류(CL 1.1.20), `--print-timeout` 만료(부분 출력) 포함.
  - `1`: 일반 실패/시작 오류 — 알 수 없는 모델, `--agent` 미존재, 잘못된 `--json-schema`, 스트림 입력의 잘못된 JSON/event 누락/비텍스트 블록.
  - `2`: print 모드 사용법 오류(`--print`+`--prompt-interactive`), 스트림 입력의 control_request·슬래시 명령.
  - `3`: 모델/에이전트 API 실패로 턴 종료 — stderr에 `AGY_ERROR: {...}` 한 줄(정규 status, HTTP/gRPC 코드, 재시도 가능 여부, 오류 ID) (CL 1.2.6, 1.2.10). 외부 실측에서 쿼터/API 실패 시 종료 3 보고 [확인: https://github.com/wstein/workharbor/issues/84]. 쿼터 오류의 정확한 canonical status 문자열(예: `RESOURCE_EXHAUSTED`)은 [미확인].
  - Ctrl+C 등 인터럽트: 비0 [확인: CL 1.1.28].
- 치명적 오류는 stderr에 안정적인 `error:` 마커(CL 1.1.28) [확인].

### 3.5 알려진 헤드리스 버그(오케스트레이터 설계에 영향)
- **#1044**: 턴이 SUCCESS로 끝났는데 `run_command`로 띄운 백그라운드 명령이 아직 실행 중이면 종료 시 kill됨. 1.2.17까지 재현 보고, 수정 기록 없음 [확인] https://github.com/google-antigravity/antigravity-cli/issues/1044
- **#1077**(#1044 중복 처리): `invoke_subagent`로 위임한 서브에이전트 출력이 부모 스트림에 안 올라오고 SUCCESS + "기다리는 중" 텍스트만 남음 [확인] https://github.com/google-antigravity/antigravity-cli/issues/1077
- 대응 [추론]: `status=SUCCESS`만으로 완료 판정 금지. 산출물(파일·테스트 결과)을 앱이 직접 검증, 프롬프트에 "포그라운드로 검증하고 서브에이전트를 기다린 뒤 결과를 직접 쓰라" 지시, `--conversation`으로 후속 턴 재요청.
- 감사 로그: 포럼 답변은 디스크의 `transcript.jsonl`과 `.agents/hooks.json` 라이프사이클 훅을 추천 [확인: 비공식].

### 3.6 샌드박스 (`--sandbox`, Windows 포함)
- 공통(CLI 탭): 워크스페이스 폴더와 `write_file` 허용 경로는 읽기/쓰기 마운트, `read_file` 허용 경로는 읽기 전용, 나머지는 **접근 불가**; 샌드박스 명령은 기본 **네트워크 없음**, `read_url` 허용 도메인만 아웃바운드 허용 [확인] https://antigravity.google/docs/sandbox?tab=cli
- 민감 파일(`~/.ssh`, `.env`) 차단, 임시 디렉터리·빌드 캐시 쓰기 가능, `.git`은 읽기 전용(CL 1.1.10, 1.0.9) [확인].
- **Windows**: 2.0 앱 기준 "Enable Sandbox Mode (Preview)", 향후 통합 권한 시스템으로 이전 예정. Windows 섹션은 ".env 차단"과 "네트워크가 켜져 있을 때 read_url 도메인만 허용"을 기술하고 격리 기술명은 밝히지 않음 [확인]. CLI 1.2.17부터 관리자 권한 불필요, Python/Git/npm 기본 동작; 1.3.3 MS Store판 PowerShell 7에서 Windows PowerShell로 폴백; 1.0.7 Windows 샌드박스 네트워크 프록시 수정(→ 프록시 기반 허용목록으로 추정) [확인/추론].
  - 결론: Windows CLI에서 `--sandbox`는 "워크스페이스 밖 쓰기 차단 + 비허용 경로 비가시 + 네트워크 기본 차단(허용목록)"으로 동작한다고 문서상 해석되나, Windows 특유 세부(정확한 기본 쓰기 경로, 네트워크 기본값)는 [미확인] → 실측 필요.
  - 참고: settings 문서의 IDE 섹션은 "Windows 샌드박스 미지원"이라고 써 있어 다른 문서와 충돌(구 IDE 기준으로 보임) [확인: 충돌 존재].
- 설정: `enableTerminalSandbox`(기본 false), `toolPermission`(`request-review` 기본, `proceed-in-sandbox`, `strict`, `always-proceed`) [확인]. `proceed-in-sandbox`면 샌드박스 내 명령 자동 실행, 우회 요청만 승인 필요(헤드리스에선 soft-deny될 것으로 추정 [추론]).
- `unsandboxed(...)` 규칙: 샌드박스 밖 실행 허용. 샌드박스 문서는 현행 기능으로 설명하나 CL 1.2.2는 "deprecated `unsandboxed` 규칙 → `command` 규칙으로 이전" 경고를 언급 [확인: 충돌]. 신규 설정에선 사용 자제 권장 [추론].

---

## 4. 권한(permissions)

### 4.1 문법 [확인] https://antigravity.google/docs/permissions/
- 형식 `action(target)`, 리스트 `allow`(무프롬프트) / `ask` / `deny`. 우선순위 **Deny > Ask > Allow**.
- action: `read_file`, `write_file`(워크스페이스 내는 자동 허용), `read_url`, `execute_url`, `command`, `unsandboxed`, `mcp(server/tool)`.
- `command(prefix)`는 **단어 단위 리터럴 접두 일치**, `command(regex:...)`는 정규식(토큰 `^(?:pattern)$` 고정), `*`는 전체. `$(...)`, 백틱, 중괄호 확장, 복잡한 리다이렉션이 있으면 전체 줄 정확 일치만 허용. Windows에서 단어 분리가 안 되는 명령은 `regex:` 필요(예: `command(regex:git .*)`).
- `write_file` 허용은 `read_file`도 허용, `read_file` 거부는 `write_file`도 거부. 경로는 디렉터리 재귀, Windows는 드라이브 문자 제거·`\`→`/` 정규화.
- 예: `"allow": ["command(npm test)", "command(git diff)", "command(regex:npm run (build|lint|test))", "write_file(src/)"]`, `"deny": ["command(rm -rf)", "write_file(.git/)"]`.
- 0단어 토큰 규칙(`command(time)` 등)은 아무것도 매치하지 않음(CL 1.1.11). `npm run dev` 승인은 해당 스크립트만 고정(CL 1.1.21).

### 4.2 설정 파일 위치(Windows는 `%USERPROFILE%` = `C:\Users\kimmi`)
| 범위 | 경로 | 근거 |
|---|---|---|
| CLI 전역 | `%USERPROFILE%\.gemini\antigravity-cli\settings.json` | [확인] 문서, 로컬 존재 |
| Antigravity 공유 사용자 설정 | `%USERPROFILE%\.gemini\config\config.json` (Remote Control의 "Always allow" 규칙은 이 파일에만 저장, CL 1.3.4) | [확인] CL, 로컬 존재 |
| 프로젝트별 | `%USERPROFILE%\.gemini\config\projects\<project-id>.json` — 전역보다 우선(CL 1.0.12). 로컬 파일 구조: `id`, `name`, `projectResources.resources[].gitFolder{folderUri, defaultBranch, allowWrite}` | [확인] CL, 로컬 키 구조 |
| 병합 | CLI 권한 = 프로젝트 권한 + 공유 사용자 설정 권한 + CLI settings.json 병합(CL 1.0.5). `/permissions` 패널에서 3개 구성을 편집 | [확인] CL |
| 플래그 | 명령줄 플래그는 해당 세션에서 저장값을 덮어씀 | [확인] settings 문서 |

- 프로젝트 파일 안의 권한 키 이름/형식은 문서에 없음 [미확인] (바이너리에 `persistProjectPermissions`, "Project %q config not found. Using cached permissions." 문자열 존재 [확인]). → `/permissions`의 project 탭에서 한 번 추가한 뒤 파일을 보고 형식을 고정하는 방식 권장 [추론].
- 프로젝트 선택: 플래그 없으면 `default-cli-project`; `--project=<id|name>`, `--new-project`; `--conversation`으로 재개하면 그 대화의 프로젝트 사용 [확인] https://antigravity.google/docs/projects/. → 직원(에이전트)별로 Antigravity 프로젝트를 나눠 `--project`로 지정하면 프로젝트 단위 권한 분리가 가능 [추론].

### 4.3 헤드리스가 권한을 쓰는 방식 [확인]
- 1.1.5부터 헤드리스도 저장된 `settings.json` 정책(permissions, 파일 접근, 샌드박스, 자동실행, artifact review)을 따름.
- 대화형 프롬프트 없음 → 승인 필요한 도구는 **soft-deny**: 실행은 계속, 종료 0, stderr에 필요한 allow 규칙 이름 출력(CL 1.1.3), JSON의 `denied_actions`에 기록(CL 1.1.27). 거부를 다른 명령으로 우회하지 않도록 개선(CL 1.2.15).
- 워크스페이스 내 읽기/쓰기는 자동 허용, 셸 명령은 기본 Ask, URL 읽기도 기본 Ask(CL 1.1.28).

### 4.4 워크스페이스(저장소) 안의 프로젝트 설정 파일로 명령 허용이 가능한가?
- `.agents/settings.json` 등 **워크스페이스 내 권한 설정 파일은 문서화돼 있지 않고, 바이너리에도 `.agents/settings.json`·`.agents/config.json`·`.agents/permissions` 문자열이 없음** [확인]. 따라서 "저장소에 파일을 두어 특정 명령을 허용"하는 공식 경로는 없음.
- 워크스페이스 `.agents/`가 인식하는 것: `skills/`, `agents/`, `hooks.json`, `mcp_config.json`, `skills.json`, `rules.json`, `agents.json`, `plugins.json`(cwd~프로젝트 루트 사이 모든 `.agents/`에서 로드, CL 1.2.16) [확인: 문서/CL].
- 대안: `.agents/hooks.json`의 `PreToolUse` 훅이 `decision`으로 `allow`, `auto_approve`, `deny`, `ask`, `force_ask`, `deny_unless_prior_grant`를 반환 가능하고 `permissionOverrides`도 지원 [확인] https://antigravity.google/docs/hooks/. 워크스페이스 훅은 **폴더 신뢰(trust) 후** 로드(CL 1.1.1) [확인]. 헤드리스에서 훅이 실행되는지는 문서에 없음 [미확인]이나 1.1.17에서 실행 경로가 단일화됨 [추론: 실행될 가능성 높음] → 실측 필요.
- 워크스페이스 신뢰 목록은 CLI settings.json의 `trustedWorkspaces` 배열에 저장됨 [확인: 로컬 키]. 헤드리스에서 미신뢰 폴더의 동작은 [미확인] → 앱이 작업 폴더를 미리 신뢰 목록에 넣는 방식 검토(단, 사용자 동의 하에) [추론].

---

## 5. 커스텀 에이전트 / 서브에이전트

### 5.1 위치 [확인] https://antigravity.google/docs/subagents, https://antigravity.google/docs/cli/commands/agents/
- 워크스페이스: `.agents/agents/<name>.md` 또는 `.agents/agents/<name>/agent.md` (저장소 루트)
- 전역: `~/.gemini/config/agents/<name>.md` 또는 `.../agents/<name>/agent.md` (설정 루트에 직접 둔 파일은 탐색 안 됨)
- 플러그인: `plugins/<plugin>/agents/`, 플러그인 루트 `agents.json`(CL 1.3.3)
- 대화 중 `define_subagent` 도구로 임시 정의(마크다운으로 기록). 커뮤니티는 런타임 정의 직후 호출 실패를 보고 → 정적 파일 권장 [확인: 비공식].
- `agents.json`(및 `skills.json`/`rules.json`/`plugins.json`) 매니페스트: 디렉터리 항목은 직속 항목만 로드, 중첩 항목은 `{"path": "shared_skills", "include_only": ["category/my-skill"]}`처럼 지정(CL 1.2.10) [확인]. 최상위 스키마 전체는 [미확인].

### 5.2 frontmatter 필드
| 필드 | 타입/기본값 | 출처 |
|---|---|---|
| `name` | string, 필수 | 문서 |
| `description` | string, 필수(플래너의 위임 판단 근거) | 문서 |
| `tools` | string[], 기본 `[]`. **오타·미매핑 도구명은 서브에이전트 hang 유발 가능** | 문서 |
| `mainAgent` | bool, 기본 true(주 에이전트로 선택 가능) | 문서 |
| `subagent` | bool, 기본 true(`invoke_subagent` 대상) | 문서, CL 1.1.5 |
| `model` | `inherit`(기본) / `flash` / `pro` | 문서, CL 1.1.5 |
| `commandExecutionPolicy` | `off` / `auto` / `eager` / `sandbox`(기본) | 문서 |
| `mcpServers` | object[] | 문서 |
| `skills`, `plugins` | string[] (에이전트 파일 기준 상대경로 허용, CL 1.3.1) | 문서 |
| `rules` | 규칙 파일 직접 지정(지정 시 항상 적용) | CL 1.1.15 |
| `agents` | 의존 서브에이전트 목록(예: `[child.md]`) | CL 1.1.27 |
| `hooks` | 훅 경로 | CL 1.3.1 |
| `hidden`, `inheritMcp` | bool | CL 1.1.6 |
| `excludeDefaultComponents` | true면 기본 프롬프트 섹션·내장 도구 제외 | CL 1.2.1 |

- 마크다운 에이전트는 기본적으로 주변 skills/rules/subagents를 상속(CL 1.1.25). `tools`에 `invoke_subagent`가 있으면 서브에이전트 목록·사용법이 시스템 프롬프트에 자동 주입(CL 1.2.5) [확인].
- 확인된 도구명 예: `view_file`, `grep_search`, `code_search`, `run_command`, `command_status`, `write_to_file`, `replace_file_content`, `multi_replace_file_content`, `invoke_subagent`, `define_subagent`, `manage_subagents`, `send_message`, `manage_task` [확인: 문서/CL]. `tools` 생략 시 기본 도구 세트를 받는지 [미확인] → `init.tools`로 실측 검증 권장.

### 5.3 선택과 실행
- `--agent <name>`(1.1.1 추가)로 주 에이전트 지정; `agy agents`로 목록. 이름 불일치 시 헤드리스 종료 1 + 목록(1.3.4). 워크스페이스 `.agents/agents/`가 헤드리스·`--agent`에서 인식 안 되던 버그는 1.2.11에 수정 [확인]. 매칭 키는 frontmatter `name`으로 추정 [추론].
- 1.3.4 로컬: `agy agents`/`agy models`에 `--output-format`을 주면 "flags provided but not defined" 오류 (CL 1.1.12의 기술과 불일치) [확인: 로컬]. `E:\AIMAX_오마주`에서 `agy agents` 출력은 비어 있었음 [확인: 로컬].

### 5.4 서브에이전트 워크스페이스 모드
- `inherit`(부모 워크스페이스), `branch`(격리된 Git worktree), `share`(공유 디렉터리 저장소) [확인: 문서].
- branch worktree는 앱 데이터 디렉터리의 `worktrees/` 아래(CL 1.2.10; 이전엔 `.system_generated/worktrees`), 다중 저장소 워크스페이스에선 저장소별 격리 복사본(CL 1.3.3), kill 시 임시 worktree 제거 [확인].
- 누가 모드를 고르는지, `invoke_subagent`에 어떻게 전달하는지, 결과 병합 방식은 문서에 없음 [미확인] (부모가 서브에이전트 worktree에 전체 접근 가능하다고만 기술). 모델이 도구 인자로 선택하는 것으로 추정 [추론].
- 서브에이전트는 부모 대화 이력을 상속하지 않음, 병렬 실행 가능, 상태 `running/done/error/killed` [확인].

### 5.5 최소 예제 (`<repo>\.agents\agents\backend-dev.md`)
```markdown
---
name: backend-dev
description: Implements and tests backend changes in this repository. Use for API and database tasks.
tools:
  - view_file
  - grep_search
  - run_command
  - write_to_file
  - replace_file_content
model: inherit
mainAgent: true
subagent: true
commandExecutionPolicy: sandbox
---
# System Prompt
You are the backend engineer of a small software company.
Work only inside this repository, run the test suite before finishing,
and finish with a short summary of the files you changed.
```
실행: `agy -p "Implement ticket #12" --agent backend-dev --output-format stream-json --mode accept-edits --sandbox` [추론: 조합은 문서·help 기반, 실측 필요].

---

## 6. 약관 / 정책

- **FAQ(공식)** https://antigravity.google/docs/faq/ [확인]
  - 헤드리스 모드는 직접 스크립팅과 CI 워크플로용으로 의도됨.
  - `agy`를 서드파티 코딩 에이전트나 **애플리케이션** 안에 감싸는 것, 계정 로테이션 자동화는 소비자 플랜에서 비지원 — "is not supported under consumer plans."
  - 서드파티 소프트웨어(Claude Code, OpenClaw, OpenCode 등)로 Antigravity 로그인을 쓰는 것은 ToS 위반이며 계정 정지/해지 가능. 서드파티 에이전트에서 Gemini를 쓰려면 Gemini Enterprise나 AI Studio API 키 권장.
- **Antigravity 추가 약관** https://antigravity.google/terms [확인]
  - 6조: 서비스 남용·방해 금지; 서드파티 소프트웨어·도구·서비스로 서비스에 접근하는 것은 위반(예시: OpenClaw + Antigravity OAuth); 구글이 제공하지 않은 제품과 연계 사용도 포함된다고 기술. 위반 시 Antigravity 및/또는 Gemini CLI 계정 정지·해지 가능.
  - Gemini Enterprise/Workspace/Enterprise API 키로 접근하는 사용자는 이 추가 약관 대신 관리자 측 약관 적용.
  - Google 서비스 약관·개인정보처리방침이 함께 적용. 서드파티 모델(예: Anthropic)을 주 에이전트로 쓰면 해당 모델 약관(Anthropic Commercial Terms)도 적용.
  - 계정 공유·재판매·rate limit은 추가 약관에 명시 없음 [확인: 언급 없음]. 다만 Google 계정은 개인 계정이고 FAQ가 계정 로테이션을 비지원으로 명시하므로, 계정 공유·다중 계정 순환·타인에게 사용량 재판매는 위험 [추론].
- **집행 사례**: 포럼에 OpenClaw/OpenCode 사용 후 403 "service has been disabled ... for violation of Terms of Service" 정지 사례 다수 [확인: 커뮤니티].
- **포럼 Q&A(2026-09-15)**: 로컬 오케스트레이터가 공식 `agy`를 헤드리스 워커로 띄우는 것은 "fully supported", 대화형과 동일한 계정 한도 소모, 토큰 추출·백엔드 직접 호출은 비지원이라고 답변. 답변자가 구글 직원으로 표시되지 않음 → 공식 입장 아님 [확인] https://discuss.ai.google.dev/t/is-external-orchestration-of-antigravity-cli-headless-mode-supported-with-account-based-usage/183051
- **해석** [추론]
  - 가장 안전한 형태: 계정 소유자 본인이 자기 PC에서 자기 작업을 위해 공식 바이너리를 스크립트로 호출(= FAQ의 "direct scripting"에 가까움), 토큰 미접촉, 단일 계정, 적정 동시성.
  - 위험한 형태: 앱을 타인에게 배포해 그들의 구독으로 돌리게 하는 상용 제품, "애플리케이션 내 wrapping"으로 보이는 UX, 다중 계정 로테이션, 쿼터 회피 목적의 대량 병렬.
  - "not supported"는 문언상 "금지"보다 약하지만, 약관 6조와 결합하면 계정 정지 리스크를 배제할 수 없음. 배포형 제품이라면 Gemini Enterprise(관리자 약관 적용) 경로 또는 구글의 서면 확인을 받는 것이 바람직.

---

## 7. 대안 엔진(참고, 한 줄씩)

- **Gemini CLI**: `gemini -p "..." --output-format stream-json`(이벤트 `init/message/tool_use/tool_result/error/result`, 종료코드 0/1/42/53), 자동 승인 `--yolo`/`--approval-mode`. **단, 무료·Google AI Pro·Ultra의 Google 로그인은 2026-06-18부로 서비스 중단**(Code Assist Standard/Enterprise, 유료 API 키만 유지) → 구독 전용 엔진으로는 부적합 [확인] https://developers.googleblog.com/en/an-important-update-transitioning-gemini-cli-to-antigravity-cli/ , https://geminicli.com/docs/cli/headless
- **Claude Code**: `claude -p "..." --output-format stream-json --verbose [--include-partial-messages] [--input-format stream-json] [--permission-mode acceptEdits|dontAsk|auto] [--allowedTools ...] [--resume <id>]`. Pro/Max 로그인으로 동작(단 `--bare`는 OAuth/키체인을 읽지 않으므로 API 키 필요). 정책: 구독 OAuth는 Claude Code 등 네이티브 앱의 통상적 사용용, Pro/Max 한도는 통상적 개인 사용 가정, 개발자가 Claude.ai 로그인을 자기 앱에 제공하거나 사용자 대신 구독 자격증명으로 요청을 라우팅하는 것은 불허, 단 최종 사용자가 수정되지 않은 바이너리에 자기 구독으로 로그인하는 것은 허용 [확인] https://code.claude.com/docs/en/headless , https://code.claude.com/docs/en/legal-and-compliance . 제거 env: `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_USE_BEDROCK/VERTEX` 등 [추론].
- **Codex CLI**: `codex exec --json --sandbox workspace-write [-o last.txt] [--output-schema schema.json]`, 재개 `codex exec resume --last|<id>`; 이벤트 `thread.started/turn.started/turn.completed/turn.failed/item.*/error`. 기본적으로 저장된 CLI 인증(ChatGPT 로그인 포함) 재사용, `CODEX_API_KEY`/`OPENAI_API_KEY`는 제거 대상 [확인] https://learn.chatgpt.com/docs/non-interactive-mode (구 developers.openai.com/codex/noninteractive)

---

## 8. 오케스트레이터 구현 체크리스트 [추론]
1. 자식 env 화이트리스트 + §2.2 변수 강제 제거, `AGY_CLI_DISABLE_AUTO_UPDATE=true`.
2. 실행 전 점검: CLI settings.json에 `modelProvider` 없음, `"useG1Credits": false` 명시, 데스크톱 앱 AI Credit Overages = Never; `agy -p "/usage" --output-format json`, `agy -p "/credits" --output-format json`으로 잔여 쿼터·크레딧 확인(쿼터 소모 없음).
3. 실행: `agy -p <prompt> --output-format stream-json --agent <name> --mode accept-edits --sandbox [--project <id>] [--add-dir ...]`, 필요 시 `--print-timeout`(만료는 성공 종료이므로 앱이 별도 판정).
4. 파싱: `event` ∈ {init, step_update, result}; `step_type` ∈ {user_input, agent_response, tool, checkpoint}; 미지 값 허용. 종료코드 3이면 stderr의 `AGY_ERROR:` JSON을 파싱해 쿼터/재시도 판단.
5. 완료 판정은 `status=SUCCESS`가 아니라 산출물 검증 + `denied_actions` 비어 있음 확인(#1044/#1077).
6. 권한은 `~/.gemini/antigravity-cli/settings.json` 또는 프로젝트별 설정의 `permissions.allow`로 최소 허용(`command(npm test)` 등), `--dangerously-skip-permissions`는 지양. 저장소 단위 제어가 필요하면 `.agents/hooks.json` PreToolUse 훅(헤드리스 동작 실측 필요).
7. 동시 실행 수 제한(같은 계정 쿼터 공유), 계정 전환(`AGY_ACCOUNT`) 금지, 토큰 파일 접근 금지.

## 9. 미확인 항목(실측 필요)
- `/usage`·`/credits` JSON 출력 스키마; 명시적 `useG1Credits:false`가 상속값을 이기는지.
- 프로젝트 파일(`~/.gemini/config/projects/<id>.json`)의 권한 키 형식.
- 헤드리스에서 워크스페이스 훅 실행 여부, 미신뢰 워크스페이스에서의 동작.
- Windows `--sandbox`의 기본 네트워크 정책과 쓰기 가능 경로 목록.
- `--dangerously-skip-permissions`가 deny 규칙을 무시하는지, step `state`에 ACTIVE/DONE 외 값이 오는지, 쿼터 오류의 canonical status 문자열.
- `tools` 생략 시 기본 도구 세트 부여 여부, 서브에이전트 workspace 모드 지정 방법.
