# AIMAX Web Console (ops app) — 기능/계약/라이프사이클 모델

분석 대상 (모두 데이터로만 취급, 코드 복제 아님):
- `inline_0.js` (10,272 lines, vanilla JS, 프레임워크 없음) — 아래에서 `JS:L<n>`
- `body.html` (1,515 lines) — `HTML:L<n>`
- `style_0.css` (3,476 lines), `style_1.css` (견적서 인쇄 팝업용 18 lines) — `CSS:L<n>`

전 파일을 처음부터 끝까지 읽었다. 주의할 점:
- 이것은 **클라이언트가 소비하는 시점의 계약**이다. 서버가 실제로 더 많은 필드를 보낼 수 있다.
- JS가 참조하지만 마크업에는 없는 ID가 많다 (`#yunmiTopic`, `#yunmiJobResult`, `#yunmiGenerationMode`, `#onboardingGuideProgress`, `#employeeList`, `#setupChecklist`, `#downloadAgentBtn`, `#launchAgentBtn`, `#agentStatus`, `#productPills`, `#recentJobs` 등). 모두 `?.`나 null 검사로 보호되어 있어서, **제거된 예전 UI의 흔적**이다 (예전 온보딩 가이드, 대시보드 실행기 카드, 윤미 네이티브 폼). `#researchTab` 패널은 있지만 사이드바에 버튼이 없다 (HTML:L225 vs L47-54).
- 코드 주석의 날짜는 2026-05 ~ 2026-09이다 (예: JS:L11, L512, L563, L614).

---

## 1. 전체 UI

### 1.1 셸
- 두 개의 `<main>`이 있다: `#loginView` (HTML:L10-33)와 `#appView.shell` (HTML:L35). `setView(loggedIn)`이 둘 중 하나를 보이게 한다 (JS:L2448).
- 셸 그리드: 사이드바 292px | 본문. 사이드바는 76px로 접을 수 있고 접힘 상태는 localStorage `aimax_sidebar_collapsed`에 저장된다 (CSS:L41-52, JS:L9, L8545-8559). 900px 이하에서는 한 열로 쌓인다 (CSS:L3091).
- 사이드바 nav는 6개 탭이다. 버튼마다 `data-tab`, `data-title`, `data-short`가 있다 (HTML:L48-53).

| data-tab | 제목 | 패널 |
|---|---|---|
| overview | 대시보드 | `#overviewTab` |
| staff | 직원 채용 | `#staffTab` |
| jobs | 직원 업무지시 | `#jobsTab` |
| feedback | 직원 피드백 | `#feedbackTab` |
| settings | 설정 | `#settingsTab` |
| updates | 업데이트 및 오류보고 | `#updatesTab` |

- 탭을 바꾸면 `.tab-panel`을 숨기고 `#{tab}Tab`을 보이며, 탭마다 정해진 부제를 설정한다 (JS:L10245-10268).
- 사이드바 하단에는 사용자 이메일과 로그아웃 버튼이 있다 (HTML:L55-58).
- 상단 바 (sticky): 페이지 제목/부제 + "새로고침" 버튼 (HTML:L62-70).
- 상단 바 아래의 전역 배너 (HTML:L72-107):
  - `#passwordNotice` (첫 로그인 비밀번호 변경)
  - `#globalUpdateNotice` (실행기 업데이트. 다운로드 / 자세히 / 다시 보지 않기)
  - `#pendingReportsNotice` (조치가 필요한 오류 보고)
  - `#jobGuardNotice` (작업 차단 + "조치했어요, 다시 시도")
- 모달 (HTML:L1291-1511): 직원 이력서 `#staffResumePrompt`, SmartScreen 안내, Mac 호환성, Intel Mac 미지원, 설치 가이드, 필수 업데이트, 서비스 공지, 오류 보고 확인 요청, 웹 시크릿 이전 안내. 토스트는 `#toast`이며 3.2초 동안 보인다 (JS:L1048).

### 1.2 로그인 흐름
1. 화면 문구는 "구매자 이메일과 임시 비밀번호로 접속" (HTML:L20).
2. `POST /api/auth/login`에 `{email, password, device_label: navigator.platform}`을 보내고 `session_token`을 받는다 (JS:L9042-9066).
3. 토큰은 localStorage `aimax_session_token`에 저장한다. 실패하면 sessionStorage, 그것도 안 되면 메모리에 둔다 (JS:L142-160). 메모리 모드이면 새로고침 시 다시 로그인해야 한다는 토스트를 띄운다.
4. 모든 요청에 `authorization: Bearer <token>`, `content-type: application/json`, `cache: no-store`를 붙인다. 응답이 non-2xx이면 `Error`를 던지는데, `status`와 `body`를 함께 싣는다. 에러 코드는 `body.error`에 있다 (JS:L2419-2446).
5. `boot()` (JS:L8983-9007)는 플랫폼 감지(macos/windows)와 Mac 아키텍처 감지(WebGL renderer, UA-CH)를 한다. 이어서 웹 설정을 로드하고, 토큰이 있으면 `refreshAll()`을 부른다. 실패하면 토큰을 폐기하고 로그인 화면으로 간다.
6. `me.requires_password_change`가 true이면 비밀번호 패널이 보인다. 이때 현재 비밀번호 칸은 숨겨지고(첫 변경에는 필요 없음), 업무지시와 다운로드는 모두 막힌다 (JS:L4542-4547, L3252, L3389-3392).

### 1.3 로그인 직후 사용자가 보는 것 (대시보드)
- "AIMAX는 우리 회사의 일을 직원 단위로 나눠 맡기는 AI 사무실" 스토리 카드가 있다. 아바타 4명(예리/현주/송이/윤미)과 상태 스트립(키워드 조사 / 글쓰기 / 고객 찾기 / 결과 기록)이 함께 나온다 (HTML:L129-148).
- "AIMAX Brain" 미리보기 카드에는 자동재생 mp4/webm 영상이 있다 (HTML:L149-163, JS:L2454).
- **Next-action 카드**: `nextAction()`이 우선순위대로 하나의 할 일을 계산한다 (JS:L4361-4442).
  1. 비밀번호 변경
  2. 필수 업데이트
  3. 실행기 연결 (로컬 직원 권한이 있고 미연결일 때)
  4. 번들 split 불일치 → 통합 실행기 재설치
  5. 직원별 차단 사유 → 설정 열기
  6. 사용 가능 직원 없음
  7. 설치형 직원만 있음
  8. "일을 맡길 수 있습니다"
- 팝업 우선순위: 필수 업데이트 > 오류보고 확인 요청 > 서비스 공지(1회) > 웹 시크릿 이전 공지(1회). 각 `shouldShow*`가 상위 팝업이 보이는지 검사한다 (JS:L3899-4030).

### 1.4 탭별 내용
- **직원 채용** (HTML:L177-223, JS:L5573-5623)
  - 통계 4개: 전체 / 사용 가능 / 설정 필요 / 권한 없음.
  - 실행 방식 필터: 전체 / 웹 / PC 설치형 / 모바일·토스. 상태 필터와 검색창이 있다.
  - 카드 그리드는 `auto-fill minmax(260px)`이다. 카드를 클릭하면 **이력서 모달**이 열린다 (JS:L5711-5807). 모달 구성은 다음과 같다.
    - 3열: 기본정보 / 기술(점 5개) / 업무성향 | 자기소개 / 경력 | 근무환경(실행 옵션) / 채용조건 / 추천사 / 면접 한마디 / 근무기록.
    - 큰 액션 버튼에는 "AIMAX WORK ORDER" / "PARTNER SERVICE" / "STATUS HOLD" 서브라벨이 붙는다.
- **직원 업무지시** (HTML:L361-912)
  - 상단에 직원 선택 카드가 2열로 놓인다 (`#jobEmployeeSwitch`, JS:L5544).
  - 그 아래에 직원별 폼이 하나만 보인다. 폼마다 `data-job-kind`가 있다: `yeri_write`, `hyunju_find`, `yunmi_script`, `sangsu_quote`, `songi_research`, `semu_tax_invoice`.
  - 각 폼은 좌측 입력(300-430px) + 우측 "작업 미리보기" 레이아웃이다 (CSS:L921).
  - 하단에는 "일 맡긴 기록" 테이블이 있다. 열은 작업 / 직원 / 상태 / 비용 / 생성 / 최근 로그 / 중단이다 (JS:L6981-7057).
- **직원 피드백**: 직원 선택, 만족도 1-5, 좋았던 점, 개선 요청, "운영팀 답변 필요" 체크 (HTML:L1215-1286).
- **설정** (HTML:L914-1084)
  - 로컬 설정 체크리스트 (span-8) + 실행기 카드 (연결 / 마지막 연결 / 버전 / "로컬 설정 열기").
  - 글쓰기 스타일 카드, 웹 작업 설정 (AI 모델, 블로그 프로필, 서로이웃 멘트 카드).
  - AI/API 연결 (Gemini, Apify, YouTube, OpenAI, Claude) + "기존 실행기 키 가져오기".
  - API 키 발급 가이드 링크.
- **업데이트 및 오류보고** (HTML:L1086-1213)
  - 현재 / 최신 / 필수 최소 버전, 브라우저 환경.
  - 추천 설치 파일 + 다운로드, 배포 기준 설명, 내 권한의 설치 파일 테이블.
  - 오류 보고 폼, 수집 항목 설명, 내 문의/피드백 목록.

---

## 2. API 계약 (클라이언트가 소비하는 형태)

