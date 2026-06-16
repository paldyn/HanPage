# Task M100-1158 최종 보고서 — rhwp-studio 다크테마 지원

- 이슈: https://github.com/edwardkim/rhwp/issues/1158
- 브랜치: `local/task_m100_1158`
- 작성일: 2026-06-16
- 기준 브랜치: `upstream/devel`

## 1. 완료 범위

- rhwp-studio에 `system | light | dark` 테마 설정을 추가했다.
- 설정 저장값과 실제 적용값을 분리해
  - 저장값: `system | light | dark`
  - 실제 적용값: `light | dark`
  구조로 정리했다.
- 앱 시작 시 `document.documentElement.dataset.themeMode`,
  `document.documentElement.dataset.themeEffective`,
  `color-scheme`를 반영하도록 했다.
- system 모드에서 `prefers-color-scheme` 변경을 따라가도록 했다.
- 보기 메뉴에 `테마 > 시스템 설정 / 밝게 / 어둡게` 항목을 추가하고 active 상태를 동기화했다.
- `meta[name="theme-color"]`가 현재 테마에 맞게 갱신되도록 했다.
- 메뉴바, 툴바, 서식바, 상태바, 작업영역, command palette, 공통 dialog, 주요 개별 dialog를
  semantic token 기반으로 전환했다.
- dark mode에서도 편집 용지와 눈금자 body는 흰색을 유지하도록 분리했다.
- 눈금자 canvas는 theme 변경 시 palette를 다시 읽고 redraw 하도록 했다.
- 표/셀 선택 오버레이도 token 기반으로 정리했다.
- 테마 스모크 E2E `rhwp-studio/e2e/theme-mode.test.mjs`를 추가했다.

## 2. 비범위 / 유지한 값

- 문서 내용, 인쇄, export SVG/WASM 렌더 색은 테마에 따라 반전하지 않았다.
- `style-bar.css`의 글자색/형광펜 미리보기 막대는 실제 선택 색상 샘플이라 절대색을 유지했다.

## 3. 구현 스테이지

- Stage 1: 테마 설정 저장, DOM dataset 반영, 보기 메뉴 추가, 앱 chrome token화
- Stage 2: 개별 dialog/overlay 색상 정리
- Stage 3: 표 선택 오버레이 token화 및 잔여 절대색 최소화

## 4. 검증

Stage 1 검증:

- `cd rhwp-studio && npm run build`
- `cd rhwp-studio && node e2e/theme-mode.test.mjs --mode=headless`
- in-app browser에서 `http://localhost:7700` 로드 확인

Stage 2 검증:

- `cd rhwp-studio && npm run build`
- `cd rhwp-studio && node e2e/theme-mode.test.mjs --mode=headless`

Stage 3 검증:

- `cd rhwp-studio && npm run build`
- `cd rhwp-studio && node e2e/theme-mode.test.mjs --mode=headless`

## 5. 최종 판단

- theme 설정/저장/새로고침 유지/system 연동은 구현 완료
- dark mode의 주요 UI chrome과 대표 dialog는 token 기반으로 정리 완료
- 편집 용지는 dark mode에서도 흰색 유지
- 남은 절대색은 실제 색상 샘플 성격만 남겨 의도된 값으로 판단
