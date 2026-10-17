//! AI 직원 명부. 이름·이력서는 이 앱의 가상 페르소나이며, 업무 폼은 화면이 그대로 그린다.
//!
//! 폼 필드 종류: text, textarea, number, date, select, choice(chips|cards), checkbox,
//! folder(폴더 선택), items(견적 항목 표). `visibleWhen`으로 조건부 표시를 정한다.

use serde_json::{json, Value};

pub struct StepDef {
    pub key: &'static str,
    pub label: &'static str,
    /// 이 단계 전에 받아야 하는 확인 종류 ("apply" = 파일 이동, "merge" = 코드 병합).
    pub gate: Option<&'static str>,
    /// 이 단계를 실제로 하는 직원. None이면 업무를 맡은 직원.
    pub actor: Option<&'static str>,
}

const fn step(key: &'static str, label: &'static str, gate: Option<&'static str>) -> StepDef {
    StepDef { key, label, gate, actor: None }
}

const fn by(key: &'static str, label: &'static str, actor: &'static str) -> StepDef {
    StepDef { key, label, gate: None, actor: Some(actor) }
}

/// 개발팀 직원들 (사무실 화면에서 개발실에 앉는다).
pub const DEV_TEAM: &[&str] = &["taeo", "yuna", "geonwoo", "sohee", "minhyuk", "daeun"];

pub fn steps(employee_id: &str, safe: bool) -> Vec<StepDef> {
    match employee_id {
        // 안전 모드: 사장님이 OpenCode 터미널에서 실행하면 태오가 팀원을 서브에이전트로 부른다.
        "taeo" if safe => vec![
            step("setup", "작업 공간 준비", None),
            step("team", "팀 개발 (직접 실행)", None),
            step("merge", "병합", Some("merge")),
            step("deliver", "결과 보고", None),
        ],
        "yuna" | "geonwoo" if safe => vec![
            step("setup", "작업 공간 준비", None),
            step("build", "개발·리뷰 (직접 실행)", None),
            step("merge", "병합", Some("merge")),
            step("deliver", "결과 보고", None),
        ],
        "harin" => vec![
            step("prepare", "요청 확인", None),
            step("write", "초안 작성", None),
            step("review", "발행 전 점검", None),
            step("deliver", "결과 정리", None),
        ],
        "doyun" => vec![
            step("compute", "금액 계산", None),
            step("deliver", "견적서 작성", None),
        ],
        "seojin" => vec![
            step("collect", "자료 수집", None),
            step("analyze", "브리프 작성", None),
            step("deliver", "출처 정리", None),
        ],
        "minjae" => vec![
            step("prepare", "타깃 정리", None),
            step("research", "후보 탐색", None),
            step("deliver", "결과 정리", None),
        ],
        "sua" => vec![
            step("scan", "폴더 살펴보기", None),
            step("plan", "정리 계획", None),
            step("apply", "파일 옮기기", Some("apply")),
            step("deliver", "정리 보고", None),
        ],
        "jiho" => vec![
            step("prepare", "요청 확인", None),
            step("write", "대본 작성", None),
            step("deliver", "결과 정리", None),
        ],
        "yena" => vec![
            step("collect", "공고 읽기", None),
            step("analyze", "자격 점검", None),
            step("deliver", "결과 정리", None),
        ],
        // 개발팀: 전사 합동 자율 제작 파이프라인 (리서치 -> 기획·설계 -> 코어·화면 개발 -> QA·리뷰 -> 보완 -> 착수 가이드 -> 완료)
        "taeo" => vec![
            step("setup", "프로젝트 착수·준비", None),
            by("research", "도메인·UX 리서치", "seojin"),
            step("plan", "프로그램 기획·설계", None),
            by("backend", "코어·로직 개발", "geonwoo"),
            by("frontend", "화면·UI 개발", "yuna"),
            by("test", "QA·테스트 검증", "sohee"),
            by("review", "코드·품질 리뷰", "minhyuk"),
            step("fix", "리뷰 반영·보완", None),
            by("docs", "착수 가이드·문서화", "daeun"),
            step("merge", "병합·착수 완료", Some("merge")),
            step("deliver", "결과 보고", None),
        ],
        "yuna" | "geonwoo" => vec![
            step("setup", "작업 공간 준비", None),
            step("build", "개발", None),
            by("review", "코드 리뷰", "minhyuk"),
            step("merge", "병합", Some("merge")),
            step("deliver", "결과 보고", None),
        ],
        "sohee" => vec![
            step("setup", "작업 공간 준비", None),
            step("test", "테스트 작성·실행", None),
            step("merge", "병합", Some("merge")),
            step("deliver", "결과 보고", None),
        ],
        "daeun" => vec![
            step("setup", "작업 공간 준비", None),
            step("docs", "문서 작성", None),
            step("merge", "병합", Some("merge")),
            step("deliver", "결과 보고", None),
        ],
        "minhyuk" => vec![
            step("review", "코드 리뷰", None),
            step("deliver", "리뷰 보고", None),
        ],
        _ => vec![],
    }
}

/// 실행 전 필요한 준비물. "opencode" = OpenCode 준비, "business" = 사업장 상호.
pub fn requirements(employee_id: &str) -> &'static [&'static str] {
    match employee_id {
        "doyun" => &["business"],
        "sua" => &[],
        _ => &["opencode"],
    }
}

fn web_search_field(default: &str, label: &str) -> Value {
    json!({
        "id": "webSearch", "kind": "select", "label": label, "default": default,
        "choices": [
            { "value": "0", "label": "사용 안 함" },
            { "value": "3", "label": "가볍게 · 검색 최대 3회" },
            { "value": "6", "label": "꼼꼼하게 · 검색 최대 6회" },
            { "value": "10", "label": "깊게 · 검색 최대 10회" }
        ],
        "hint": "OpenCode가 직접 웹을 검색하고 출처를 붙입니다."
    })
}