공통 규칙:
- base는 동일 오리진(`api.aimax.ai.kr`)이다.
- 에러 body는 `{error: <code>, message?}`이고, 코드는 `errorMessage()`가 한국어 문구로 바꾼다 (JS:L2337-2412).
- 유료 작업은 body에 **`confirm_paid: true`가 반드시 있어야 한다**. 없으면 `*_paid_confirmation_required`가 반환된다 (JS:L2365-2366).

### 2.1 Auth
| Method/Path | Request | Response (소비 필드) | 용도 / 근거 |
|---|---|---|---|
| POST `/api/auth/login` | `{email, password, device_label}` | `session_token` | 로그인 (JS:L9046) |
| GET `/api/auth/me` | – | `user{email, status, entitlements{products[], product}, account_segment}`, `can_execute`, `requires_password_change` | 권한/상태의 근원 (JS:L8187, L2462-2512, L2687-2700) |
| POST `/api/auth/logout` | `{}` | – | 로그아웃 후 watcher를 정지하고 토큰을 폐기 (JS:L9845) |
| POST `/api/auth/change-password` | `{current_password, new_password}` | – | 10자 이상이어야 하고, 이메일 local-part를 포함하면 안 된다. 에러: `invalid_current_password`, `password_too_short`, `password_contains_email` (JS:L9792, L2340-2344) |

`account_segment` 정규화 결과는 `makefamily_member` / `member_and_buyer` / `test` / `operator` / `paid_buyer`(기본)이다. 한국어 입력값도 받는다: 원회원, 수강생, 운영자 등 (JS:L2687-2694).

### 2.2 Jobs
| Method/Path | Request | Response | 근거 |
|---|---|---|---|
| GET `/api/jobs` | – | `jobs[]` (필드는 §3) | JS:L8144, L8190 |
| POST `/api/jobs` | `{kind, payload, target_platform?, target_device_label?, server_generation?, confirm_paid?}` | `job` | JS:L8589-8634 |
| POST `/api/jobs/{id}/cancel` | `{reason:"user_cancelled"}` | – | JS:L8641 |
| POST `/api/jobs/guard/acknowledge` | `{job_kind}` | – | JS:L9028 |

POST `/api/jobs`의 에러 처리:
- 409 + `guard_paused` 또는 `runner_update_required` → guard 배너를 띄운다.
- 401/403 → "로그인이 만료되었거나 권한이 없습니다".
- 응답 메시지가 없거나 `HTTP xxx`이면 → 네트워크 안내 문구.

### 2.3 Agent / Downloads / Version
| Method/Path | Request | Response | 근거 |
|---|---|---|---|
| GET `/api/agent/status?platform=` | – | `agent{connected, status, version, platform, device_label, last_seen_at, readiness{...}, diagnostics{bundle_split_mismatch}, version_info{...}}` | JS:L8189, L982-1038, L3361-3386 |
| POST `/api/agent/commands` | `{type, platform, payload?}` | `command{id}` | JS:L7476, L8435 |
| GET `/api/agent/commands/{id}` | – | `command{status, assigned_at, logs[{message}], result{...}}` | JS:L8416 |
| GET `/api/version?current=&platform=` | – | `agent{current_version, latest_version, min_version, update_required, update_available, release_notes, platform}` | JS:L8197. `agent.version_info`가 있으면 그쪽을 우선한다 (JS:L2217-2221) |
| GET `/api/downloads/options` | – | `downloads[{platform, product, exists, label, filename, size, url}]` | JS:L8203, L3708-3719, L4689-4711 |
| POST `/api/downloads/tickets` | `{platform, product}` | `url`, `download{filename}` | 단기 서명 URL로 추정 (JS:L8363, L5684) |
| GET `/api/downloads/agent?platform=&product=` | – | 파일 | 실행 옵션 URL로만 등장한다. 클라이언트는 이 경로를 보면 티켓 발급으로 바꿔서 연다 (JS:L5682-5694) |

### 2.4 Workers
| GET `/api/workers` | – | `workers[]`, `job_kinds[]` | JS:L8214, L2518-2627 (필드는 §5) |
|---|---|---|---|

### 2.5 Reports
| Method/Path | Request | Response | 근거 |
|---|---|---|---|
| POST `/api/reports` | 오류: `{source:"aimax-webapp", user_input{work_context, visible_error, user_note}, web_context{...}, system{...}}`. 피드백: `{source:"staff_feedback", report_kind:"feedback", feedback{...}, user_input, ...env}` | `report_id, status, status_label, public_message, next_update_message, status_updated_at, stored_at` | JS:L7902-7989 |
| GET `/api/reports/mine` | – | `reports[]` | JS:L7226 |
| POST `/api/reports/{id}/response` | `{response:"resolved"\|"still_failing", note}` | `report` | JS:L8112 |

### 2.6 Research (송이)
| Method/Path | Request | Response | 근거 |
|---|---|---|---|
| GET `/api/research/projects` | – | `projects[]` | JS:L7235 |
| POST `/api/research/projects` | `{name, industry, goal, instagram_profile_url?, content_category?, content_topic?}` | `project` | JS:L7534, L9650 |
| PATCH `/api/research/projects/{id}` | 위 필드 일부 | `project` | JS:L7551 |
| DELETE `/api/research/projects/{id}` | – | – | 소속 item도 함께 삭제된다 (JS:L7564, L9438) |
| POST `/api/research/projects/{id}/profile/apify` | `{confirm_paid:true, instagram_profile_url}` | `project` (에러 body에도 `project`가 실림) | JS:L7803 |
| GET `/api/research/items` | – | `items[]` | JS:L7240 |
| POST `/api/research/items` | `{project_id, url?, title?, source_text?, category?, tags?(쉼표 문자열), instagram_profile_url?, content_category?, content_topic?}` | `item` | 동기적으로 링크를 읽고 요약한다 (JS:L7577, L10189) |
| PATCH `/api/research/items/{id}` | `{memo, category, tags}` | `item` | JS:L7660 |
| POST `/api/research/items/{id}/apify` | `{confirm_paid:true}` | `item` (에러 body에도 `item`이 실림) | JS:L7826 |
| POST `/api/research/items/{id}/ai-analysis` | `{confirm_paid:true, model, include_video:true}` | `item` | JS:L7845 |
| POST `/api/research/items/{id}/yunmi-script` | `{}` | `job` (kind=yunmi_script) | JS:L7705 |
| GET `/api/research/brief?project_id=` | – | `brief` (문자열. 클립보드로 복사한다) | JS:L7678 |
| GET `/api/research/discovery?project_id=` | – | `runs[]`, `candidates[]` | JS:L7244, L7619 |
| POST `/api/research/discovery/search` | `{project_id, platform, keyword, sort_mode, date_range_days, max_results}` | `run`, `candidates[]` | JS:L7591, L9513 |
| POST `/api/research/discovery/candidates/{id}/import` | `{}` | `item, candidate` | JS:L7636 |
| GET/POST `/api/research/discovery/subscriptions` | POST: `{project_id, keyword, platform, frequency, sort_mode, date_range_days, max_results}` | `subscriptions[]` | JS:L7256, L9593 |
| PATCH/DELETE `/api/research/discovery/subscriptions/{id}` | PATCH: `{action:"pause"\|"resume"}` | – | JS:L5997, L6014 |
| GET `/api/research/integrations` | – | `integrations{gemini, apify, youtube, media_tools}` | JS:L7274 (§6) |

### 2.7 Tax (세무 직원)
| Method/Path | Request | Response | 근거 |
|---|---|---|---|
| GET `/api/tax/settings` | – | `server_configured`, `settings{business_registered, member_connected, corp_num, corp_name, ceo_name}`. 503 `tax_not_configured`이면 서버 미구성, 403이면 권한 없음 | JS:L5069-5095 |
| PUT `/api/tax/settings` | `{corp_num, corp_name, ceo_name}` | `settings`. 에러 `member_join_fields_required` | JS:L5102-5129 |
| GET `/api/tax/invoices` | – | `invoices[]` | JS:L5089 |
| POST `/api/tax/invoices` | 초안 payload (§7) | `invoice{id}` | JS:L5131 |
| POST `/api/tax/invoices/{id}/preflight` | `{}` | `unit_cost, balance, can_issue, checked_at` | JS:L5143 |
| POST `/api/tax/invoices/{id}/issue` | `{confirm_paid:true}` | – | JS:L5165 |
| POST `/api/tax/invoices/{id}/sync` | `{}` | – | JS:L5185 |

### 2.8 User 설정
| Method/Path | Request | Response | 근거 |
|---|---|---|---|
| GET `/api/user/secrets` | – | `secrets{providers{<p>{configured, source:"web_user"\|"server_global", web_configured, server_configured, updated_at}}}` | 값은 절대 내려오지 않는다 (JS:L7389, L3070-3087) |
| PUT `/api/user/secrets/{provider}` | `{value}` | `secrets`, `verification{verified, username, plan}` | 저장 시 키를 검증한다. username/plan은 Apify 응답 형태로 보인다 (JS:L7408-7428) |
| DELETE `/api/user/secrets/{provider}` | – | `secrets` | JS:L7436 |
| GET `/api/user/writing-style` | – | `style`, `options[{key,label,hint}]` | JS:L7377 |
| POST `/api/user/writing-style` | `{style}` | – | JS:L7359 |

