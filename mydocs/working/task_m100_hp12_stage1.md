# Task #12 Stage 1 — dev 재현 + computed 측정 → 정확 원인 확정 (완료 보고서)

- 이슈: [paldyn/HanPage#12](https://github.com/paldyn/HanPage/issues/12) · 브랜치: `local/task12`
- 단계: **Stage 1 (원인 확정)** — 소스 무수정. dev 서버 재현 + DOM computed 측정.
- 결론: **계획서 §3 유력 가설(단일 열 `totalWidth`)은 실재하나 스크린샷 증상의 직접 원인이 아님.** 직접 원인은 **오버레이(자동 숨김) 스크롤바**로 인해, 기하학적으로 존재하는 가로 오버플로가 **시각적으로 노출되지 않는 것**이다. → "한 줄 패치(`Math.max`)"는 본 증상에 **무효(no-op)** 임을 측정으로 확인. (Stage 1 = 측정 후 수정 원칙의 정당성 입증)

---

## 1. 측정 환경 / 방법

- dev: `cd rhwp-studio && npx vite --port 7700` (headless Chrome, puppeteer-core).
- 절차: 빈 새 문서 생성(`__eventBus.emit('create-new-document')`) → 뷰포트×줌 조합별로 `#scroll-container`/`#scroll-content`/조상 체인 computed 측정 + `scrollLeft` 도달성 프로브 + 스크린샷.
- 임시 스크립트 3종(`_task12_measure.mjs`/`_task12_chain.mjs`/`_task12_scrollprobe.mjs`)은 측정 후 삭제(커밋 제외). 증빙 스크린샷: `output/poc/task12/before_w900_z160.png` (gitignore 영역).

페이지 폭(px) = `pageInfo.width × zoom`. 빈 새 문서 1페이지 기준 원폭 ≈ 793.7px(줌1.0), 1269.9px(줌1.6).

## 2. 측정 결과

### 2-1. 뷰포트×줌 조합 (단일 열, `#scroll-content` 폭 = `maxPageWidth + 40`)

| 케이스 | 뷰포트 | 줌 | 페이지px | content폭 | container clientW | container scrollW | 가로오버플로(기하) | margin L/R | canvas 좌측끝 | 결과 |
|--------|------:|---:|------:|------:|------:|------:|:--:|:--:|------:|------|
| 기준 wide | 1280 | 1.0 | 793.7 | 833.7 | 1260 | 1260 | 없음 | 213/213 | 233.5 | **중앙정렬 OK** |
| wide z1.6 | 1280 | 1.6 | 1269.9 | 1309.9 | 1260 | 1310 | 있음 | 0/0 | 20.5 | 스크롤 필요 |
| **버그 mid** | **900** | **1.6** | 1269.9 | 1309.9 | 900 | 1310 | **있음** | **0 / −409.9** | 20.5 | **우측 짤림 + 스크롤바 미표시** |
| 버그 narrow | 700 | 1.0 | 793.7 | 833.7 | 700 | 834 | 있음 | 0/0 | 20.3 | 우측 짤림 + 스크롤바 미표시 |
| 버그 tiny | 500 | 1.6 | 1269.9 | 1309.9 | 500 | 1310 | 있음 | 0/0 | 20.5 | 우측 짤림 + 스크롤바 미표시 |

- **`#scroll-content` 폭은 뷰포트와 무관하게 항상 `페이지폭 + 40`** (`vs.totalWidth` = `maxPageWidth + 40`로 확인). 뷰포트가 줄어도 그대로 1309.9.
- content > container 일 때 `margin: 0 auto` 가 **과제약(over-constrained)** → `margin-left` 0, `margin-right` 음수(−409.9) 로 해소 → **중앙정렬 붕괴, 좌측 정렬**. (버그 mid 에서 직접 관측)
- 캔버스는 `#scroll-content` 내부에서 `left:50%; translateX(-50%)` 로 중앙 → 캔버스 좌측끝 ≈ 20px, 우측끝 ≈ 1289px → **container(900) 밖 우측 ~389px 짤림**.

### 2-2. 조상 체인 (버그 mid: 900 / 줌1.6)

| 요소 | clientWidth | scrollWidth | overflow-x | 비고 |
|------|------:|------:|:--:|------|
| html / body | 900 | 1039 | (visible) | 별건: 툴바 min-content (§5) |
| `#editor-area` | 900 | 900 | hidden | **자체 오버플로 없음** → 1fr 트랙 blowout 아님, 클리핑 주체 |
| `#scroll-container` | 900 | **1310** | **auto** | **가로 오버플로 존재 → 스크롤 가능** |
| `#scroll-content` | 1310 | — | — | `style.width = 1309.92px` |

→ `#editor-area`(grid `20px 1fr`, overflow hidden)는 뷰포트로 정상 클램프(blowout 아님). 오버플로는 `#scroll-container` 내부에 정상적으로 존재.

### 2-3. 스크롤 도달성 프로브 (버그 mid)

```
clientWidth 900, scrollWidth 1310, maxScrollLeft 410
scrollLeft: 0 → (99999 대입) → 410   reachable: true
```

→ **컨테이너는 실제로 가로 스크롤된다**(scrollLeft 가 최대 410까지 도달). 즉 가려진 우측은 **스크롤로 접근 가능**하다. 문제는 **스크롤바가 보이지 않는 것**뿐.

## 3. 정확 원인 (확정)

증상("우측 짤림 + 스크롤바 안 생김 + 가운데 안 옴")은 **두 결함의 합**이다:

1. **[직접 원인] 오버레이/자동 숨김 스크롤바** — `#scroll-container { overflow: auto }` 는 기하학적으로 오버플로(scrollWidth 1310 > clientWidth 900)를 가지지만, 데스크톱 웹뷰(macOS WKWebView, Windows 11 WebView2) 및 macOS 브라우저의 **오버레이 스크롤바**는 상호작용 전 **자동으로 숨겨지고 레이아웃 공간도 점유하지 않는다** → 사용자에게 "스크롤바가 안 생김 → 가려진 부분 접근 불가"로 인지. (프로브로 스크롤 자체는 정상 동작 확인)
2. **[부가] content > container 시 좌측 정렬** — `layoutSingleColumn()` 이 `totalWidth = maxPageWidth + 40` 로 뷰포트를 무시 → content 가 container 보다 넓어지면 `margin:0 auto` 과제약으로 **좌측 정렬**(우측만 짤림). 스크롤 0 위치에서 페이지 좌측만 보이고 우측이 잘려 보이는 양상의 원인.

## 4. 계획서 §3 가설 재평가 — `Math.max` 한 줄 패치는 본 증상에 **무효**

계획서의 유력 방향: `totalWidth = Math.max(maxPageWidth + 40, viewportWidth)`.

- 페이지 < 뷰포트: `max(833, 1280)=1280` → content=뷰포트. 그러나 **현재도 `margin:0 auto`로 이미 중앙정렬**(기준 wide 케이스 OK) → 가시적 변화 없음.
- **페이지 > 뷰포트(= 스크린샷 케이스)**: `max(1309, 900)=1309` → **현재와 동일**. content 여전히 > container → 좌측 짤림·스크롤바 미표시 그대로. → **증상 미해결(no-op)**.

즉 한 줄 패치는 스크린샷 증상을 고치지 못한다. **Stage 1(측정 우선)이 섣부른 패치를 차단**한 사례.

## 5. 잔여 관찰 (별건, 본 타스크 범위 밖)

- body scrollWidth 1039(>clientWidth 900): 좁은 뷰포트에서 **도구 상자(#icon-toolbar) 등의 min-content 폭**으로 인한 body 레벨 가로 오버플로로 추정. 사용자 스크린샷에서 툴바는 900px에 표시되며, 본 이슈(편집 영역 중앙정렬·스크롤)와 별개 → **본 타스크에서 다루지 않음**(필요 시 별도 이슈).

## 6. Stage 2 수정안 (승인 요청 — 계획서 가설 대비 **방향 수정** 포함)

> ⚠️ 계획서 §3은 `virtual-scroll.ts` 단일 수정으로 가정했으나, 측정 결과 **직접 원인이 스크롤바 가시성(CSS)** 임이 확인되어 수정 대상이 달라진다. 아래 방향으로의 진행 승인을 요청.

- **A. [필수] `#scroll-container` 스크롤바 상시 노출** — `styles/editor.css` 에 `::-webkit-scrollbar`(WKWebView·WebView2·Chromium/Safari 웹 공통 지원) 스타일을 부여해 오버레이 자동 숨김을 무력화하고 **항상 보이는 스크롤바 트랙**을 렌더. Firefox(웹) 대비 `scrollbar-width`/`scrollbar-color` 폴백 병기. → "창 < 페이지 → 가로 스크롤 생성" 수용 기준 직접 충족.
- **B. [방어적/선택] 단일 열 `totalWidth` 를 그리드와 일관화** — `layoutSingleColumn(viewportWidth)` 로 `totalWidth = Math.max(maxPageWidth + 40, viewportWidth)`. content 가 뷰포트보다 좁아지는 경우를 차단해 캔버스 `left:50%` 중앙정렬을 항상 ≥뷰포트 기준으로 안정화(증상 직접 해결은 아니나 일관성·방어). 최소 수정 원칙상 **A만으로 수용 기준 충족** 가능하므로, B 포함 여부는 지시에 따름.
- **회귀 가드**: 그리드 모드(줌 ≤ 0.5 다중 페이지) 무변경 확인, 기준 wide 중앙정렬 무변경 확인, `tsc`/`npm run build` 클린.
- **검증**: 브라우저 리사이즈 시각검증(수정 후 스크롤바 노출 + 우측 접근) + before/after 스크린샷.

## 7. 게이트

- [x] dev 재현 + computed 측정 + 스크롤 도달성 프로브 + 스크린샷
- [x] 정확 원인 확정 (오버레이 스크롤바 = 직접 원인 / 단일열 좌측정렬 = 부가)
- [x] 계획서 유력 가설(`Math.max`) 무효 확인 → 수정 방향 재설정
- [ ] **(승인 대기)** Stage 2 착수 — 수정안 A(필수)/B(선택) 적용 + 시각검증 + 회귀 + `tsc`