pub fn all(safe: bool) -> Vec<Value> {
    vec![
        json!({
            "id": "harin", "name": "하린", "role": "블로그 작가", "team": "콘텐츠팀",
            "mode": "ai", "hue": 18,
            "summary": "키워드와 가게 이야기를 받아 네이버 블로그에 바로 옮길 수 있는 발행 전 초안을 씁니다.",
            "motto": "검색에 걸리고, 끝까지 읽히는 글을 씁니다.",
            "capabilities": ["키워드 글 구성", "브랜드 문체 반영", "제목·마무리 후보", "발행 전 점검표"],
            "inputSummary": "키워드, 글 스타일, 분량",
            "outputSummary": "제목 후보·본문·해시태그가 담긴 블로그 초안",
            "costSummary": "구독 포함 · API 키·추가 요금 없음",
            "notice": "자동으로 발행하지 않습니다. 완성된 초안을 복사해 블로그 편집기에 붙여 넣으세요.",
            "resume": {
                "employeeNo": "ILSON-2026-B01", "experience": "6년차", "hometown": "강원 속초",
                "formerRole": "지역 매거진 객원 에디터",
                "intro": "동네 가게 기사를 매주 세 편씩 마감하며, 사장님 말투를 살리면서도 검색에 걸리는 글을 쓰는 법을 익혔습니다. 과장 없이 가게의 진짜 장점을 찾아 문장으로 옮기는 일을 가장 좋아합니다.",
                "career": [
                    { "period": "2020 – 2022", "org": "속초 생활매거진", "note": "상권 탐방 연재 120회" },
                    { "period": "2022 – 2025", "org": "로컬 브랜드 콘텐츠 스튜디오", "note": "소상공인 블로그 대행 300편" },
                    { "period": "2026 –", "org": "일손 콘텐츠팀", "note": "블로그 초안 담당" }
                ],
                "skills": [
                    { "label": "키워드 배치", "score": 5 }, { "label": "소제목 설계", "score": 5 },
                    { "label": "말투 맞춤", "score": 4 }, { "label": "마무리 문구", "score": 4 },
                    { "label": "마감 속도", "score": 5 }
                ],
                "reference": { "quote": "초안이 오면 고칠 데가 거의 없어요. 우리 가게 말투 그대로라 손님들이 놀랍니다.", "from": "제주 베이커리 사장님" },
                "interviewNote": "확인 안 된 가격이나 후기는 절대 지어내지 않고 [확인 필요]로 남긴다고 답함."
            },
            "steps": steps_json(safe, "harin"),
            "form": [
                { "title": "필수 입력", "description": "키워드와 스타일만 정하면 바로 맡길 수 있습니다.", "fields": [
                    { "id": "keyword", "kind": "text", "label": "핵심 키워드", "required": true, "placeholder": "예: 성수동 브런치 카페", "maxLength": 60 },
                    { "id": "template", "kind": "choice", "variant": "cards", "label": "글 스타일", "default": "info", "choices": [
                        { "value": "info", "label": "정보 정리형", "hint": "목차와 비교표로 궁금증을 풀어 줍니다.", "example": ["1. 이런 분께 맞아요", "2. 가격·시간 한눈에 보기", "3. 자주 묻는 질문"] },
                        { "value": "review", "label": "경험·후기형", "hint": "방문 전후 이야기로 신뢰를 쌓습니다.", "example": ["처음 문을 열었을 때", "직접 써 보니 달랐던 점", "이런 분께 추천해요"] },
                        { "value": "consult", "label": "상담 유도형", "hint": "고민을 짚고 문의로 이어지게 합니다.", "example": ["이런 고민 있으셨죠?", "해결 방법 세 가지", "편하게 문의 주세요"] },
                        { "value": "list", "label": "추천 리스트형", "hint": "고르는 기준과 추천 목록으로 정리합니다.", "example": ["고르는 기준 3가지", "상황별 추천 5선", "한 줄 정리"] }
                    ]}
                ]},
                { "title": "자주 쓰는 설정", "fields": [
                    { "id": "length", "kind": "select", "label": "분량", "default": "1500", "choices": [
                        { "value": "800", "label": "짧게 · 약 800자" },
                        { "value": "1500", "label": "보통 · 약 1,500자" },
                        { "value": "2500", "label": "길게 · 약 2,500자" }
                    ]},
                    { "id": "subKeywords", "kind": "text", "label": "보조 키워드", "placeholder": "쉼표로 구분 · 예: 주차, 단체석, 디저트" },
                    { "id": "cta", "kind": "text", "label": "마무리 행동 유도", "placeholder": "예: 네이버 예약에서 10% 할인 받기", "visibleWhen": { "field": "template", "oneOf": ["consult", "info", "list"] } }
                ]},
                { "title": "고급 설정", "advanced": true, "fields": [
                    { "id": "audience", "kind": "text", "label": "읽는 사람", "placeholder": "예: 주말 데이트 장소를 찾는 20대" },
                    { "id": "facts", "kind": "textarea", "label": "꼭 들어갈 사실", "placeholder": "영업시간, 가격, 위치, 이벤트 등 확인된 정보만 적어 주세요." },
                    { "id": "useBrandStyle", "kind": "checkbox", "label": "설정에 저장한 브랜드 문체 반영", "default": true }
                ]}
            ]
        }),
        json!({
            "id": "doyun", "name": "도윤", "role": "견적 담당", "team": "경영지원팀",
            "mode": "local", "hue": 210,
            "summary": "작업 항목과 금액을 받아 부가세 계산까지 끝낸 견적서를 만들고 PDF로 인쇄할 수 있게 정리합니다.",
            "motto": "숫자는 줄 맞춰, 문장은 정중하게.",
            "capabilities": ["견적서 작성", "부가세 자동 계산", "한글 금액 표기", "PDF 인쇄"],
            "inputSummary": "받는 곳, 작업 항목, 수량·단가, 유의사항",
            "outputSummary": "발송 전 검토용 견적서와 인쇄 화면",
            "costSummary": "무료 · 이 PC에서 바로 계산",
            "resume": {
                "employeeNo": "ILSON-2026-Q02", "experience": "9년차", "hometown": "경북 안동",
                "formerRole": "인테리어 사무소 경리",
                "intro": "현장 견적을 하루 열 건씩 정리하던 경리 출신입니다. 단가 한 줄, 부가세 한 원까지 맞아야 마음이 놓이는 성격이라 숫자가 틀린 견적서는 내보내지 않습니다.",
                "career": [
                    { "period": "2017 – 2021", "org": "안동 인테리어 사무소", "note": "현장 견적·정산" },
                    { "period": "2021 – 2025", "org": "디자인 에이전시 경영지원", "note": "견적·계약 문서 관리" },
                    { "period": "2026 –", "org": "일손 경영지원팀", "note": "견적서 담당" }
                ],
                "skills": [
                    { "label": "견적서 작성", "score": 5 }, { "label": "부가세 계산", "score": 5 },
                    { "label": "항목 정리", "score": 5 }, { "label": "비고 문구", "score": 4 },
                    { "label": "인쇄 서식", "score": 4 }
                ],
                "reference": { "quote": "견적서 보내기 전에 다시 계산기 두드릴 일이 없어졌습니다.", "from": "공방 운영 대표" },
                "interviewNote": "금액은 사장님이 정하고, 본인은 계산과 서식만 책임진다고 선을 그음."
            },
            "steps": steps_json(safe, "doyun"),
            "form": [
                { "title": "받는 곳", "fields": [
                    { "id": "clientName", "kind": "text", "label": "받는 곳", "required": true, "placeholder": "예: 주식회사 하늘상사" },
                    { "id": "clientContact", "kind": "text", "label": "담당자·연락처", "placeholder": "예: 김민지 팀장 · minji@example.com" },
                    { "id": "projectName", "kind": "text", "label": "건명", "placeholder": "예: 홈페이지 개편 작업" }
                ]},
                { "title": "작업 항목", "fields": [
                    { "id": "items", "kind": "items", "label": "작업 항목", "required": true, "default": [
                        { "name": "기본 디자인", "spec": "메인 시안 1종", "qty": 1, "unitPrice": 300000 },
                        { "name": "수정 대응", "spec": "2회 포함", "qty": 1, "unitPrice": 50000 }
                    ]},
                    { "id": "vatMode", "kind": "choice", "variant": "chips", "label": "부가세", "default": "separate", "choices": [
                        { "value": "separate", "label": "부가세 별도 (10% 추가)" },
                        { "value": "included", "label": "부가세 포함 금액" },
                        { "value": "exempt", "label": "면세" }
                    ]}
                ]},
                { "title": "기본 정보", "fields": [
                    { "id": "quoteDate", "kind": "date", "label": "견적일", "defaultToday": true },
                    { "id": "validDays", "kind": "number", "label": "유효 기간(일)", "default": 30, "min": 1, "max": 365 },
                    { "id": "delivery", "kind": "text", "label": "납기 예정", "placeholder": "예: 계약 후 3주" }
                ]},
                { "title": "고급 설정", "advanced": true, "fields": [
                    { "id": "notes", "kind": "textarea", "label": "유의사항", "placeholder": "한 줄에 하나씩 적어 주세요." },
                    { "id": "message", "kind": "textarea", "label": "전달 말씀", "placeholder": "견적서 끝에 들어갈 인사말" }
                ]}
            ]
        }),
        json!({
            "id": "seojin", "name": "서진", "role": "자료조사원", "team": "리서치팀",
            "mode": "ai_local", "hue": 158,
            "summary": "링크와 웹 검색으로 공개 자료를 모아, 근거 링크가 붙은 조사 브리프로 정리합니다.",
            "motto": "출처가 없으면 결론도 없습니다.",
            "capabilities": ["웹 검색 조사", "링크 본문 읽기", "출처 연결 브리프", "다음 행동 제안"],
            "inputSummary": "조사 주제, 참고 링크, 조사 목적",
            "outputSummary": "출처 번호가 달린 조사 브리프",
            "costSummary": "구독 포함 · API 키·추가 요금 없음",
            "resume": {
                "employeeNo": "ILSON-2026-R03", "experience": "7년차", "hometown": "전북 전주",
                "formerRole": "도서관 정보봉사 사서",
                "intro": "이용자 질문 하나에 자료 열 개를 찾아 드리던 사서 출신입니다. 확인된 사실과 추정을 꼭 나눠 적고, 모든 문장 뒤에 어디서 왔는지 번호를 붙입니다.",
                "career": [
                    { "period": "2019 – 2022", "org": "전주 시립도서관", "note": "정보봉사·자료 큐레이션" },
                    { "period": "2022 – 2025", "org": "마케팅 리서치 회사", "note": "경쟁사·트렌드 보고서" },
                    { "period": "2026 –", "org": "일손 리서치팀", "note": "조사 브리프 담당" }
                ],
                "skills": [
                    { "label": "자료 검색", "score": 5 }, { "label": "출처 검증", "score": 5 },
                    { "label": "비교표 정리", "score": 4 }, { "label": "핵심 요약", "score": 5 },
                    { "label": "다음 행동 제안", "score": 4 }
                ],
                "reference": { "quote": "경쟁사 조사를 맡기면 링크까지 다 붙어 와서 회의 자료로 바로 씁니다.", "from": "교육 스타트업 마케터" },
                "interviewNote": "자료가 부족하면 결론을 부풀리지 않고 '근거 부족'이라고 쓰겠다고 함."
            },
            "steps": steps_json(safe, "seojin"),
            "form": [
                { "title": "필수 입력", "description": "참고 링크를 넣거나 웹 검색을 켜 주세요.", "fields": [
                    { "id": "topic", "kind": "text", "label": "조사 주제", "required": true, "placeholder": "예: 2026 하반기 필라테스 학원 가격 정책 비교" },
                    { "id": "purpose", "kind": "choice", "variant": "chips", "label": "조사 목적", "default": "compare", "choices": [
                        { "value": "compare", "label": "경쟁사 비교" },
                        { "value": "trend", "label": "시장·트렌드" },
                        { "value": "reference", "label": "레퍼런스 수집" },
                        { "value": "factcheck", "label": "사실 확인" }
                    ]},
                    { "id": "urls", "kind": "textarea", "label": "참고 링크", "placeholder": "한 줄에 하나씩 · 최대 8개", "hint": "이 PC가 공개 페이지를 직접 읽어 본문을 AI에게 전달합니다." }
                ]},
                { "title": "자주 쓰는 설정", "fields": [
                    web_search_field("6", "웹 검색"),
                    { "id": "focus", "kind": "textarea", "label": "꼭 알고 싶은 점", "placeholder": "예: 가격대, 체험 수업 여부, 후기에서 반복되는 불만" }
                ]}
            ]
        }),
        json!({
            "id": "minjae", "name": "민재", "role": "영업 개척", "team": "영업팀",
            "mode": "ai", "hue": 262,
            "summary": "우리 상품에 맞는 잠재 고객을 공개 자료에서 찾아 우선순위와 첫 연락 문구까지 정리합니다.",
            "motto": "거절은 기록하고, 가능성은 다시 두드립니다.",
            "capabilities": ["타깃 고객 정의", "잠재 고객 탐색", "우선순위 정리", "첫 연락 문구"],
            "inputSummary": "파는 상품, 찾는 고객, 지역, 연락 채널",
            "outputSummary": "근거 링크가 붙은 후보 목록과 연락 문구",
            "costSummary": "구독 포함 · API 키·추가 요금 없음",
            "notice": "연락은 자동으로 보내지 않습니다. 정리된 문구를 보고 사장님이 직접 연락하세요.",
            "resume": {
                "employeeNo": "ILSON-2026-S04", "experience": "8년차", "hometown": "충남 천안",
                "formerRole": "B2B 장비 영업사원",
                "intro": "하루 서른 곳에 전화를 돌리던 영업사원 시절, 무작정 연락보다 '왜 이 고객인가'를 먼저 적는 사람이 계약을 따낸다는 걸 배웠습니다. 후보마다 이유와 근거를 함께 드립니다.",
                "career": [
                    { "period": "2018 – 2021", "org": "산업 장비 유통사", "note": "신규 거래처 개척" },
                    { "period": "2021 – 2025", "org": "SaaS 스타트업 세일즈팀", "note": "인바운드·아웃바운드 리드 관리" },
                    { "period": "2026 –", "org": "일손 영업팀", "note": "잠재 고객 탐색 담당" }
                ],
                "skills": [
                    { "label": "타깃 정의", "score": 5 }, { "label": "후보 탐색", "score": 4 },
                    { "label": "우선순위 판단", "score": 5 }, { "label": "연락 문구", "score": 4 },
                    { "label": "끈기", "score": 5 }
                ],
                "reference": { "quote": "누구한테 먼저 연락할지 이유까지 적어 줘서 첫 통화가 편해졌어요.", "from": "세무사 사무소 실장" },
                "interviewNote": "공개되지 않은 개인 연락처는 수집하지 않는다는 원칙을 먼저 말함."
            },
            "steps": steps_json(safe, "minjae"),
            "form": [
                { "title": "필수 입력", "fields": [
                    { "id": "offering", "kind": "text", "label": "우리가 파는 것", "required": true, "placeholder": "예: 학원용 출결 관리 앱" },
                    { "id": "target", "kind": "text", "label": "찾는 고객", "required": true, "placeholder": "예: 수강생 50명 이하 보습학원 원장님" },
                    { "id": "region", "kind": "text", "label": "지역", "placeholder": "예: 경기 수원시 (비우면 전국)" }
                ]},
                { "title": "자주 쓰는 설정", "fields": [
                    { "id": "channel", "kind": "choice", "variant": "chips", "label": "첫 연락 채널", "default": "email", "choices": [
                        { "value": "email", "label": "이메일" },
                        { "value": "blog", "label": "블로그 이웃·댓글" },
                        { "value": "instagram", "label": "인스타그램 DM" },
                        { "value": "call", "label": "전화·방문" }
                    ]},
                    { "id": "count", "kind": "select", "label": "후보 수", "default": "10", "choices": [
                        { "value": "5", "label": "5곳" }, { "value": "10", "label": "10곳" }, { "value": "20", "label": "20곳" }
                    ]},
                    web_search_field("6", "웹 검색")
                ]},
                { "title": "고급 설정", "advanced": true, "fields": [
                    { "id": "candidates", "kind": "textarea", "label": "이미 가진 후보 목록", "placeholder": "상호나 링크를 한 줄에 하나씩 · 우선순위만 매겨 드립니다." },
                    { "id": "strength", "kind": "textarea", "label": "우리만의 강점", "placeholder": "예: 설치 당일 교육, 첫 달 무료" }
                ]}
            ]
        }),
        json!({
            "id": "sua", "name": "수아", "role": "사무 정리", "team": "경영지원팀",
            "mode": "local", "hue": 330,
            "summary": "어지러운 폴더의 파일을 종류·날짜별로 나눌 계획을 먼저 보여 드리고, 확인하시면 옮깁니다.",
            "motto": "찾는 데 쓰던 시간을 일하는 시간으로.",
            "capabilities": ["폴더 파일 분류", "변경 전 미리보기", "이름 충돌 방지", "한 번에 되돌리기"],
            "inputSummary": "정리할 폴더, 정리 기준",
            "outputSummary": "정리 보고서와 되돌리기 기록",
            "costSummary": "무료 · 파일은 이 PC 밖으로 나가지 않음",
            "notice": "파일을 지우지 않습니다. 옮기기 전에 계획을 보여 드리고, 끝난 뒤에도 되돌릴 수 있습니다.",
            "resume": {
                "employeeNo": "ILSON-2026-O05", "experience": "10년차", "hometown": "인천 연수",
                "formerRole": "법무법인 문서 관리 담당",
                "intro": "서류 수만 건의 이름 규칙을 만들고 지켜 온 문서 관리자 출신입니다. 정리는 지우는 게 아니라 제자리를 찾아 주는 일이라고 생각해서, 무엇을 어디로 옮길지 꼭 먼저 보여 드립니다.",
                "career": [
                    { "period": "2016 – 2020", "org": "인천 법무법인", "note": "소송 서류 보관·색인" },
                    { "period": "2020 – 2025", "org": "공유오피스 운영사", "note": "입주사 문서·자료실 관리" },
                    { "period": "2026 –", "org": "일손 경영지원팀", "note": "파일 정리 담당" }
                ],
                "skills": [
                    { "label": "분류 기준 설계", "score": 5 }, { "label": "이름 충돌 처리", "score": 5 },
                    { "label": "변경 기록", "score": 5 }, { "label": "되돌리기", "score": 4 },
                    { "label": "꼼꼼함", "score": 5 }
                ],
                "reference": { "quote": "바탕화면 파일 400개가 5분 만에 제자리로 갔어요. 무서울 줄 알았는데 미리 보여 줘서 안심.", "from": "1인 쇼핑몰 대표" },
                "interviewNote": "확인 없이 파일을 옮기는 일은 하지 않겠다고 세 번 강조함."
            },
            "steps": steps_json(safe, "sua"),
            "form": [
                { "title": "필수 입력", "fields": [
                    { "id": "folder", "kind": "folder", "label": "정리할 폴더", "required": true, "hint": "폴더 바로 안의 파일만 정리합니다. 하위 폴더는 건드리지 않습니다." },
                    { "id": "rule", "kind": "choice", "variant": "cards", "label": "정리 기준", "default": "type", "choices": [
                        { "value": "type", "label": "종류별", "hint": "문서·이미지·영상·압축 등으로 나눕니다.", "example": ["문서/견적서.pdf", "이미지/메뉴판.png", "압축/자료.zip"] },
                        { "value": "month", "label": "월별", "hint": "파일을 마지막으로 고친 달로 나눕니다.", "example": ["2026-09/견적서.pdf", "2026-10/메뉴판.png"] },
                        { "value": "type_month", "label": "종류별 + 월별", "hint": "종류 폴더 안을 다시 달별로 나눕니다.", "example": ["문서/2026-10/견적서.pdf", "이미지/2026-09/메뉴판.png"] }
                    ]}
                ]}
            ]
        }),
        json!({
            "id": "jiho", "name": "지호", "role": "숏폼 작가", "team": "콘텐츠팀",
            "mode": "ai", "hue": 42,
            "summary": "릴스·쇼츠용 대본을 타깃별 여러 안으로 써서, 장면·자막·대사까지 표로 정리합니다.",
            "motto": "첫 2초에 멈추게, 마지막 2초에 움직이게.",
            "capabilities": ["후킹 첫 문장", "장면별 대본·자막", "타깃별 여러 안", "해시태그·촬영 팁"],
            "inputSummary": "주제, 목적, 플랫폼, 길이",
            "outputSummary": "장면 표가 담긴 숏폼 대본 1~3안",
            "costSummary": "구독 포함 · API 키·추가 요금 없음",
            "resume": {
                "employeeNo": "ILSON-2026-V06", "experience": "5년차", "hometown": "광주 동구",
                "formerRole": "라디오 광고 카피라이터",
                "intro": "15초 라디오 광고를 쓰며 첫 문장이 전부라는 걸 배웠습니다. 같은 상품도 누구에게 말하느냐에 따라 첫 마디를 바꿔, 여러 안을 나란히 비교해 보실 수 있게 드립니다.",
                "career": [
                    { "period": "2021 – 2023", "org": "지역 방송국 광고팀", "note": "라디오 CM 카피 400편" },
                    { "period": "2023 – 2025", "org": "숏폼 콘텐츠 에이전시", "note": "브랜드 릴스 기획" },
                    { "period": "2026 –", "org": "일손 콘텐츠팀", "note": "숏폼 대본 담당" }
                ],
                "skills": [
                    { "label": "후킹 문장", "score": 5 }, { "label": "장면 구성", "score": 4 },
                    { "label": "타깃 분화", "score": 5 }, { "label": "자막 리듬", "score": 4 },
                    { "label": "트렌드 감각", "score": 4 }
                ],
                "reference": { "quote": "세 안을 동시에 찍어 올렸더니 어떤 문장이 먹히는지 바로 보였어요.", "from": "네일숍 원장님" },
                "interviewNote": "유행어보다 사장님 가게에서 실제로 찍을 수 있는 장면만 쓰겠다고 함."
            },
            "steps": steps_json(safe, "jiho"),
            "form": [
                { "title": "필수 입력", "fields": [
                    { "id": "topic", "kind": "text", "label": "주제", "required": true, "placeholder": "예: 가을 신메뉴 밤 라떼 출시" },
                    { "id": "objective", "kind": "choice", "variant": "cards", "label": "목적", "default": "visit", "choices": [
                        { "value": "save", "label": "저장 유도", "hint": "꿀팁형으로 저장·공유를 부릅니다.", "example": ["후킹: 이거 모르면 손해예요", "전개: 3가지 팁", "CTA: 저장해 두세요"] },
                        { "value": "inquiry", "label": "문의 전환", "hint": "고민을 짚고 상담 문의로 이어 갑니다.", "example": ["후킹: 혹시 이런 고민?", "전개: 해결 전후", "CTA: 프로필 링크로 문의"] },
                        { "value": "visit", "label": "방문·구매", "hint": "보는 순간 가 보고 싶게 만듭니다.", "example": ["후킹: 첫 모금 클로즈업", "전개: 메뉴·가격·위치", "CTA: 이번 주말 오세요"] },
                        { "value": "awareness", "label": "브랜드 알리기", "hint": "사장님과 가게의 이야기를 보여 줍니다.", "example": ["후킹: 새벽 5시의 주방", "전개: 고집하는 한 가지", "CTA: 팔로우"] }
                    ]}
                ]},
                { "title": "자주 쓰는 설정", "fields": [
                    { "id": "platform", "kind": "select", "label": "플랫폼", "default": "reels", "choices": [
                        { "value": "reels", "label": "인스타그램 릴스" }, { "value": "shorts", "label": "유튜브 쇼츠" }, { "value": "tiktok", "label": "틱톡" }
                    ]},
                    { "id": "duration", "kind": "select", "label": "길이", "default": "30", "choices": [
                        { "value": "15", "label": "15초" }, { "value": "30", "label": "30초" }, { "value": "60", "label": "60초" }
                    ]},
                    { "id": "variants", "kind": "select", "label": "대본 수", "default": "3", "choices": [
                        { "value": "1", "label": "1안" }, { "value": "2", "label": "2안 (A/B)" }, { "value": "3", "label": "3안 (A/B/C)" }
                    ]}
                ]},
                { "title": "고급 설정", "advanced": true, "fields": [
                    { "id": "target", "kind": "text", "label": "보는 사람", "placeholder": "예: 퇴근길 직장인" },
                    { "id": "reference", "kind": "textarea", "label": "참고하고 싶은 점", "placeholder": "좋았던 영상의 구성, 꼭 넣을 장면 등" }
                ]}
            ]
        }),
        json!({
            "id": "yena", "name": "예나", "role": "지원사업 분석", "team": "경영지원팀",
            "mode": "ai_local", "hue": 96,
            "summary": "정부·지자체 지원사업 공고를 읽고, 우리 회사가 신청할 수 있는지와 준비할 것을 정리합니다.",
            "motto": "어려운 공고문 속에서 우리 회사 몫을 찾아 드립니다.",
            "capabilities": ["공고 핵심 요약", "신청 자격 점검", "준비 서류 목록", "사업계획서 목차"],
            "inputSummary": "공고 링크 또는 본문, 회사 기본 정보",
            "outputSummary": "자격 점검표와 준비 체크리스트",
            "costSummary": "구독 포함 · API 키·추가 요금 없음",
            "notice": "최종 신청 자격은 반드시 주관기관 공고문과 담당자에게 확인하세요.",
            "resume": {
                "employeeNo": "ILSON-2026-G07", "experience": "7년차", "hometown": "대전 유성",
                "formerRole": "창업지원센터 사업 담당",
                "intro": "창업지원센터에서 공고문을 쓰는 쪽에 있었습니다. 그래서 심사위원이 어디를 먼저 보는지, 사장님들이 어떤 서류에서 자주 떨어지는지 압니다. 어려운 말을 쉬운 말로 바꿔 드립니다.",
                "career": [
                    { "period": "2019 – 2022", "org": "대전 창업지원센터", "note": "지원사업 공고·접수 운영" },
                    { "period": "2022 – 2025", "org": "경영 컨설팅사", "note": "사업계획서 검토 200건" },
                    { "period": "2026 –", "org": "일손 경영지원팀", "note": "지원사업 분석 담당" }
                ],
                "skills": [
                    { "label": "공고 해석", "score": 5 }, { "label": "자격 점검", "score": 5 },
                    { "label": "서류 체크", "score": 4 }, { "label": "계획서 목차", "score": 4 },
                    { "label": "쉬운 말 설명", "score": 5 }
                ],
                "reference": { "quote": "자격이 되는지 애매했던 부분을 표로 짚어 줘서 담당자 문의가 한 번에 끝났어요.", "from": "식품 제조 스타트업 대표" },
                "interviewNote": "공고에 없는 내용은 추측하지 않고 '확인 필요'로 표시하겠다고 함."
            },
            "steps": steps_json(safe, "yena"),
            "form": [
                { "title": "공고", "description": "공고 링크나 본문 중 하나는 꼭 넣어 주세요.", "fields": [
                    { "id": "noticeUrls", "kind": "textarea", "label": "공고 링크", "placeholder": "한 줄에 하나씩 · 예: 기업마당, K-Startup 공고 주소" },
                    { "id": "noticeText", "kind": "textarea", "label": "공고 본문 붙여넣기", "placeholder": "링크가 열리지 않으면 공고 내용을 그대로 붙여 넣어 주세요." }
                ]},
                { "title": "우리 회사", "fields": [
                    { "id": "bizAge", "kind": "select", "label": "업력", "default": "1-3", "choices": [
                        { "value": "pre", "label": "예비창업" }, { "value": "0-1", "label": "1년 미만" },
                        { "value": "1-3", "label": "1~3년" }, { "value": "3-7", "label": "3~7년" }, { "value": "7+", "label": "7년 이상" }
                    ]},
                    { "id": "headcount", "kind": "select", "label": "상시 근로자", "default": "1-4", "choices": [
                        { "value": "0", "label": "대표 1인" }, { "value": "1-4", "label": "1~4명" },
                        { "value": "5-9", "label": "5~9명" }, { "value": "10+", "label": "10명 이상" }
                    ]},
                    { "id": "revenue", "kind": "select", "label": "작년 매출", "default": "lt1", "choices": [
                        { "value": "none", "label": "매출 없음" }, { "value": "lt1", "label": "1억 미만" },
                        { "value": "1-10", "label": "1억~10억" }, { "value": "10+", "label": "10억 이상" }
                    ]},
                    { "id": "extra", "kind": "textarea", "label": "보유 인증·특이사항", "placeholder": "예: 벤처기업 인증, 여성기업, 특허 1건, 청년 대표" }
                ]},
                { "title": "고급 설정", "advanced": true, "fields": [
                    web_search_field("0", "관련 공고 웹 검색")
                ]}
            ]
        }),
    ]
    .into_iter()
    .chain(dev_team(safe))
    .collect()
}