### 2.9 Launch 티켓
| POST `/api/scriptbot/launch` | `{}` | `url`, `key_source` (`"none"`이면 Gemini 키 안내 노출), `cost_notice` | 윤미 iframe. `url + ?embed=1`로 연다 (JS:L5504-5542, HTML:L548-551) |
|---|---|---|---|
| POST `/api/eunseo/launch` | `{}` | `url` | 은서 웹 프롬프터 (JS:L2829-2833, L5675-5680) |

---

## 3. Jobs 라이프사이클

### 3.1 Job kind 전체 목록
| kind | 직원 | 실행 위치 | 서버 job 생성 여부 |
|---|---|---|---|
| `yeri_write` | 예리 블로그 글쓰기 | 로컬 실행기 (Naver SmartEditor). 서버 글 생성 fallback이 있다 | 예 (JS:L238, L9225-9263) |
| `hyunju_find` | 현주 영업사원 (서로이웃 신청) | 로컬 실행기 | 예 (JS:L243, L9265-9298) |
| `yunmi_script` | 윤미 숏폼 스크립트 | 웹(서버). 현재는 iframe scriptbot이 주 경로 | 예 (legacy 폼, `/research/items/{id}/yunmi-script`) (JS:L248, L9300-9344) |
| `songi_research` | 송이 자료조사 | 웹(서버) research API | **아니다**. research item을 job row처럼 합성해서 보여준다 (JS:L1911-1929) |
| `sangsu_quote` | 상수 경리(견적서) | 브라우저 내부만 | 아니다 (localStorage) (JS:L258, HTML:L560) |
| `semu_tax_invoice` | 세무 직원 전자세금계산서 | 웹(서버) tax API | job이 아니라 별도 tax API를 쓴다 (JS:L810) |

이 외의 kind는 `/api/workers`의 `job_kinds[]`에서 동적으로 추가될 수 있다. 카탈로그에 없는 kind는 클라이언트에서 삭제한다 (JS:L2527-2534, L2607-2626). 서버측 worker 코드명의 예는 `yunmi_script_writer`이다 (JS:L498).

### 3.2 Job payload
**`yeri_write`** (JS:L9240-9258):
- `keywords[]`: 쉼표로 분리하며, 키워드마다 글 1편이 된다.
- `mode`: publish / save / schedule.
- `style_id?`: consult / info / review.
- `ai_model`, `image_model`.
- `word_count`: 300 / 800 / 1500 / 2500.
- `image_count`: 0-8. UI는 0-6을 제공한다.
- `category?`, `cta_link?`, `cta_text?`.
- `schedule_date?`, `schedule_hour?` (0-23), `schedule_interval` (1-72시간, 문자열).
- `seo_research_enabled`, `keyword_emphasis_enabled`, `style_reference_text?`.
- `seo_reference_text?`, `seo_reference_urls[]`: 최대 5개 (JS:L4813-4822).

**`hyunju_find`** (JS:L9285-9293):
- `keywords[]`, `target_mode` (keyword / blogger_followers), `blogger_url`.
- `max_per_keyword` (1-50), `speed_mode` (safe / normal / fast).
- `messages[]?`: 최대 20개, 각 220자 (JS:L1954-1963). 비우면 로컬 앱에 저장된 멘트를 쓴다.

**`yunmi_script`** (legacy, JS:L1223-1237, L9300-9344):
- `topic, platform:"릴스/쇼츠/틱톡 공통", format:"shortform", duration:"60초", tone:"friendly_expert", objective, reference_url, reference_text, cta`.
- `mode`: `no_paid_alpha` 또는 `ai_beta`. `ai_beta`일 때 `ai_model, request_id(UUID), confirm_paid`가 붙는다.

비웹 kind에는 body 최상위에 `target_platform`, `target_device_label`이 붙는다. 실행할 PC 장치를 지정하는 용도이다 (JS:L8561-8568).

### 3.3 Job 객체 필드 (소비되는 것, JS:L942-979, L1352-1442, L5394-5449)
- 기본: `id, kind, status, label, worker_label, worker_code, staff_code, created_at, updated_at, assigned_at, finished_at, failed_reason, failed_stage, diagnostic`.
- `logs[{at, level, message}]`: 테이블에는 마지막 로그만 표시한다.
- `result`:
  - 공통: `ok, success, total, stage, failed_keyword, error, visible_error, user_message`.
  - `usage{input_tokens, output_tokens, billable_output_tokens}`.
  - `cost{total_won, text_won, image_won, estimated_total_won, model, provider}`.
  - `images{generated, attempted, inserted, failure_count, failures[{stage, error_code, message, local_image_path}], soft_failure_accepted, image_skipped_no_key, mode_overridden_to_save, local_paths[], items[{local_image_path}]}`.
  - `posts[{keyword, source, title, char_count, target_char_count, status, error, images, recovery_markdown_path, recovery_manifest_path}]`, `failed_posts[]`.
  - `recovery_markdown_path`, `recovery_manifest_path`: 실패해도 원고/복구 파일을 **로컬 경로**로 보존한다는 뜻이다.
  - `diagnostic{code,title,message,user_actionable,admin_action_required,action_label,actions[]}`.
  - 윤미 전용: `result.yunmi | result` 안에 `mode (no_paid_alpha / paid_ready_mock / ai_generated), summary, variants[{code, title, angle, target_label, target_audience, target_strategy, target_reason, duration, title_candidates[], hook, hook_method{label,sample,reason}, rows[{time, screen|visual, dialogue|line, subtitles|caption}], anchor_bridges[{time,anchor,bridge}], shooting_guide{visual, audio_tone, energy, expression_gesture, cut_points}, cta_candidates{comment, follow, save}, script}], hook_options[], patterns[{label,insight}], final_recommendation{text, criteria[]}, paid_call{request_id, idempotency_key, provider, model, executed, status, auto_retry}, copy_text`.

### 3.4 상태
- live 상태 (폴링 대상, 중단 가능): `queued`, `generating`, `ready_for_publish`, `running` (JS:L1931-1937).
- 종료 상태: `done`, `failed`, `cancelled`, `error` (JS:L1380).
- 상태 pill 라벨: 대기 / 진행 중 / 완료 / 실패 / 취소 (JS:L3721-3731).
- 송이 합성 row의 상태는 item에서 유도한다. 링크/AI 실패 → failed, Apify 미수집 → queued, AI queued/running → running, 그 외 → done (JS:L1876-1883).

### 3.5 실패 단계 (stage) 와 진단
- stage 목록: `content_generation, browser_start, naver_login, smart_editor_open/title/font/input/save/publish/schedule, image_generation, image_upload, image_insert_verification, image_insert_exception, image_prompt_empty, image_completion, init` (JS:L1444-1486).
- stage가 비어 있으면 텍스트에서 추론한다. 예: `localhost:8669`이나 chromedriver → browser_start, 429/quota → content_generation (JS:L1506-1518).
- 진단 코드와 사용자 조치 가능 여부 (JS:L1543-1598):

| 코드 | 사용자 조치 | 비고 |
|---|---|---|
| `api_key_missing` | 가능 | |
| `api_key_invalid` | 가능 | |
| `rate_limited` | 가능 | |
| `quota_exceeded` | 가능 | |
| `image_paid_required` | 가능 | "이미지 0장으로 재시도" 버튼 → `yeri-image-zero` |
| `runner_update_required` | 가능 | |
| `naver_login_required` | 가능 | |
| `model_not_found` | **불가** (관리자) | AIMAX 모델 목록이 낡았다는 신호로 처리한다 |
| `admin_action_required` | **불가** (관리자) | 에디터 셀렉터/실행기 문제 |

- **비용 경고**: stage가 `content_generation`이거나 `smart_editor_*`이면 "글 생성 비용이 이미 발생했을 수 있다"는 문구를 붙인다. quota/429는 예외다 (JS:L1532-1541).

### 3.6 Cancel
- live 상태이고 송이 row가 아닌 job에만 "중단" 버튼이 있다.
- confirm 문구는 "이미 생성된 원고나 복구 파일은 가능한 한 보존"이다.
- POST cancel 후 `refreshAll` + agent watch 60초를 시작한다 (JS:L1949-1952, L8636-8651).
- 실제로 중단하는 주체는 로컬 실행기이다. 서버가 cancel 플래그를 세우고 agent가 heartbeat에서 반영하는 구조로 추정한다.

### 3.7 Guard (반복 오류 차단)
- 서버는 같은 오류가 반복되면 해당 `job_kind`의 신규 job을 **409 `guard_paused` + `message`**로 거절한다. 실행기 버전이 최소 기준 미만이면 409 `runner_update_required`를 반환한다.
- 클라이언트 동작 (JS:L8570-8587, L9019-9040):
  - 배너 제목은 "같은 오류가 반복되어 작업을 잠시 멈췄습니다"이다.
  - 실패한 `{kind, payload}`를 `state.jobGuardBlocked`에 보관한다.
  - "조치했어요, 다시 시도"를 누르면 POST `/api/jobs/guard/acknowledge {job_kind}`를 보내고, 성공하면 **같은 payload로 `createJob`을 자동 재호출**한다.
  - `runner_update_required`일 때는 ack 버튼을 숨긴다.
- 의미: 서버측 circuit breaker이다. 사용자가 조치했다고 명시적으로 확인하기 전까지 비용을 유발하는 반복 실행을 막는다.

