# Task #12 — 창 좁힘 시 문서 중앙정렬·가로 스크롤 버그 (최종 결과 보고서)

- 이슈: [paldyn/HanPage#12](https://github.com/paldyn/HanPage/issues/12) · 마일스톤 M100(v1.0.0)
- 브랜치: `local/task12` (origin/main 분기) · 머지 타깃: **`main` PR** (#1/#5/#7 선례)
- PR: [paldyn/HanPage#14](https://github.com/paldyn/HanPage/pull/14) (base `main` ← `task12-scrollbar`)
- 상태: **수정 완료 · 검증 통과 · main PR #14 생성** (머지/이슈 클로즈 승인 대기)

## 1. 증상

창(또는 브라우저 뷰포트)을 문서 페이지보다 좁히면 페이지가 중앙정렬되지 않고 **오른쪽으로 밀려 짤리며, 가로 스크롤바도 보이지 않아** 가려진 영역에 접근 불가. Windows·macOS 데스크톱 공통. 공유 프런트(`rhwp-studio`)라 웹(GitHub Pages)도 동일.

## 2. 정확 원인 (Stage 1 — dev 측정으로 확정)

| 구분 | 내용 |
|------|------|
| **직접 원인** | `#scroll-container { overflow: auto }` 의 스크롤바가 데스크톱 웹뷰(macOS WKWebView·Windows WebView2)·macOS 브라우저에서 **오버레이(자동 숨김)** 로 렌더 → 가로 오버플로(`scrollWidth 1310 > clientWidth 900`)가 있어도 **바가 안 보여** 가려진 영역이 접근 불가로 인지. (프로브: `scrollLeft` 0→410 도달, 스크롤 자체는 정상) |
| 부가 원인 | `layoutSingleColumn()` 의 `totalWidth = maxPageWidth + 40` 이 뷰포트를 무시 → content > container 일 때 `margin:0 auto` 과제약으로 **좌측 정렬**(우측만 짤림). |

**측정으로 계획서 유력 가설을 기각**: `totalWidth = Math.max(maxPageWidth+40, viewportWidth)` 한 줄 패치는 페이지>뷰포트에서 `max()` 가 그대로 `page+40` 을 반환해 **무효(no-op)**. → Stage 1(측정 우선)이 섣부른 패치를 차단.

상세: `mydocs/working/task_m100_hp12_stage1.md`

## 3. 수정 (Stage 2 — CSS 최소 수정)

`rhwp-studio/src/styles/editor.css` — `#scroll-container` 스크롤바 **상시 노출** (1파일 +27줄):

- WebKit/Chromium(WKWebView·WebView2·웹): `::-webkit-scrollbar`(14px) + track/thumb/corner 테마색 → 오버레이 무력화·가시 스크롤바 강제.
- Firefox(웹): `scrollbar-width: auto` + `scrollbar-color` 폴백.
- `overflow: auto` 유지 → 콘텐츠가 들어맞으면 바는 미표시(빈 트랙 없음).
- **`virtual-scroll.ts`/`canvas-view.ts` 무수정** → 단일 열·그리드 레이아웃 기하 불변(회귀 불가). 부가 원인(B)은 측정상 가시 효과 0이라 **미적용**(최소 수정 원칙).

상세: `mydocs/working/task_m100_hp12_stage2.md`

## 4. 검증 결과

> headless Chrome 은 `::-webkit-scrollbar` 커스텀 스크롤바를 그리지 않으므로 **headful**(실제 창)로 검증.

| 케이스 | 가로바 점유 | hasHScroll | 중앙정렬 | 결과 |
|--------|--------:|:--:|:--:|------|
| 창 < 페이지 (900 / z1.6) | 15px | true | — | **가로 스크롤바 노출, scrollLeft 425 도달 → 우측 접근 가능** ✓ |
| 창 > 페이지 (1280 / z1.0) | 0 | false | 유지 | **페이지 중앙정렬** ✓ |
| 그리드 (22p 샘플 / z0.4) | — | — | — | `gridMode true, columns 3` → **회귀 0** ✓ |
| `npm run build`(tsc+vite) | — | — | — | **클린** ✓ |

- 증빙 스크린샷(`output/poc/task12/`, gitignore): `before_w900_z160.png`(수정 전 짤림·바 없음) / `after_headful_w900_z160.png`(수정 후 가로+세로 바) / `after_headful_w1280_z100.png`(중앙정렬+세로바만).
- **수용 기준(계획서 §2) 5개 전부 충족.**

## 5. 영향 범위

- **공유 프런트(`rhwp-studio`) 수정 → 웹·데스크톱 양쪽 개선.** CSS 외형 변경(스크롤바 가시화)으로 레이아웃/엔진 로직 무영향.
- ⚠️ **Pages 배포**: 본 수정이 `main` 머지되면 `deploy-pages.yml`(main push 트리거)로 **다음 웹 배포에 포함**(의도된 개선).
- **데스크톱 반영**: 차기 릴리스 CI 빌드 시(리네임 후 `hanpage-desktop-v*`).
- 무영향: rhwp 엔진 식별자, 데스크톱 셸(#5/#7), CI 워크플로.

## 6. 잔여/후속 (본 타스크 범위 밖)

- 좁은 뷰포트에서 body 레벨 가로 오버플로(scrollWidth≈1039, 도구 상자 min-content 추정) 관측 → 편집 영역 중앙정렬·스크롤과 별개. 필요 시 별도 이슈.

## 7. 커밋

| 커밋 | 내용 |
|------|------|
| `76379491` | 수행계획서 + 오늘할일 |
| `a534dc76` | Stage 1 — 정확 원인 확정(측정) |
| `2b6b291d` | Stage 2 — 스크롤바 상시 노출(editor.css) + 보고서 |
| _(본 커밋)_ | Stage 3 — 최종 보고서 + 오늘할일 갱신 |