fn folder_field() -> Value {
    json!({
        "id": "folder", "kind": "folder", "label": "작업 폴더 (git 저장소)", "required": true,
        "hint": "업무마다 별도 작업 공간(git worktree)에서 일합니다. 사장님이 병합하기 전까지 원래 폴더는 그대로입니다."
    })
}

fn optional_folder_field() -> Value {
    json!({
        "id": "folder", "kind": "folder", "label": "작업 폴더 (비워 두면 새 프로젝트 폴더 자동 생성)", "required": false,
        "hint": "비워 두면 ‘내 문서/일손_프로젝트’ 아래에 새 폴더와 git 저장소를 자동으로 만들어 시작합니다. 기존 프로젝트를 수정하려면 폴더를 골라 주세요."
    })
}

fn permission_section(allow_run: bool) -> Value {
    let mut fields = vec![];
    if allow_run {
        fields.push(json!({ "id": "allowRun", "kind": "checkbox", "label": "테스트·빌드 명령 실행 허용 (OpenCode 안에서)", "default": true }));
    }
    fields.push(json!({ "id": "allowInit", "kind": "checkbox", "label": "git 저장소가 아니면 새로 만들어 시작", "default": true }));
    json!({ "title": "실행 권한", "advanced": true, "fields": fields })
}