### 3.8 폴링 / 갱신 주기
| 대상 | 주기 | 종료 조건 | 근거 |
|---|---|---|---|
| job watch | 3000ms (첫 tick 1200ms) | live job이 0개가 되면 | JS:L8150-8168 |
| agent/전체 watch | 3000ms (첫 tick 1500ms) | 기본 60초. 설정/import는 90초, job 생성 후는 120초 | JS:L8170-8184, L8615 |
| agent command 대기 | 1500ms | 기본 12초. import 45초, open_settings 60초. `done`/`failed` 또는 `delivered`/`assigned_at`에서 끝 | JS:L8408-8423 |
| discovery run 대기 | 2500ms | 최대 90초. completed/failed에서 끝 | JS:L7612-7630 |
| 전체 refresh | 탭 visible 전환, window focus, 수동 버튼 | – | JS:L9861-9870 |

`refreshAll`은 단일 in-flight promise로 중복 실행을 막는다 (JS:L8231-8239). 로드 순서는 다음과 같다 (JS:L8186-8229).
1. me
2. agent status + jobs (병렬)
3. version
4. downloads
5. reports
6. secrets
7. writing-style
8. research integrations
9. research data
10. workers (`applyWorkerCatalog`)
11. tax
12. renderAll

---

## 4. 로컬 PC 실행기 (Agent)

### 4.1 정체
"AIMAX 로컬 실행기 / 통합 실행기"로 불린다. 근거와 특징은 다음과 같다.
- Windows 설치 파일 `aimax-bundle-windows.exe` (HTML:L1129), macOS DMG (Apple Silicon 전용) (HTML:L1386, L1403).
- Chrome/Selenium (chromedriver, `localhost:8669`)으로 Naver에 로그인하고 SmartEditor에 입력/저장/발행/예약한다 (JS:L1514, L1690-1700, L3203-3209).
- Naver ID/PW는 **PC에만 저장**하고 웹 서버로 보내지 않는다 (HTML:L1498).
- 로컬 AI 키(`readiness.ai_keys`)도 보관할 수 있다. 예전 방식이고 지금은 웹 시크릿으로 이전 중이다.
- yt-dlp/ffmpeg 미디어 도구를 갖고 있다. YouTube 쇼츠 후보 검색과 영상 다운로드에 쓴다 (JS:L1284-1287, L6071-6091).

### 4.2 연결 / 기동
- 커스텀 프로토콜 `aimax://agent/connect?account=<email>`과 `aimax://agent/open-settings`를 쓴다. 로그인 계정 이메일을 넘겨서, 실행기에 다른 계정 토큰이 있으면 재로그인을 유도한다 (JS:L8384-8406).
- 기동 후 15초 동안은 busy이고, 그 뒤 60초 status watch로 연결을 확인한다.
- 온라인 판정은 `agent.connected` (서버 계산)와 `last_seen_at`이다. 서버가 heartbeat 기준으로 판단하는 것으로 추정한다 (JS:L4476-4478).
- `device_label`로 장치를 표시한다.

### 4.3 readiness 구조 (`agent.readiness`)
- `naver_account{status, has_id}`, `ai_keys{gemini|openai|claude: status}`, `neighbor_messages{status, count}`, `browser{status}`, `workers{<job_kind>: status}`.
- status 값: `ready / missing / needs_attention / unknown / unavailable` (JS:L4052-4067).
- `agent.status === "blocked_bundle_split_mismatch"` 또는 `diagnostics.bundle_split_mismatch`가 있으면, 번들 권한 사용자가 개별 직원 전용 실행기를 쓰고 있다는 뜻이다. 이때 통합 설치 파일 재실행을 안내한다 (JS:L3361-3386).

### 4.4 Command (웹 → 실행기 원격 명령)
- 생성은 POST `/api/agent/commands {type, platform, payload}`이다.
- 실행기가 폴링해서 받아가고, `assigned_at`이나 `delivered`를 남긴 뒤 `done`/`failed`와 `logs`/`result`를 보고한다.
- 클라이언트에서 확인된 type:

| type | payload | result | 근거 |
|---|---|---|---|
| `open_settings` | – | 실패 로그에 "저장을 취소"가 있으면 사용자 취소 | JS:L8435-8456 |
| `import_local_provider_secrets` | `{providers:["gemini","apify","openai","claude"]}` | `providers{<p>{status: imported\|missing\|failed}}`, `imported_count`, `missing_count`, `failed_count` | 실행기가 로컬 키를 서버로 업로드한다. 구버전이면 "지원하지 않는 웹앱 명령"으로 실패 (JS:L7462-7531) |

- 상태 전이: (생성) → `delivered` (`assigned_at` 기록) → `done | failed`.

### 4.5 Job 수행
- 서버 job은 `target_platform`/`target_device_label`로 특정 PC를 지정한다.
- 실행기가 queued job을 가져가면 `assigned_at`이 찍힌다.
- 진행 로그와 result를 업로드하는데, 로컬 파일 경로(recovery/이미지)도 포함된다.
- 에러 텍스트에 실행기 Python 모듈명이 노출된다: `measure_visible_char_count`, `cannot import name ... posting`, `unexpected keyword argument image_provider` (JS:L1639-1662). 실행기는 **Python 기반**으로 추정한다.

### 4.6 예리 "서버 글 생성" 하이브리드
조건 (JS:L3106-3147): `job_kinds[yeri_write].server_generation.enabled`이고, 다음을 모두 만족해야 한다.
- 선택 모델의 provider가 `supported_providers`에 있다 (기본 `["gemini"]`).
- 실행기가 연결되어 있고 업데이트가 불필요하다.
- Naver 계정이 ready이다.
- 웹 시크릿이 있다.
- 로컬 키가 없거나 worker가 not-ready이다 (또는 `real_test_only` 모드).

동작 (JS:L3496-3518, L8596-8603):
- confirm 창을 띄우고 body에 `server_generation:true, confirm_paid:true`를 넣는다.
- 서버가 글을 생성한 뒤 실행기가 Naver 입력만 담당한다.
- `real_test_only`일 때는 `max_word_count`와 `max_image_count` 제한을 클라이언트도 검사한다 (JS:L3483-3494).

### 4.7 버전 / 다운로드
- `version_info{current_version, latest_version, min_version, update_required, update_available, release_notes}`.
- update_required이면 모달과 job 차단이 일어난다. available이면 dismiss 가능한 배너를 띄우고, 키는 `aimax_update_notice_dismissed:<platform>:<latest>`이다 (JS:L2227-2335).
- 다운로드 대상: Windows이거나 product가 하나라도 있으면 항상 `bundle` 하나만 받는다 (JS:L3625-3632).
- 다운로드 흐름 (JS:L8344-8382):
  1. Mac이면 아키텍처를 검사한다. Intel이면 차단, 불명이면 확인을 받는다.
  2. Windows이면 SmartScreen 안내 모달을 띄운다.
  3. POST tickets로 받은 URL을 `<a download>`로 연다.
  4. 설치 가이드 모달을 띄운다.
  5. 20초 busy가 걸린다.

### 4.8 추정 아키텍처
- **Cloud control plane**: 인증, 권한, job 큐, command 큐, 시크릿 금고, 서버측 LLM/Apify/yt-dlp 실행, 보고서, 버전/배포 매니페스트, tax 프록시.
- **Local executor (Python + Chrome)**: Naver 계정과 브라우저 세션을 보유하고, job/command를 polling해서 수행한 뒤 진행 상황을 report한다.
- 웹앱은 이 둘을 관찰하고 지시하는 thin console이다.

---

## 5. Workers (AI 직원) 카탈로그

### 5.1 `/api/workers` 응답에서 소비하는 필드 (JS:L2535-2626)
- `workers[]`: `staff_code|product` (키), `code`, `name|label`, `role`, `avatar_image`, `profile_image`, `product`, `job_kind`, `execution`, `type`, `module_key`, `status`, `access_policy`, `required_settings[]`, `repo_url`, `release_url`, `setup_download_url`, `portable_download_url`, `download_label`, `supported_platforms[]`, `version`, `short_description`, `capabilities[]`, `execution_options[{kind,label,platforms[],status,url,launch_endpoint,primary,description}]`, `external_url`, `cta_label`, `partner{name,maker,support_note}`, `resume{employeeNo, team, experience, hometown, formerRole, statement, intro, career[{period,org,note}], reference{quote,from}, interviewLine, skills[{label,score}], workStyle}`.
- `job_kinds[]`: `kind, label, required_product, staff_code, worker_code, execution, api_mode, queue, server_generation{enabled, supported_providers[], real_test_only, max_word_count, max_image_count}`.
- 카탈로그가 비어 있지 않으면 하드코딩 fallback 중 카탈로그에 없는 직원을 **삭제**한다. 서버가 정본이다 (JS:L2519-2526).

### 5.2 실행 유형 (execution/type)
- `local_agent`: 로컬 실행기.
- `web_module`: 웹 폼.
- `external_download` / `desktop_app`: 설치형 앱 직접 다운로드.
- `external_tool` / `multi_channel`: 여러 실행 옵션.
- `partner_external`: 외부 파트너 사이트로 링크만 연결한다. job, 실행기, 오류보고 대상이 아니다 (JS:L266-269, L2654-2720).

실행 옵션 kind는 `web_app, web_module, android_apk, toss_mini, mobile_web, windows_download, mac_download, pc_download, local_agent, partner_external`이다. 옵션 status는 `available, testing, unsupported, coming_soon`이다 (JS:L2722-2843).

### 5.3 권한 모델
- products: `yeri, hyunju, songi, ddakfit, yunmi, jieun, semu, nakyung, hyojin, sangsu, eunseo, blog_team(=yeri+hyunju), bundle(전체, member-only 제외), free` (JS:L220-235, L2462-2481).
- access_policy:
  - `public`: active 사용자 전원.
  - `partner`: 로그인만 하면 된다.
  - `makefamily_member` / `member_only`: account_segment가 member, operator, test 중 하나여야 한다 (JS:L2643-2704).
