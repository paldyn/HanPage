# Task #38 (M100) — 웹 데스크톱 앱 다운로드 버튼 지연 표시 수정 최종 보고서

- **이슈**: [paldyn/HanPage#38](https://github.com/paldyn/HanPage/issues/38)
- **브랜치**: `local/task38` · **커밋**: `345243e6`
- **계획서**: `plans/task_m100_hp38.md`

## 1. 문제

웹 헤더의 "데스크톱 앱 다운로드" 버튼(Task #29)이 페이지 진입 후 몇 초 늦게 나타남.

원인: `rhwp-studio/src/main.ts` `initialize()`에서 버튼 설치가 `await wasm.initialize()`(~12MB WASM 로드) **뒤**에 실행됨. 메뉴 타이틀은 index.html 정적 요소라 즉시 보이는 반면 버튼만 WASM 완료를 대기.

## 2. 수정

`installAppDownloadButton(document.getElementById('menu-bar')!)` 호출을 `initialize()` **최상단**(웹폰트/WASM await 이전)으로 이동. 변경 파일 1개(main.ts), 실질 1줄 이동.

안전 근거: 버튼은 WASM·eventBus·dispatcher 미사용(#menu-bar에 append만) / MenuBar 생성자는 컨테이너 비파괴(`querySelectorAll('.menu-item')` + 리스너 부착) / CSS `margin-left:auto`로 DOM 순서 무관 우측 정렬 / 데스크톱(Tauri)에선 `isDesktopApp()` 조기 반환으로 불변.

## 3. 검증

- **웹 빌드**: `npm run build` (tsc+vite) 통과.
- **Puppeteer 실측** (vite dev, headless Chrome, 버튼 등장 vs 초기화 완료 시각):

| | 버튼 등장 | 초기화 완료 |
|---|---|---|
| 수정 전 (devel) | **3005ms** | 3005ms (동시 = WASM 대기 확정) |
| 수정 후 | **42ms** | 2833ms |

- 라이브(원격 회선)에서는 WASM 다운로드가 더 느려 개선 폭이 더 큼.

## 4. 부수 발견 (환경, 소스 무관)

로컬 `rhwp-studio` 빌드가 이 변경 이전부터 실패 상태였음 — 원인은 스테일 `pkg/`(5/30자 WASM 타입 정의가 0.7.15 엔진과 불일치, `wasm-bridge.ts` TS2554 3건). Docker WASM 재빌드(`docker compose run wasm`)로 해소. CI/Pages는 매번 WASM을 새로 빌드하므로 영향 없음.