fn dev_resume(no: &str, exp: &str, hometown: &str, former: &str, intro: &str, career: [(&str, &str, &str); 3], skills: [(&str, u8); 5], reference: (&str, &str), note: &str) -> Value {
    json!({
        "employeeNo": no, "experience": exp, "hometown": hometown, "formerRole": former, "intro": intro,
        "career": career.iter().map(|(p, o, n)| json!({ "period": p, "org": o, "note": n })).collect::<Vec<_>>(),
        "skills": skills.iter().map(|(l, s)| json!({ "label": l, "score": s })).collect::<Vec<_>>(),
        "reference": { "quote": reference.0, "from": reference.1 },
        "interviewNote": note
    })
}

fn dev_team(safe: bool) -> Vec<Value> {
    vec![
        json!({
            "id": "taeo", "name": "태오", "role": "테크 리드", "team": "개발팀", "mode": "ai_local", "hue": 185,
            "summary": "만들고 싶은 프로그램이나 기능을 말하면 서진의 도메인·UX 리서치부터 기획·설계, 건우·유나의 개발, 소희의 QA 테스트, 민혁의 리뷰, 다은의 착수 가이드까지 전 과정을 지휘합니다.",
            "motto": "작게 나누면, 끝까지 갑니다.",
            "capabilities": ["리서치·요구사항 정의", "프로그램 기획·설계(PRD)", "코어·화면 팀원 배정", "QA·리뷰·착수 가이드"],
            "inputSummary": "만들고 싶은 프로그램·기능 (작업 폴더 생략 시 자동 생성)",
            "outputSummary": "리서치·설계서, 완성된 프로그램 코드, QA·리뷰 보고서, 착수 안내서",
            "costSummary": "구독 포함 · API 키·추가 요금 없음",
            "notice": "폴더를 비워 두면 새 프로젝트 폴더를 자동으로 만들어 끝까지 완성합니다. 기존 저장소를 지정하면 별도 작업 공간에서 일한 뒤 병합 확인을 받습니다.",
            "resume": dev_resume("ILSON-2026-D01", "12년차", "경남 창원", "결제 서비스 플랫폼팀 리드",
                "결제 서비스에서 장애 없는 배포를 이끌던 리드 출신입니다. 큰 요청을 하루 안에 끝낼 수 있는 조각으로 나누고, 누가 무엇을 맡을지 먼저 정한 뒤 시작합니다.",
                [("2014 – 2019", "모바일 결제 스타트업", "백엔드 개발"), ("2019 – 2025", "결제 플랫폼 회사", "플랫폼팀 리드"), ("2026 –", "일손 개발팀", "테크 리드")],
                [("작업 분해", 5), ("설계", 5), ("일정 감각", 4), ("코드 리뷰", 4), ("소통", 5)],
                ("요청 한 줄을 주면 설계서가 먼저 와서, 무엇이 바뀔지 미리 알 수 있었어요.", "1인 SaaS 대표"),
                "모르는 요구사항은 추측하지 않고 설계서에 ‘확인 필요’로 남기겠다고 함."),
            "steps": steps_json(safe, "taeo"),
            "form": [
                { "title": "필수 입력", "fields": [
                    optional_folder_field(),
                    { "id": "request", "kind": "textarea", "label": "만들고 싶은 프로그램·기능", "required": true, "placeholder": "예: 메모 프로그램 만들고 싶어 / 할 일 목록에 마감일과 정렬 기능을 추가해 주세요." }
                ]},
                { "title": "자율 공정 (SOP) 구성", "fields": [
                    { "id": "devs", "kind": "choice", "variant": "chips", "label": "개발 담당", "default": "both", "choices": [
                        { "value": "both", "label": "건우(코어) + 유나(화면)" }, { "value": "frontend", "label": "유나 (화면)" }, { "value": "backend", "label": "건우 (서버·로직)" }
                    ]},
                    { "id": "withResearch", "kind": "checkbox", "label": "서진이 사전 도메인·UX 리서치 수행", "default": true },
                    { "id": "withTests", "kind": "checkbox", "label": "소희가 QA·테스트 작성 및 검증", "default": true },
                    { "id": "withDocs", "kind": "checkbox", "label": "다은이 착수 가이드(README) 작성", "default": true }
                ]},
                permission_section(true)
            ]
        }),
        json!({
            "id": "yuna", "name": "유나", "role": "프론트엔드 개발자", "team": "개발팀", "mode": "ai_local", "hue": 285,
            "summary": "화면과 사용자 경험을 만듭니다. 요청한 화면을 구현하고, 민혁의 리뷰를 받은 뒤 병합 대기로 올립니다.",
            "motto": "눌러 보고 싶은 화면이 좋은 화면입니다.",
            "capabilities": ["화면 구현", "컴포넌트 설계", "반응형·접근성", "스타일 정리"],
            "inputSummary": "작업 폴더, 만들 화면",
            "outputSummary": "화면 변경(병합 대기)과 리뷰 의견",
            "costSummary": "구독 포함 · API 키·추가 요금 없음",
            "resume": dev_resume("ILSON-2026-D02", "6년차", "서울 마포", "커머스 앱 프론트엔드 개발자",
                "커머스 앱의 장바구니와 결제 화면을 만들었습니다. 버튼 하나의 위치가 매출을 바꾸는 걸 봐서, 기능만큼 쓰기 편한 화면에 신경 씁니다.",
                [("2020 – 2023", "에이전시", "웹 퍼블리싱·프론트"), ("2023 – 2025", "커머스 앱 회사", "결제 화면 담당"), ("2026 –", "일손 개발팀", "프론트엔드")],
                [("React·TypeScript", 5), ("CSS 레이아웃", 5), ("접근성", 4), ("상태 관리", 4), ("디자인 감각", 4)],
                ("시안 없이 말로만 설명했는데 원하던 화면이 나왔어요.", "카페 예약 서비스 대표"),
                "기존 코드 스타일을 먼저 읽고 맞춰 쓰겠다고 함."),
            "steps": steps_json(safe, "yuna"),
            "form": [
                { "title": "필수 입력", "fields": [
                    folder_field(),
                    { "id": "request", "kind": "textarea", "label": "만들 화면·기능", "required": true, "placeholder": "예: 설정 화면에 다크 모드 토글을 추가해 주세요." }
                ]},
                permission_section(true)
            ]
        }),
        json!({
            "id": "geonwoo", "name": "건우", "role": "백엔드 개발자", "team": "개발팀", "mode": "ai_local", "hue": 120,
            "summary": "서버, 데이터, 핵심 로직을 만듭니다. 구현 뒤 민혁의 리뷰를 받아 병합 대기로 올립니다.",
            "motto": "보이지 않는 곳이 튼튼해야 오래 갑니다.",
            "capabilities": ["API·로직 구현", "데이터 구조", "오류 처리", "성능 점검"],
            "inputSummary": "작업 폴더, 만들 기능",
            "outputSummary": "로직 변경(병합 대기)과 리뷰 의견",
            "costSummary": "구독 포함 · API 키·추가 요금 없음",
            "resume": dev_resume("ILSON-2026-D03", "9년차", "대구 수성", "물류 시스템 백엔드 개발자",
                "하루 수십만 건의 배송 데이터를 처리하는 시스템을 만들었습니다. 예외 상황을 먼저 떠올리고, 실패해도 데이터가 망가지지 않게 짭니다.",
                [("2017 – 2021", "물류 IT 회사", "배송 추적 API"), ("2021 – 2025", "핀테크 회사", "정산 시스템"), ("2026 –", "일손 개발팀", "백엔드")],
                [("API 설계", 5), ("데이터베이스", 5), ("오류 처리", 5), ("성능", 4), ("보안 감각", 4)],
                ("엣지 케이스까지 챙겨서 운영 중에 터지는 일이 없었습니다.", "온라인 클래스 운영사 CTO"),
                "테스트 없이 고친 코드는 고친 게 아니라고 말함."),
            "steps": steps_json(safe, "geonwoo"),
            "form": [
                { "title": "필수 입력", "fields": [
                    folder_field(),
                    { "id": "request", "kind": "textarea", "label": "만들 기능", "required": true, "placeholder": "예: 주문 목록 API에 기간 필터와 페이지 나누기를 추가해 주세요." }
                ]},
                permission_section(true)
            ]
        }),
        json!({
            "id": "sohee", "name": "소희", "role": "QA 엔지니어", "team": "개발팀", "mode": "ai_local", "hue": 355,
            "summary": "테스트를 작성하고 실행해 버그를 찾습니다. 발견한 문제와 보강한 테스트를 병합 대기로 올립니다.",
            "motto": "사장님보다 먼저 버그를 만나는 게 제 일입니다.",
            "capabilities": ["테스트 작성", "테스트 실행", "버그 재현", "품질 보고"],
            "inputSummary": "작업 폴더, 확인할 기능",
            "outputSummary": "테스트 결과 보고와 테스트 코드(병합 대기)",
            "costSummary": "구독 포함 · API 키·추가 요금 없음",
            "resume": dev_resume("ILSON-2026-D04", "7년차", "광주 서구", "게임 회사 QA 리드",
                "출시 전 게임에서 버그 수천 개를 찾아낸 QA 출신입니다. 정상 경로보다 이상한 입력과 경계값을 먼저 넣어 봅니다.",
                [("2019 – 2022", "모바일 게임사", "QA 테스터"), ("2022 – 2025", "게임 회사", "QA 리드·자동화"), ("2026 –", "일손 개발팀", "QA 엔지니어")],
                [("테스트 설계", 5), ("자동화", 4), ("버그 재현", 5), ("경계값 감각", 5), ("보고서", 4)],
                ("배포 전에 소희 님이 잡아 준 버그가 한두 개가 아니에요.", "예약 앱 개발 대표"),
                "실패한 테스트를 지우거나 건너뛰게 고치지 않겠다고 약속함."),
            "steps": steps_json(safe, "sohee"),
            "form": [
                { "title": "필수 입력", "fields": [
                    folder_field(),
                    { "id": "request", "kind": "textarea", "label": "확인할 기능", "required": true, "placeholder": "예: 회원가입 입력 검증 로직의 테스트를 보강하고 실행해 주세요." }
                ]},
                permission_section(true)
            ]
        }),
        json!({
            "id": "minhyuk", "name": "민혁", "role": "코드 리뷰어", "team": "개발팀", "mode": "ai_local", "hue": 235,
            "summary": "변경된 코드를 읽고 버그, 위험, 더 나은 방법을 짚어 줍니다. 코드는 고치지 않고 의견만 드립니다.",
            "motto": "좋은 리뷰는 다음 버그를 막습니다.",
            "capabilities": ["버그 찾기", "보안·위험 점검", "가독성 의견", "통과·수정 판정"],
            "inputSummary": "작업 폴더, 검토 범위",
            "outputSummary": "심각도별 리뷰 의견과 판정",
            "costSummary": "구독 포함 · API 키·추가 요금 없음",
            "notice": "리뷰만 합니다. 저장소의 파일은 바꾸지 않습니다.",
            "resume": dev_resume("ILSON-2026-D05", "11년차", "부산 해운대", "오픈소스 프로젝트 메인테이너",
                "오픈소스 프로젝트에서 수천 건의 풀 리퀘스트를 검토했습니다. 지적만 하지 않고, 왜 위험한지와 어떻게 고치면 되는지를 함께 적습니다.",
                [("2015 – 2020", "포털 서비스", "검색 백엔드"), ("2020 – 2025", "오픈소스 재단", "메인테이너"), ("2026 –", "일손 개발팀", "코드 리뷰어")],
                [("버그 탐지", 5), ("보안", 5), ("설계 감각", 4), ("설명력", 5), ("꼼꼼함", 5)],
                ("리뷰 의견만 읽어도 공부가 됩니다.", "주니어 개발자"),
                "확신이 없는 지적은 ‘확인 필요’로 구분해 적겠다고 함."),
            "steps": steps_json(safe, "minhyuk"),
            "form": [
                { "title": "필수 입력", "fields": [
                    folder_field(),
                    { "id": "scope", "kind": "choice", "variant": "chips", "label": "검토 범위", "default": "uncommitted", "choices": [
                        { "value": "uncommitted", "label": "커밋하지 않은 변경" }, { "value": "last1", "label": "최근 커밋 1개" }, { "value": "last3", "label": "최근 커밋 3개" }
                    ]},
                    { "id": "focus", "kind": "textarea", "label": "특히 볼 점", "placeholder": "예: 결제 금액 계산이 맞는지 중점적으로 봐 주세요." }
                ]}
            ]
        }),
        json!({
            "id": "daeun", "name": "다은", "role": "테크 라이터", "team": "개발팀", "mode": "ai_local", "hue": 65,
            "summary": "README, 사용 설명서, 변경 기록을 처음 보는 사람도 따라 할 수 있게 씁니다.",
            "motto": "설명서가 없으면 기능도 없는 겁니다.",
            "capabilities": ["README 정리", "설치·실행 안내", "API 문서", "변경 기록"],
            "inputSummary": "작업 폴더, 정리할 문서",
            "outputSummary": "문서 변경(병합 대기)",
            "costSummary": "구독 포함 · API 키·추가 요금 없음",
            "resume": dev_resume("ILSON-2026-D06", "8년차", "충북 청주", "SaaS 회사 기술 문서 담당",
                "개발자 문서 사이트를 혼자 운영하며 문의 메일을 절반으로 줄였습니다. 직접 따라 해 보며 막히는 곳부터 고칩니다.",
                [("2018 – 2021", "IT 출판사", "기술서 편집"), ("2021 – 2025", "SaaS 회사", "개발자 문서"), ("2026 –", "일손 개발팀", "테크 라이터")],
                [("README", 5), ("설치 안내", 5), ("쉬운 설명", 5), ("API 문서", 4), ("예제 코드", 4)],
                ("README대로만 하면 한 번에 실행돼요.", "사이드 프로젝트 팀장"),
                "코드에 없는 기능은 문서에 쓰지 않겠다고 함."),
            "steps": steps_json(safe, "daeun"),
            "form": [
                { "title": "필수 입력", "fields": [
                    folder_field(),
                    { "id": "request", "kind": "textarea", "label": "정리할 문서", "required": true, "default": "README를 처음 보는 사람도 설치하고 실행할 수 있게 정리해 주세요." }
                ]},
                permission_section(false)
            ]
        }),
    ]
}