- 직원 상태 값: `available, needs_setup, no_entitlement, agent_offline, unsupported_agent, windows_only, unknown` (JS:L3239-3304).

### 5.4 직원 매핑 (요청 목록과 대조)
| 코드 | 이름 / 역할 | 실행 | required_settings / 비고 | 근거 |
|---|---|---|---|---|
| yeri | 예리 · 블로그 글쓰기 | local_agent | naver_account, ai_key. Naver 블로그 글 작성, 즉시/임시/예약 발행 | JS:L272-311 |
| hyunju | 현주 · 영업사원 | local_agent | naver_account, neighbor_messages. 키워드/타겟 블로거 팔로워 대상 서로이웃 신청, 속도 3단계 | JS:L312-351 |
| songi | 송이 · 자료조사원 | **partner_external** (2026-09-02부터) | `https://hoomcha.com/aimax`, 제작 "정보람". 기존 research 웹 모듈 코드는 남아 있다 | JS:L512-562 |
| yunmi | 윤미 · 스크립트작가 | web_module (beta) | execution option web_app이 scriptbot 티켓을 발급하고 iframe으로 연다. 사용자 Gemini 키를 쓴다 | JS:L483-511 |
| sangsu | 상수 · 경리 | web_module (`quote_generator`) | **견적서만** 담당. 브라우저에서 PDF/인쇄하며 API 호출이 없다 | JS:L441-482, HTML:L554-636 |
| semu | 세무 직원 · 전자세금계산서 | web_module (`tax`) | Popbill 테스트베드. 2026-09-08에 카탈로그에서 삭제됐지만 폼/JS는 남아 있다 | JS:L614, L810-815, HTML:L789-900 |
| jieun | 지은 · AI 오피스 지원 | external_download | Win Setup 0.1.6, Mac **Tauri** 0.2.1 aarch64 DMG. 캡처/모자이크/OCR/화면녹화/Windows 종료 | JS:L615-683 |
| nakyung | 나경 · 판서 | external_download (Windows) | Pencil 1.0.0 Setup/portable. 전체화면 판서, 같은 Wi-Fi 실시간 공유 | JS:L352-400 |
| eunseo | 은서 · 녹화 프롬프터 | multi_channel, member 전용 | 웹 `/eunseo` (launch 티켓), Android APK (testing), Toss 미니앱 (coming_soon) | JS:L684-767 |
| jiwon | 지원 · 정부지원사업 길잡이 | partner_external | `ddakfit.bccconsulting.kr/aimax` (product `ddakfit`) | JS:L563-613 |
| hyunseong | 현성 · PM | 미구현 (needs_setup) | 기능 준비 중이며 카탈로그에서 숨겨진다 | JS:L401-440 |
| hyojin | 효진 | 서버 카탈로그 전용 | fallback 정의가 없고 라벨/피드백 옵션에만 나온다 | JS:L229, HTML:L1236 |
| (맥스 알람) | – | – | **이 클라이언트에는 없다** | grep 결과 없음 |

이력서 데이터(사번 `AIMAX-2026-<CODE>`, 소속, 경력, 출신, 전 직장, 기술 점수 1-5, 추천사, 면접 한마디)는 캐릭터 설정이다. 오리지널 제품에서는 **자체 캐릭터와 문구로 새로 써야 한다**.

---

## 6. Research 모듈 (송이) 워크플로우

### 6.1 엔티티
- **Project**: `id, name, industry, goal, instagram_profile_url, content_category, content_topic, profile_fetch_status (apify_scraped/apify_failed), profile_source_text`.
- **Item**:
  - 기본: `id, project_id, url, title, platform, category, tags[], memo, summary, insights[], source_text`.
  - 4대 노트: `hook_note, copy_note, structure_note, performance_note`.
  - 심화: `hooks[], flow[{timestamp,description}], benchmarking[], copywriting_points[], visual_cues[], performance_reasons{views,likes,comments,saves}, script_brief | personalized_plan` (윤미 전달용 JSON/텍스트).
  - 링크 상태: `link_fetch_status`, `link_fetch_error`.
  - Apify: `apify_*` (collection_status, actor_id, run_id, status_url, error).
  - AI: `ai_analysis_status (queued/running/completed/failed)`, `ai_analysis_model`, `ai_analysis_error(_detail)`.
  - 미디어: `media_analysis_status (attached/failed/unsupported/skipped)`, `frame_capture_status`, `frame_captures[{image_url,timestamp,description}]`.
  - 근거: JS:L6299-6806, L8038-8067.
- `link_fetch_status` 값: `html, json, text, youtube*, youtube_discovery, apify_needs_approval, apify_local_pending, apify_key_missing, apify_scraped, apify_failed, *failed, *_error` (JS:L6299-6314, L2414-2417).

### 6.2 흐름 A — 링크로 분석 (JS:L9623-9703)
1. 프로젝트를 선택하거나 새로 만들고 개인화 정보를 PATCH한다. 개인화는 IG 프로필 URL **또는** (카테고리 + 주제)가 있어야 한다.
2. `confirmSongiAutoRun`이 예상 원가와 단계 설명을 보여주고 동의를 받는다. 단계 설명 예: Apify는 링크당 결과 1개만 수집하고 다운로드/전사는 끈다. YouTube는 서버가 저해상도 임시 파일을 받아 Gemini에 첨부하고, 실패하면 자막/본문으로 분석한다 (JS:L7753-7783).
3. 동의했고 IG 프로필이 필요하면 → `profile/apify` (Apify Instagram Profile).
4. URL마다 순서대로 진행한다.
   - `POST items`: 서버가 링크를 읽는다 (공개 http/https만 허용. SSRF 방어 에러코드 `research_url_not_public`, `research_url_protocol_not_allowed`).
   - Instagram/TikTok이면 `items/{id}/apify`.
   - 그다음 `items/{id}/ai-analysis` (Gemini, `include_video:true`).
5. 결과 카드: 성과 해석 요약, "윤미에게 넘기기"(→ `yunmi-script` job 생성), 오류 보고, 근거 접기(브리프 / 태그 / 4대 노트 / 6개 인사이트 패널 / 프레임 캡처 / 원문 미리보기).

### 6.3 흐름 B — 키워드로 찾기 (Discovery) (JS:L9478-9554)
- 플랫폼별 수집 방식:
  - `youtube`: 무료. 서버 또는 로컬 yt-dlp로 공개 검색 메타데이터만 가져오고 API quota를 쓰지 않는다.
  - `instagram | tiktok | threads | meta_ads`: 사용자 Apify 키로 서버가 수집한다.
- 옵션: `sort_mode` (top/recent), `date_range_days` (7/30/90), `max_results` (8/12/20, meta_ads는 최소 10).
- `run.source_mode`가 `server_ytdlp` 또는 `server_apify`이면 즉시 결과가 온다. 그 외는 **로컬 실행기에 위임**하므로 실행기를 기동하고 run을 폴링한다 (JS:L9520-9531).
- Candidate 필드: `id, run_id, platform, title, creator, published_at, url, thumbnail_url, candidate_score, measurement_badge, reason, content_format, imported_item_id, metrics{view_count, views_per_hour, duration_seconds, engagement_rate, like_count, comment_count, share_count, ad_variant_count, is_active_ad, is_short_form}`. 점수 내림차순으로 정렬한다 (JS:L5883-5889, L6113-6181).
- "분석에 추가"를 누르면 candidate import로 item이 만들어진다.

### 6.4 구독 (자동수집)
- `frequency`는 daily 또는 weekly이다. 비용 추정은 회당 비용 × 월 30.4회 또는 4.35회이다 (JS:L5927-5936).
- 구독 필드: `status (active/paused)`, `pause_reason (user_paused, research_apify_key_missing, research_apify_invalid_token, research_apify_credit_exhausted, repeated_failures, research_discovery_keyword_invalid)`, `per_run_estimate_usd`, `monthly_estimate_usd`, `run_count`, `total_cost_usd`, `next_run_at`, `last_run_at`, `last_new_count`, `last_run_error`.
- 서버 cron이 새 후보만 모으고, 키 오류나 크레딧 부족이 생기면 자동으로 일시정지한다 (JS:L5938-6023, L9579-9583).

### 6.5 Integrations 객체 (JS:L4095-4117, L5848-5865, L6038-6092)
- `gemini{configured, user_configured, global_configured, model}`
- `apify{configured, user_configured, global_configured, local_configured, discovery_pricing_usd{<platform>{start, per_result}}}`
- `youtube{server_configured, local_configured, local_agent_connected, local_yt_dlp_status}`
- `media_tools{video_file_analysis_ready, video_download{available}, frame_extract{available}}`

### 6.6 브리프
- GET brief는 프로젝트 전체를 한 문서로 만들어 준다. 클라이언트는 그것을 클립보드로 복사한다.
- 다운스트림은 윤미(숏폼 대본)이다.
- 정리하면 파이프라인은 **수집 → 정규화 → LLM 구조화 분석 → 개인화 기획 → 대본 생성**이다.

---

## 7. 세금계산서 모듈 (세무 직원)

