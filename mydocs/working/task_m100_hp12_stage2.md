# Task #12 Stage 2 — 최소 수정 + 시각검증 + 회귀 + tsc (완료 보고서)

- 이슈: [paldyn/HanPage#12](https://github.com/paldyn/HanPage/issues/12) · 브랜치: `local/task12`
- 단계: **Stage 2 (수정·검증)** — Stage 1 확정 원인(오버레이 스크롤바)에 대한 최소 수정.
- 승인 방향: **A (스크롤바 가시성, CSS 최소 수정)** — 작업지시자 추천안 채택. B(`totalWidth` 일관화)는 측정상 무효라 생략.

---

## 1. 수정 내용 (CSS-only, +27줄, 1파일)

`rhwp-studio/src/styles/editor.css` — `#scroll-container` 스크롤바 상시 노출:

- **WebKit/Chromium 계열**(macOS WKWebView·Windows WebView2·웹): `::-webkit-scrollbar`(width/height 14px) + track/thumb/corner 색을 테마 변수로 지정 → 오버레이(자동 숨김)를 무력화하고 **가시 스크롤바 강제**.
- **Firefox**(웹): `scrollbar-width: auto` + `scrollbar-color` 폴백.
- `overflow: auto` **유지** → 콘텐츠가 뷰포트에 들어맞으면 스크롤바는 나타나지 않음(빈 트랙 없음).
- **`virtual-scroll.ts`/`canvas-view.ts` 무수정** → 단일 열·그리드 레이아웃 기하 불변(회귀 불가).

```
 rhwp-studio/src/styles/editor.css | 27 +++++++++++++++++++++++++++
 1 file changed, 27 insertions(+)
```

## 2. 빌드 검증

- `npm run build` (= `tsc && vite build`) **클린**. CSS 번들 정상(`dist/assets/index-*.css`).
- (사전존재 경고 `chunk > 500kB` 는 WASM/JS 크기 관련, 본 변경 무관.)

## 3. 시각검증 (실제 렌더 = headful)

> ⚠️ **headless Chrome 은 `::-webkit-scrollbar` 커스텀 스크롤바를 화면에 그리지 않는다**(headless 렌더 한계). 따라서 수정 효과는 **headful**(실제 창)로 검증했다. headful 은 macOS/Windows 데스크톱 웹뷰의 실제 동작에 더 가깝다.

headful 측정(빈 새 문서, `#scroll-container` 기준):

| 케이스 | 가로 스크롤바 점유(px) | 세로 스크롤바 점유(px) | hasHScroll | clientWidth | 결과 |
|--------|--------:|--------:|:--:|--------:|------|
| **버그 900 / z1.6** (페이지 1269 > 창) | **15** | 15 | true | 885 (=900−15) | **가로 스크롤바 노출 → 가려진 우측 접근 가능** ✓ |
| **중앙 1280 / z1.0** (페이지 793 < 창) | **0** | 15 | false | 1245 (=1260−15) | 가로바 없음, **페이지 중앙정렬 유지** ✓ |

- 스크린샷(`output/poc/task12/`, gitignore):
  - `before_w900_z160.png` — (수정 전) 우측 짤림 + 스크롤바 없음
  - `after_headful_w900_z160.png` — (수정 후) **하단 가로 스크롤바 + 우측 세로 스크롤바 노출**
  - `after_headful_w1280_z100.png` — (수정 후) **페이지 중앙정렬, 세로 스크롤바만** (가로 없음)
- 스크롤 도달성: 버그 케이스에서 `scrollLeft` 0 → 425 도달(가려진 우측 전 영역 접근). ✓

## 4. 그리드 모드 회귀 (수용 기준: 회귀 0)

- 다중 페이지 샘플(`3-11월_실전_통합_2022.hwpx`, 22페이지) 로드 후 뷰포트 1280 / 줌 0.4:
  - `gridMode: true`, **`columns: 3`** — 다중 열 배치 정상.
- 변경이 **CSS-only**(스크롤바 외형)이고 그리드 레이아웃 코드(`virtual-scroll.ts::layoutGrid`)는 손대지 않았으므로 **열 배치 기하 불변**. 스크롤바가 그리드 모드에서도 동일하게 가시화되는 것은 의도된 일관 동작.

## 5. 수용 기준 점검 (계획서 §2)

- [x] 창 > 페이지: 페이지 **중앙정렬 유지** (1280/z1.0 검증)
- [x] 창 < 페이지: **가로 스크롤 생성** → 페이지 전체 접근 (900/z1.6 검증, scrollLeft 425 도달)
- [x] 여러 줌 배율 동일 동작 (z1.0 / z1.6 / z0.4 확인)
- [x] 기존 **그리드 모드** 회귀 0 (columns 3 정상, 레이아웃 코드 무변경)
- [x] `tsc` 클린 (`npm run build` 통과)

## 6. 게이트

- [x] 최소 수정(CSS) 적용 + `tsc`/build 클린
- [x] headful 시각검증(스크롤바 노출 + 중앙정렬 무회귀) + 스크린샷 증빙
- [x] 그리드 모드 무회귀 확인
- [ ] **(승인 대기)** Stage 3 — 최종 보고서 + `main` 머지 PR + (승인) 이슈 #12 클로즈