fn steps_json(safe: bool, id: &str) -> Value {
    Value::Array(
        steps(id, safe)
            .into_iter()
            .map(|s| {
                let gate = s.gate.or((safe && is_ai_step(id, s.key)).then_some("handoff"));
                json!({ "key": s.key, "label": s.label, "gate": gate, "actor": s.actor.unwrap_or(id) })
            })
            .collect(),
    )
}

/// AI가 하는 단계인지 (안전 모드에서는 사장님이 OpenCode에서 실행한다).
pub fn is_ai_step(employee_id: &str, key: &str) -> bool {
    match employee_id {
        "harin" | "jiho" => key == "write",
        "seojin" | "yena" => key == "analyze",
        "minjae" => key == "research",
        id if DEV_TEAM.contains(&id) => matches!(key, "team" | "research" | "plan" | "frontend" | "backend" | "build" | "test" | "docs" | "review" | "fix"),
        _ => false,
    }
}

/// 단계를 실제로 하는 직원.
pub fn actor_of(employee_id: &str, step_key: &str, safe: bool) -> String {
    steps(employee_id, safe)
        .iter()
        .find(|s| s.key == step_key)
        .and_then(|s| s.actor)
        .unwrap_or(employee_id)
        .to_string()
}

pub fn find(id: &str) -> Option<Value> {
    all(true).into_iter().find(|e| e["id"] == id)
}