- **Provider: 팝빌(Popbill) 테스트베드**. "팝빌 테스트베드 연결을 준비 중" (JS:L3286, L3398), "팝빌에서 실제 문서 상태를 다시 확인" (JS:L5189). 화면에는 "실제 국세청 전송 아님"이 명시되어 있다 (HTML:L795, L885).
- 문서번호 `mgt_key`(Popbill MgtKey 개념), 포인트 잔액, 발행 단가, 회원가입 필드 부족 에러가 있다. Popbill 파트너 연동(파트너 키는 서버 보관, 사업자 회원 자동가입 시도)으로 판단한다.
- **Settings** (공급자): `corp_num` (숫자 10자리), `corp_name`, `ceo_name` → 서버가 `business_registered`, `member_connected`를 돌려준다. 등록한 사업자번호만 공급자로 쓸 수 있다 (HTML:L798-820, JS:L4964-4994).
- **Draft payload** (JS:L4913-4927):
  - `invoicee{corp_num, corp_name, ceo_name, email}`
  - `items[{item_name, quantity, unit_price, supply_cost, tax}]`: 공급가액 = 수량 × 단가, 세액 = round(공급가액 × 0.1)이며 세액은 수정할 수 있다.
  - `write_date` (YYYYMMDD), `purpose_type` (청구/영수), `totals{supply_cost_total, tax_total, total_amount}`.
- **Invoice 상태**: `draft → submitting → issued`. 예외는 `unknown` (결과 불명), `failed`. `failure{code, message, stage}`와 `recovery_guidance`가 함께 온다 (JS:L4829-4838, L4996-5021).
- **발행 흐름** (2단계 비용 확인):
  1. 초안을 저장한다 (`POST invoices`).
  2. "발행 준비"는 preflight이다. `unit_cost`, `balance`(포인트), `can_issue`를 받는다.
  3. "표시된 발행 단가와 잔여 포인트를 확인했습니다" 체크박스를 켜야 발행 버튼이 활성화된다.
  4. `issue {confirm_paid:true}`.
  5. 응답이 실패하면 즉시 refresh한다. `unknown`/`submitting` 상태의 문서에는 "상태 확인"(sync) 버튼이 생긴다. 이것이 **멱등성/결과 불명 처리** 패턴이다 (JS:L5023-5043, L5143-5190, L9178-9191).
- 실패 문서에는 "오류 보고" 버튼이 있다. 보고 내용에는 mgt_key, 상태, 실패 단계, `테스트베드=true`가 들어간다 (JS:L5192-5202).

---

## 8. User secrets / 글쓰기 스타일 / 웹 설정

- **Providers** (JS:L96-102, HTML:L987-1048):

| provider | 용도 |
|---|---|
| `gemini` | 송이 분석, 예리 기본, 윤미 |
| `apify` | IG/TikTok/Threads/Meta Ads 수집 |
| `youtube` | Data API. 선택 고급 커넥터이며 기본 경로는 쓰지 않는다 |
| `openai` | gpt-* 글 모델과 gpt-image |
| `claude` | "claude" 글 모델 |

- 저장 위치: 서버측 "웹 보안 저장소"이다. 응답에 원문은 없고 `configured/source/updated_at`만 온다.
  - `source = web_user`: 사용자 키.
  - `source = server_global`: 운영사가 제공하는 전역 키 (JS:L4521-4525).
- 저장할 때 서버가 즉시 검증한다. 결과는 `verification.verified/username/plan`이다.
- 로컬 실행기 키 가져오기 정책: Gemini, Apify, OpenAI, Claude 4종만 옮긴다. YouTube 키와 Naver 비밀번호/쿠키/세션은 옮기지 않는다 (JS:L7464, HTML:L986).
- **글쓰기 스타일**: 서버에 저장되는 계정 기본값이다. `style`과 `options[{key,label,hint}]`이 있다. 클라이언트가 아는 키는 3개이다 (JS:L7282-7385, HTML:L384-391).

| key | 라벨 | 구성 |
|---|---|---|
| `consult` | 상담 유도형 | 소제목 · 확인 목록 · 비교표 · 사진 설명 |
| `info` | 정보 정리형 | 소제목 · 단계 목록 · 과정 사진 · 요약 |
| `review` | 후기·추천형 | 소제목 · 비교표 · 장단점 · 사진 설명 |

  job별로 `style_id`를 넘기면 계정 기본값보다 우선한다.
- **웹 작업 설정**: 서버가 아니라 **브라우저 localStorage** `aimax_web_work_settings`에 저장된다 (JS:L2028-2060).
  - 필드: `ai_model`, `image_model`, `blog_profile` (≤4000자), `neighbor_messages[]` (≤20개 × 220자).
  - "멘트 초안 만들기"는 LLM이 아니라 **템플릿 10개**에 프로필 앞 120자를 끼워 넣는다 (JS:L2183-2201).

---

## 9. Reports (오류 보고 / 직원 피드백)

- **하나의 엔드포인트**에서 `report_kind`로 구분한다 (`feedback` 또는 기본값 error). `source=staff_feedback`도 피드백으로 처리한다 (JS:L3788-3800).
- **자동 첨부되는 진단 정보** (JS:L981-1046):
  - `web_context`: URL, UA, 언어, 플랫폼, Mac 아키텍처, 타임존, viewport, screen, ISO 시각.
  - `system.app`: name "AIMAX Web App", 실행기 버전, mode web.
  - `system.runtime`.
  - `system.agent`: connected, status, version, platform, device_label, readiness, diagnostics.
  - `system.research.media_tools`.
  - `system.user_secrets`: configured 플래그만 담는다.
  - `system.jobs_recent`: 최근 5개 job 요약 (id, kind, status, timestamps, last_log, result 요약).
  - 선택: `system.research_item`. 송이/윤미 실패 시 단계/코드/모델/Apify run/요청 ID/idempotency_key를 담는다 (JS:L8038-8067, L8091-8106).
- **피드백 객체**: `employee_code, employee_label, rating(1-5), good, improve, contact_needed, selected_employee, selected_job_kind` (JS:L7944-7953).
- **상태와 의미** (JS:L3733-3781):
  - `new`: 접수됨. 영업시간 기준 24시간 안에 1차 확인한다.
  - `reviewing` / `checking`: 확인 중.
  - `working`: 조치 중.
  - `waiting_user` / `need_user_info`: 사용자 확인 필요.
  - `done` / `fixed` / `resolved`: 완료.
- **운영자 → 사용자 루프**: report에 `user_action_title`, `user_action_checklist[]`, `user_response_required`가 오면 체크리스트와 메모 칸을 보여준다. 사용자는 "해결됐어요 / 아직 안 돼요"로 `/response`에 답한다.
  - 미확인 상태는 강제 모달로 띄운다. 키는 `aimax_report_attention_seen:<id>:<status_updated_at>`이다 (JS:L3822-3930, L8111-8125).
  - 배너도 함께 띄운다.
- 운영측 알림은 "admin 피드백 필터와 텔레그램 알림"으로 간다 (HTML:L1278). 추가 문의는 카카오채널로 접수 ID와 함께 받는다 (JS:L3758, L7114).
- **자동 운영 보고**: `submitOperationalErrorReportOnce(key)`는 같은 key당 하루 1회로 제한하며 localStorage `aimax_auto_report_<key>_<date>`를 쓴다. 예: 로컬 설정 열기 실패 (JS:L7991-8010, L8448).

---

## 10. LLM 모델 / 가격 / 비용 승인 / 백엔드 행동 단서

### 10.1 모델 목록
- **글쓰기 모델** (JS:L41-49):
  - `gemini-3.6-flash` (기본/고품질)
  - `gemini-3.5-flash-lite` (가장 저렴)
  - `gemini-3.1-pro-preview`
  - `gpt-5.6-luna`
  - `gpt-5.6-terra`
  - `claude` (단가 주석상 Claude Sonnet 5)
- **이미지 모델** (JS:L50-73):

| 모델 | 단가 | 그림 안 한글 |
|---|---|---|
| `gpt-image-1` | $0.042 (기본) | 깨짐 |
| `gpt-image-2` | $0.053 | **가능한 유일한 모델** (JS:L58-61) |
| `gemini-3.1-flash-image` (Nano Banana 2) | $0.067 | 깨짐 |
| `gemini-3-pro-image` | $0.134 | 깨짐 |

  깨지는 모델을 고르면 글자 없는 장면 위주로 그리도록 프롬프트 정책을 바꾼다는 안내가 나온다 (JS:L2123-2139).
- 윤미 모델은 Gemini 3종이다 (JS:L74-78). 송이 기본 분석 모델은 `gemini-3.5-flash-lite`이다 (JS:L19).
- 구모델 저장값은 provider와 가격대를 유지한 채 후속 모델로 매핑한다 (JS:L1058-1095).
- 주석에 따르면 서버는 기동 시 가용 Gemini 모델을 조회해 **fallback chain**을 만든다 (JS:L11-16).

### 10.2 가격표와 원가 추정
- 가격표 (USD / 1M tokens, JS:L79-95):

| 모델 | input | output |
|---|---|---|
| 3.6-flash | 1.50 | 7.50 |
| 3.5-flash-lite | 0.30 | 2.50 |
| 3.1-pro-preview | 1.25 | 10.00 |
| luna | 1.00 | 6.00 |
| terra | 2.50 | 15.00 |
| claude | 2.00 | 10.00 (인트로가, 이후 3/15) |

