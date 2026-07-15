# Task #41 (M100) — macOS 데스크톱 드래그&드롭 + 콤보 잘림 수정 최종 보고서

- **이슈**: [paldyn/HanPage#41](https://github.com/paldyn/HanPage/issues/41) · **브랜치**: `local/task41`
- 작업지시자 직접 지시("파악해서 수정후 맥버전 배포") — fast path (계획서 생략, 이슈에 조사 기록)

## 버그 1 — 드래그&드롭으로 파일이 열리지 않음

- **원인**: Tauri v2 기본값 `dragDropEnabled: true` → 네이티브가 webview drop을 가로챔 → HTML5 drop 이벤트 불발(웹 drop 핸들러·보안 모달 미실행), Rust 측 처리도 없어 drop 소실. Windows 데스크톱도 동일 원인.
- **수정**: `HanPage-Desktop/src-tauri/tauri.conf.json` main window에 `"dragDropEnabled": false` → HTML5 drop이 웹뷰 도달, 웹과 동일 경로(0.7.16 보안 게이트 포함).
- **검증**: Playwright WebKit E2E — 실제 .hwp로 DataTransfer drop 합성 → 보안 모달 확인 → 문서 열림(1/1쪽) PASS. 로컬 앱 빌드에서 작업지시자 수동 확인 ✅.

## 버그 2 — 서식 도구 모음 콤보 텍스트 잘림 (바탕글/대표/160 %)

- **원인**: `style-bar.css` 고정폭이 Chrome 기준. macOS WKWebView는 네이티브 select 화살표·패딩이 넓어(≈22px) 텍스트 잘림. Safari 웹도 동일.
- **수정**: `#style-name` 60→78px, `.sb-font-lang` 44→58px, `.sb-ls-select` 56→70px.
- **검증**: Playwright WebKit 실측 — 바탕글 32px·대표 20px·160% 33px 모두 새 폭에 여유. 스크린샷·작업지시자 수동 확인 ✅.

## 배포

v0.7.18 태그 이동 재빌드에 포함 (Task #40 Stage 3와 함께 완료).