fn visible(field: &Value, input: &Value) -> bool {
    let cond = &field["visibleWhen"];
    if cond.is_null() {
        return true;
    }
    let current = input[cond["field"].as_str().unwrap_or("")].as_str().unwrap_or("");
    cond["oneOf"].as_array().is_some_and(|vals| vals.iter().any(|v| v == current))
}

/// 화면에 보이는 필드만 검사한다: 필수값, 숫자 범위, 날짜 형식.
pub fn validate(employee: &Value, input: &Value) -> Vec<String> {
    let mut problems = vec![];
    for section in employee["form"].as_array().into_iter().flatten() {
        for field in section["fields"].as_array().into_iter().flatten() {
            if !visible(field, input) {
                continue;
            }
            let id = field["id"].as_str().unwrap_or("");
            let label = field["label"].as_str().unwrap_or(id);
            let value = &input[id];
            let text = match value {
                Value::String(s) => s.trim().to_string(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            };
            match field["kind"].as_str().unwrap_or("") {
                "items" => {
                    let filled = value
                        .as_array()
                        .is_some_and(|rows| rows.iter().any(|r| !r["name"].as_str().unwrap_or("").trim().is_empty()));
                    if field["required"] == true && !filled {
                        problems.push(format!("‘{label}’을 한 줄 이상 입력해 주세요."));
                    }
                }
                "number" if !text.is_empty() => match text.parse::<f64>() {
                    Ok(n) => {
                        let min = field["min"].as_f64().unwrap_or(f64::MIN);
                        let max = field["max"].as_f64().unwrap_or(f64::MAX);
                        if n < min || n > max {
                            problems.push(format!("‘{label}’은 {min}~{max} 사이로 입력해 주세요."));
                        }
                    }
                    Err(_) => problems.push(format!("‘{label}’에는 숫자만 입력해 주세요.")),
                },
                "date" if !text.is_empty() => {
                    if chrono::NaiveDate::parse_from_str(&text, "%Y-%m-%d").is_err() {
                        problems.push(format!("‘{label}’ 날짜 형식이 올바르지 않습니다."));
                    }
                }
                _ => {
                    if field["required"] == true && text.is_empty() {
                        problems.push(format!("‘{label}’을 입력해 주세요."));
                    }
                }
            }
        }
    }
    problems
}

/// 한줄 지시문에서 군더더기 어미를 덜어내 깔끔한 업무/프로젝트 제목을 만든다.
pub fn clean_project_title(prompt: &str) -> String {
    let first_line = prompt.lines().next().unwrap_or("").trim();
    let mut s = first_line
        .trim_end_matches(|c: char| matches!(c, '.' | '!' | '?' | '~' | ' '))
        .to_string();
    for tail in [
        " 프로그램을 만들고 싶습니다",
        " 프로그램을 만들고 싶어",
        " 프로그램 만들고 싶습니다",
        " 프로그램 만들고 싶어",
        " 프로그램을 만들어 주세요",
        " 프로그램을 만들어주세요",
        " 프로그램 만들어 주세요",
        " 프로그램 만들어주세요",
        " 프로그램을 만들어줘",
        " 프로그램 만들어줘",
        "을 만들고 싶습니다",
        "를 만들고 싶습니다",
        "을 만들고 싶어",
        "를 만들고 싶어",
        " 만들고 싶습니다",
        " 만들고 싶어",
        "을 만들어 주세요",
        "를 만들어 주세요",
        "을 만들어주세요",
        "를 만들어주세요",
        " 만들어 주세요",
        " 만들어주세요",
        "을 만들어줘",
        "를 만들어줘",
        " 만들어줘",
        " 개발해 주세요",
        " 개발해주세요",
        " 개발해줘",
        " 구현해 주세요",
        " 구현해주세요",
        " 구현해줘",
        " 작성해 주세요",
        " 작성해주세요",
        " 작성해줘",
        " 조사해 주세요",
        " 조사해주세요",
        " 조사해줘",
        " 써 주세요",
        " 써주세요",
        " 써줘",
        " 짜 주세요",
        " 짜주세요",
        " 짜줘",
        " 해 주세요",
        " 해주세요",
        " 해줘",
    ] {
        if let Some(stripped) = s.strip_suffix(tail) {
            let trimmed = stripped.trim();
            if !trimmed.is_empty() {
                if tail.contains("프로그램") && !trimmed.contains("프로그램") && !trimmed.contains("앱") && !trimmed.contains("웹") {
                    s = format!("{trimmed} 프로그램");
                } else {
                    s = trimmed.to_string();
                }
                break;
            }
        }
    }
    let cut: String = s.chars().take(40).collect();
    if s.chars().count() > 40 {
        format!("{cut}…")
    } else if cut.is_empty() {
        "새 자율 프로젝트".to_string()
    } else {
        cut
    }
}

/// 한줄 자동업무 라우터: 사용자의 한 줄 지시를 분석해 가장 알맞은 직원·제목·입력값을 구성한다.
/// `force_program`이 true이거나 프로그램/앱/웹/개발 성격의 요청이면 전사 합동 파이프라인(`taeo`)으로 편성한다.
pub fn route_one_line(prompt: &str, folder: Option<&str>, force_program: bool) -> (String, String, Value) {
    let trimmed = prompt.trim();
    let lower = trimmed.to_lowercase();
    let folder_str = folder.map(str::trim).unwrap_or("");

    let has_dev_keyword = [
        "프로그램", "앱", "웹", "사이트", "어플", "게임", "툴", "도구", "시스템", "페이지", "화면", "기능",
        "개발", "코딩", "코드", "구현", "스크립트", "봇", "계산기", "타이머", "메모", "가계부", "투두",
        "todo", "app", "web", "대시보드", "에디터", "서버", "api", "클론", "만들고", "만들어", "제작",
    ]
    .iter()
    .any(|kw| lower.contains(kw));

    if !force_program && !has_dev_keyword && folder_str.is_empty() {
        if lower.contains("블로그") || lower.contains("포스팅") || lower.contains("네이버 글") {
            let kw = clean_project_title(trimmed);
            return (
                "harin".into(),
                format!("‘{kw}’ 블로그 초안"),
                json!({
                    "keyword": kw,
                    "template": "info",
                    "length": "1500",
                    "facts": trimmed,
                    "useBrandStyle": true
                }),
            );
        }
        if lower.contains("릴스") || lower.contains("쇼츠") || lower.contains("숏폼") || lower.contains("틱톡") || lower.contains("대본") {
            let topic = clean_project_title(trimmed);
            return (
                "jiho".into(),
                format!("{topic} 숏폼 대본"),
                json!({
                    "topic": trimmed,
                    "platform": "reels",
                    "duration": "30",
                    "variants": "3"
                }),
            );
        }
        if lower.contains("잠재 고객") || lower.contains("거래처") || lower.contains("납품처") || lower.contains("영업 후보") {
            let target = clean_project_title(trimmed);
            return (
                "minjae".into(),
                format!("{target} 후보 찾기"),
                json!({
                    "offering": trimmed,
                    "target": target,
                    "count": 10,
                    "webSearch": "6"
                }),
            );
        }
        if lower.contains("시장 조사") || lower.contains("트렌드 조사") || lower.contains("경쟁사 조사") || lower.contains("자료 조사") || lower.contains("리서치") {
            let topic = clean_project_title(trimmed);
            return (
                "seojin".into(),
                format!("{topic} 조사"),
                json!({
                    "topic": trimmed,
                    "purpose": "trend",
                    "webSearch": "6"
                }),
            );
        }
    }

    let base_title = clean_project_title(trimmed);
    let title = if base_title.ends_with("제작") || base_title.ends_with("개발") {
        base_title
    } else {
        format!("{base_title} 제작")
    };
    (
        "taeo".into(),
        title,
        json!({
            "folder": folder_str,
            "request": trimmed,
            "devs": "both",
            "withResearch": true,
            "withTests": true,
            "withDocs": true,
            "allowRun": true,
            "allowInit": true,
            "autoMerge": folder_str.is_empty(),
            "oneLineAuto": true
        }),
    )
}

/// 업무 이름을 비워 두면 입력 내용으로 이름을 짓는다.
pub fn default_title(employee: &Value, input: &Value) -> String {
    let pick = |k: &str| input[k].as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    // 주제가 이미 '조사'로 끝나면 꼬리말을 덧붙이지 않는다.
    let suffixed = |t: String, tail: &str| if t.ends_with(tail) { t } else { format!("{t} {tail}") };
    let subject = match employee["id"].as_str().unwrap_or("") {
        "harin" => pick("keyword").map(|k| format!("‘{k}’ 블로그 초안")),
        "doyun" => pick("clientName").map(|c| suffixed(c, "견적서")),
        "seojin" => pick("topic").map(|t| suffixed(t, "조사")),
        "minjae" => pick("target").map(|t| format!("{t} 후보 찾기")),
        "sua" => pick("folder").map(|f| {
            let name = std::path::Path::new(&f).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(f);
            format!("‘{name}’ 폴더 정리")
        }),
        "jiho" => pick("topic").map(|t| format!("{t} 숏폼 대본")),
        "yena" => Some("지원사업 공고 점검".to_string()),
        "taeo" | "yuna" | "geonwoo" | "sohee" | "daeun" => pick("request").map(|r| clean_project_title(&r)),
        "minhyuk" => Some("코드 리뷰".to_string()),
        _ => None,
    };
    subject.unwrap_or_else(|| format!("{} 새 업무", employee["name"].as_str().unwrap_or("")))
}