- 환율: `USD_KRW_RATE = 1475` (JS:L20).
- 글 원가 추정: input 2,200 tokens 고정, output = 글자수 × 0.8, 글자수는 300-6000으로 clamp (JS:L1133-1159).
- 실측 주석: 3.6-flash는 약 32원/글, 3.5-flash-lite는 약 3.5원/글 (JS:L15-16).
- Apify 단가 (USD): Instagram 0.0026/건, Instagram Profile 0.0016, TikTok 0.01 (JS:L22-26).
- Discovery fallback 단가 (start + per_result): instagram 0.005 + 0.0026, tiktok 0.012 + 0.0017, threads 0.01 + 0.002, meta_ads 0.005 + 0.00075 (JS:L34-39). 서버가 `discovery_pricing_usd`를 주면 그 값을 우선한다.
- 무료 티어 안내: 무료 사용량은 0원이라 계산되지 않지만 호출 한도가 있다. 고품질 모델일수록 한도에 빨리 닿는다 (JS:L1335-1341).

### 10.3 비용 승인 원칙
웹 시크릿 공지와 코드 전반에서 공통으로 나타난다.
1. 실행 전에 `window.confirm`으로 예상 원가, 모델, 토큰, 중복 실행 경고를 보여준다.
2. 서버 요청 body에 `confirm_paid:true`가 없으면 서버가 거절한다.
3. **자동 유료 재시도를 하지 않는다.**
4. `request_id` / `idempotency_key`로 중복 제출을 막는다 (윤미, JS:L8965-8981).
5. 같은 자료에 유료 분석이 진행 중이면 `research_paid_operation_in_progress`로 동시 실행을 잠근다 (JS:L2386).
6. Apify는 "내 크레딧에서 차감"을 명시한다.
7. 세금계산서는 preflight와 체크박스를 거친다.

job 결과에는 실제 `cost`와 `usage`가 기록되어 테이블에 표시된다 (JS:L1352-1369).

### 10.4 서버 행동이 드러나는 에러 코드 (JS:L2337-2412)
- `server_generation_*`: `auth_failed, quota_exceeded, rate_limited, model_not_found, provider_transient, provider_error, timeout, invalid_response`.
- `yeri_*`: `paid_confirmation_required, server_generation_provider_mismatch, server_generation_not_allowed` (계정 allowlist), `claude/openai/gemini_key_missing`.
- `research_*`:
  - URL 검증: `invalid_url, link_fetch_failed, link_http_error, link_timeout, url_lookup_failed, url_not_public, url_protocol_not_allowed`.
  - 저장: `storage_path_required, storage_not_writable, storage_update_failed`. 예전에 로컬 저장 경로를 설정하던 기능의 흔적이다.
  - Discovery: `discovery_keyword_required, discovery_platform_not_ready, discovery_candidate_not_found, discovery_keyword_invalid` (해시태그 변환 실패).
  - Apify: `apify_key_missing, apify_not_supported, apify_profile_not_supported, apify_no_items, apify_run_still_running` (비동기 run 이어보기), `apify_run_failed, apify_run_missing, apify_failed, apify_invalid_token, apify_credit_exhausted, apify_discovery_failed`.
  - Gemini: `gemini_key_missing, gemini_invalid_api_key, gemini_invalid_json, gemini_bad_request, gemini_auth_or_permission, gemini_model_unavailable, gemini_quota_or_rate_limit, gemini_high_demand, gemini_timeout, gemini_provider_error, gemini_failed`.
- YouTube: `youtube_api_key_missing, youtube_discovery_failed, songi_youtube_discovery_failed, songi_youtube_runner_or_server_unavailable, server_ytdlp_discovery_failed`.
- 기타: `apify_token_invalid`, `tax_not_configured`, `member_join_fields_required`, `guard_paused`, `runner_update_required`.

---

## 11. 디자인 토큰 요약 (CSS:L1-25 외)

- **색** (light only, 다크모드 없음):
  - 배경과 선: bg `#f4f6f3` (따뜻한 오프화이트), panel `#fff`, panel-soft `#fbfaf7`, line `#d9ddd6` / strong `#c5ccc2`.
  - 텍스트: ink `#191d1b`, muted `#66736d`.
  - 브랜드: **teal `#0f766e`** (dark `#0b5f58`), accent blue `#315f9b` (soft `#eaf1f8`).
  - 상태: ok `#147a55` / soft `#e9f6ef`, warn `#9a6500` / soft `#fff4d8`, bad `#b3261e` / soft `#fff0ec`.
  - 사이드바: `#171c19` 다크 그린-블랙. active `#243128`, 좌측 3px brand inset bar.
- **형태**: radius 8px가 압도적이다 (54회). pill은 999px. shadow-soft `0 4px 16px rgba(22,29,25,.06)`.
- **타이포**: 시스템 폰트 스택. 상단 제목 28px, 패널 제목 17px, pill 13px.
- **레이아웃**: 12열 grid, gap 16px. 패널 padding 20px, main padding 30px, sticky topbar.
- **컴포넌트**: 버튼 min-height 40px (primary는 teal + 그림자). job workspace 2열은 1100px 이하에서 1열이 된다. 직원 grid `auto-fill minmax(260px)`. 이력서 시트 `min(1120px, A4 비율)` 3열. research 2열 (280-360px | 1fr). 모달 카드 440px. 토스트는 다크 `#171c19`.
- 견적서 인쇄 팝업은 A4 794×1123px, teal 강조 표 스타일이다 (style_1.css).
- 오리지널 앱 제안: 같은 "차분한 업무 콘솔" 톤은 유지하되 brand hue와 sidebar tone을 바꾸고, 다크모드 토큰을 추가하는 것이 좋다.

---

## 12. 오리지널 Tauri 데스크톱 앱 설계 제안

### 12.1 핵심 판단
원본은 **웹 콘솔 + 클라우드 + 로컬 Python 실행기**로 세 갈래로 나뉘어 있다. 이 때문에 생기는 문제는 다음과 같다.
- 버전 불일치 (bundle split mismatch, min_version)
- 프로토콜 핸드셰이크 (`aimax://`)
- 키 이전 (import_local_provider_secrets)
- 원격 command 폴링

Tauri 앱은 **콘솔과 실행기를 하나의 프로세스로 합치면** 이 범주의 문제를 대부분 없앤다.

### 12.2 로컬(Rust)에서 할 것
| 기능 | 구현 메모 |
|---|---|
| 작업 큐 / 상태머신 / 로그 / 비용 원장 | tokio task + SQLite. job 상태는 `queued → running(step) → done/failed/cancelled`. 진행 이벤트는 Tauri event로 UI에 push한다 (폴링이 필요 없다). cancel은 `CancellationToken`으로 한다 |
| Guard (반복 오류 차단) | 오류 fingerprint(kind + stage + code)를 카운트한다. N회 연속이면 kind를 paused 상태로 두고, UI에서 acknowledge해야 해제한다 |
| BYOK LLM 호출 (Gemini / OpenAI / Anthropic) | `reqwest` 직접 호출. 키는 **OS keychain** (`keyring` crate)에 두고 SQLite에는 메타데이터만 둔다. 사전 원가 추정과 confirm, `request_id` 멱등키, 자동 유료 재시도 금지 |
| Apify 수집 | 사용자 토큰으로 actor run → poll → dataset을 가져온다. run-still-running이면 재개한다. 크레딧/토큰 오류 시 구독을 자동으로 정지한다 |
| YouTube 메타데이터, 영상 프레임 | yt-dlp/ffmpeg를 **Tauri sidecar**로 번들한다 (라이선스 고지 필요). 메타데이터 위주로 쓰고 다운로드는 짧은 임시 파일로 한정한다 |
| 링크 읽기 | `reqwest` + readability 류 추출. 공개 http/https만 허용하고 사설 IP를 차단한다 (원본의 SSRF 규칙을 참고) |
| 브리프 / 대본 생성 | 프롬프트 템플릿은 로컬에 두고, 결과 JSON schema를 검증한다 (원본의 `invalid_json` 계열 대응) |
| 견적서 PDF | HTML 템플릿 → webview print-to-PDF, 또는 Rust `printpdf`/`typst`. 로고는 로컬 파일로 받는다 |
| 구독 스케줄러 | 앱 실행 중일 때 cron 류로 돈다. 놓친 실행은 기동 시 catch-up한다. 트레이 상주 옵션 |
| 진단 번들 | OS / 앱 버전 / 최근 job 5개 / 키 configured 플래그만 담는다. 원문 키는 넣지 않는다 |
| 블로그 자동화 (예리/현주 류) | 필요하면 `chromiumoxide`/`fantoccini` 등 CDP 기반으로 한다. **주의**: 대량 서로이웃 신청이나 자동 발행은 플랫폼 약관과 스팸 정책 위반 위험이 크다. 오리지널 제품에서는 "초안 생성 + 사용자 확인 후 게시" 또는 공식 API가 있는 채널로 축소할 것을 권장한다 |

### 12.3 클라우드가 필요한 것 (최소화)
- 라이선스와 권한 (구매 이메일 기반 활성화, product entitlement), 비밀번호 재설정.
- 자동 업데이트 매니페스트 (Tauri updater: latest / min_version / notes / 서명).
- 오류 보고와 피드백 수집, 운영자 답변 루프 (report status / checklist / response). 운영 알림 웹훅은 서버에 둔다.
- 전자세금계산서: Popbill류 **파트너 비밀키는 클라이언트에 넣으면 안 된다**. 얇은 서버 프록시(초안 / preflight / issue / sync)가 필수다. 사용자가 자기 연동 키를 가진 구조라면 예외가 된다.
- (선택) 직원 카탈로그 원격 구성. 원본처럼 서버 카탈로그가 로컬 기본값을 덮어쓰는 방식.
- (선택) 사용자가 키 없이 쓰는 "운영사 키" 모드. 이 경우 과금과 쿼터 관리를 서버가 맡는다.

