# Task #38 (M100) — 웹 데스크톱 앱 다운로드 버튼 지연 표시 수정 수행계획서

- **이슈**: [paldyn/HanPage#38](https://github.com/paldyn/HanPage/issues/38)
- **브랜치**: `local/task38` (devel `6dd2de33` 분기)

## 1. 현상 / 원인

웹 헤더의 "데스크톱 앱 다운로드" 버튼(Task #29)이 페이지 진입 후 몇 초 늦게 나타남.

`rhwp-studio/src/main.ts` `initialize()` 실행 순서:

| 행 | 내용 | 시점 |
|----|------|------|
| 113 | `await loadWebFonts([])` | 초기 |
| 115 | `await wasm.initialize()` | **~12MB WASM 로드 (수 초)** |
| 190 | `new MenuBar(...)` | WASM 후 |
| **193** | `installAppDownloadButton(...)` | **WASM 후 ← 지연 원인** |

메뉴 타이틀(파일/편집/…)은 `index.html` 정적 요소라 즉시 보이는 반면, 버튼만 JS append라 WASM 완료를 기다림.

## 2. 수정 방향 (1줄 이동)

`installAppDownloadButton(document.getElementById('menu-bar')!)` 호출을 `initialize()` **최상단**(`loadWebFonts` 이전)으로 이동.

**안전 근거**:
- 버튼은 WASM·eventBus·dispatcher 미사용 — `#menu-bar`(정적 존재)에 append만.
- `MenuBar` 생성자는 컨테이너를 교체하지 않음(`querySelectorAll('.menu-item')` + 리스너 부착만) → 먼저 붙은 버튼 보존.
- 버튼 CSS `margin-left:auto` → DOM 순서와 무관하게 우측 정렬 유지.
- Tauri 데스크톱에서는 `isDesktopApp()` 조기 반환 → 데스크톱 동작 불변.

## 3. 작업 단계 (경미 수정 — 단일 단계)

1. main.ts 호출 위치 이동 (1줄 삭제 + 1줄 삽입)
2. 검증: `npm run build`(tsc+vite) + dev 서버에서 버튼 즉시 표시 확인(브라우저), 데스크톱 빌드 경로(`build:desktop`) 회귀 없음 확인
3. 최종 보고 → devel 반영 (웹 라이브는 main merge 후 Pages 재배포)

## 4. 영향 범위

- 변경 파일: `rhwp-studio/src/main.ts` 1곳
- 엔진(Rust)·데스크톱(Tauri)·CI 변경 없음