### 12.4 SQLite 스키마 (제안)
```sql
CREATE TABLE app_kv(key TEXT PRIMARY KEY, value TEXT NOT NULL);  -- web settings, ui prefs
CREATE TABLE secret_meta(provider TEXT PRIMARY KEY, configured INTEGER, verified_at TEXT, account_hint TEXT); -- 값은 keychain
CREATE TABLE staff(code TEXT PRIMARY KEY, name TEXT, role TEXT, execution TEXT, module_key TEXT,
  status TEXT, required_settings TEXT, capabilities TEXT, profile_json TEXT, sort INTEGER);
CREATE TABLE job(id TEXT PRIMARY KEY, kind TEXT NOT NULL, staff_code TEXT, status TEXT NOT NULL,
  label TEXT, payload_json TEXT, result_json TEXT, stage TEXT, error_code TEXT, error_message TEXT,
  request_id TEXT UNIQUE, created_at TEXT, started_at TEXT, finished_at TEXT, updated_at TEXT);
CREATE TABLE job_log(id INTEGER PRIMARY KEY, job_id TEXT REFERENCES job(id) ON DELETE CASCADE,
  at TEXT, level TEXT, step TEXT, message TEXT);
CREATE TABLE job_artifact(id INTEGER PRIMARY KEY, job_id TEXT, kind TEXT, path TEXT, meta_json TEXT);
CREATE TABLE cost_ledger(id INTEGER PRIMARY KEY, job_id TEXT, provider TEXT, model TEXT, op TEXT,
  input_tokens INTEGER, output_tokens INTEGER, units REAL, usd REAL, krw INTEGER,
  estimated INTEGER, confirmed_at TEXT, created_at TEXT);
CREATE TABLE guard_state(kind TEXT PRIMARY KEY, fingerprint TEXT, fail_count INTEGER,
  paused INTEGER, paused_reason TEXT, paused_at TEXT, acknowledged_at TEXT);
CREATE TABLE research_project(id TEXT PRIMARY KEY, name TEXT, industry TEXT, goal TEXT,
  profile_url TEXT, content_category TEXT, content_topic TEXT, profile_status TEXT,
  profile_text TEXT, created_at TEXT, updated_at TEXT);
CREATE TABLE research_item(id TEXT PRIMARY KEY, project_id TEXT REFERENCES research_project(id) ON DELETE CASCADE,
  url TEXT, title TEXT, platform TEXT, category TEXT, tags TEXT, memo TEXT, source_text TEXT,
  fetch_status TEXT, fetch_error TEXT, collect_status TEXT, collect_run_id TEXT,
  analysis_status TEXT, analysis_model TEXT, analysis_error TEXT, analysis_json TEXT, -- summary/notes/hooks/flow/...
  media_status TEXT, created_at TEXT, updated_at TEXT);
CREATE TABLE research_frame(id INTEGER PRIMARY KEY, item_id TEXT, ts TEXT, path TEXT, description TEXT);
CREATE TABLE discovery_run(id TEXT PRIMARY KEY, project_id TEXT, subscription_id TEXT, platform TEXT,
  keyword TEXT, sort_mode TEXT, days INTEGER, max_results INTEGER, source_mode TEXT, status TEXT,
  error TEXT, cost_usd REAL, created_at TEXT);
CREATE TABLE discovery_candidate(id TEXT PRIMARY KEY, run_id TEXT, platform TEXT, url TEXT UNIQUE,
  title TEXT, creator TEXT, published_at TEXT, thumbnail TEXT, format TEXT, score REAL,
  reason TEXT, metrics_json TEXT, imported_item_id TEXT);
CREATE TABLE discovery_subscription(id TEXT PRIMARY KEY, project_id TEXT, keyword TEXT, platform TEXT,
  frequency TEXT, sort_mode TEXT, days INTEGER, max_results INTEGER, status TEXT, pause_reason TEXT,
  next_run_at TEXT, last_run_at TEXT, last_new_count INTEGER, run_count INTEGER, total_cost_usd REAL);
CREATE TABLE script_draft(id TEXT PRIMARY KEY, source_item_id TEXT, job_id TEXT, variants_json TEXT,
  recommendation_json TEXT, created_at TEXT);
CREATE TABLE quote(id TEXT PRIMARY KEY, data_json TEXT, logo_path TEXT, subtotal INTEGER, vat INTEGER,
  total INTEGER, pdf_path TEXT, created_at TEXT, updated_at TEXT);
CREATE TABLE tax_invoice(id TEXT PRIMARY KEY, remote_id TEXT, mgt_key TEXT, status TEXT,
  invoicee_json TEXT, items_json TEXT, write_date TEXT, purpose TEXT, supply_total INTEGER,
  tax_total INTEGER, total INTEGER, failure_json TEXT, preflight_json TEXT, updated_at TEXT);
CREATE TABLE report(id TEXT PRIMARY KEY, remote_id TEXT, kind TEXT, status TEXT, payload_json TEXT,
  public_message TEXT, checklist_json TEXT, response TEXT, synced_at TEXT, created_at TEXT);
CREATE TABLE message_template(id INTEGER PRIMARY KEY, scope TEXT, body TEXT, sort INTEGER);
```

### 12.5 Tauri command / event 목록 (제안)
- **계정과 라이선스**: `license_activate(email, code)`, `license_status()`, `update_check()`, `update_install()`.
- **설정과 시크릿**: `settings_get/set(key, value)`, `secret_set(provider, value)` (저장 + 검증), `secret_delete(provider)`, `secret_status()`, `writing_style_get/set`.
- **직원**: `staff_list()`, `staff_readiness(code)` (필수 설정을 점검하고 차단 사유와 조치를 반환).
- **작업**: `cost_estimate(kind, payload)`, `job_create(kind, payload, confirm_paid, request_id)`, `job_list(filter)`, `job_get(id)`, `job_cancel(id)`, `guard_status()`, `guard_acknowledge(kind)`.
- **리서치**:
  - 프로젝트: `research_project_create/update/delete/list`.
  - 자료: `research_item_create(url | text)`, `research_item_update`, `research_item_collect(id, confirm_paid)`, `research_item_analyze(id, model, include_video, confirm_paid)`, `research_brief(project_id)`.
  - 탐색: `discovery_search(params, confirm_paid)`, `discovery_import(candidate_id)`, `subscription_create/pause/resume/delete/list`.
- **대본**: `script_generate(source_item_id | brief, model, confirm_paid, request_id)`, `script_copy_variant`.
- **견적**: `quote_save`, `quote_render_pdf(id)` (반환값은 경로), `quote_list`.
- **세금계산서** (클라우드 프록시): `tax_settings_get/put`, `tax_draft_create`, `tax_preflight(id)`, `tax_issue(id, confirm_paid)`, `tax_sync(id)`.
- **보고**: `report_submit(kind, input, attach_diagnostics)`, `report_list()`, `report_respond(id, resolved | still_failing, note)`, `diagnostics_bundle()`.
- **Events** (backend → UI): `job:status`, `job:log`, `job:progress{step, pct}`, `cost:recorded`, `guard:paused`, `subscription:ran`, `update:available`, `report:attention`.

### 12.6 원본에서 가져갈 UX 패턴 (개념만)
- 직원 카드 + 이력서 모달로 기능을 의인화하는 것. 캐릭터와 문구는 새로 만들어야 한다.
- job 폼 좌측 입력 / 우측 실시간 미리보기 + 비용 추정 박스.
- 실패를 stage와 진단 코드로 사람이 읽는 카드로 바꾸는 것. "사용자 조치 가능 vs 관리자 조치"를 나누고 조치 버튼을 붙인다.
- Next-action 단일 CTA, readiness 체크리스트, 팝업 우선순위 체계.
- 유료 작업의 3중 안전장치: 사전 추정 + confirm, 서버/백엔드측 confirm 플래그, 멱등키와 자동 재시도 금지.

---

## 부록 A. localStorage 키 (원본)
- `aimax_session_token`, `aimax_web_work_settings`, `aimax_sidebar_collapsed`
- `aimax_update_notice_dismissed:<platform>:<latest>`
- `aimax_report_attention_seen:<report_id>:<status_updated_at>`
- `aimax_service_notice_20260530_yeri_hyunju_june1`, `aimax_web_secret_notice_20260522`
- `aimax_sangsu_quote_generator_state_v1` (로고 dataURL 포함)
- `aimax_auto_report_<key>_<YYYY-MM-DD>`

근거: JS:L3-9, L108, L2263-2269, L3862-3868, L7993.

## 부록 B. 외부 링크 (원본)
- 설치 가이드는 Notion (JS:L2).
- 윤미 scriptbot `https://aimax.ai.kr/scriptbot/` (JS:L505).
- 송이 파트너 `https://hoomcha.com/aimax` (JS:L527).
- 지원 파트너 `https://ddakfit.bccconsulting.kr/aimax` (JS:L578).
- 다운로드 경로 `/downloads/Pencil-Setup-1.0.0.exe`, `/downloads/AIMAX-Office-Manager-Setup-0.1.6.exe`, `/downloads/AIMAX-Office-Manager-macOS-0.2.1-aarch64.dmg` (JS:L367, L630, L652).
- 에셋 `/assets/avatar_*.jpg|png`, `/assets/aimax-brain-preview.mp4|webm` (HTML:L137-160).
