# Task #12 (HanPage) — 창 좁힘 시 문서 중앙정렬·가로 스크롤 버그 (수행계획서)

> **파일명 규칙**: fork-repo 이슈 `task_m100_hp{이슈}.md` (hp5/hp7 선례).
> 단일 영역(`rhwp-studio` 레이아웃) 과제라 **수행+구현을 본 문서에 통합**하고 별도 `_impl.md`는 생략한다.

- 이슈: [paldyn/HanPage#12](https://github.com/paldyn/HanPage/issues/12)
- 브랜치: `local/task12` (origin/main 기준 분기)
- 마일스톤: M100 (v1.0.0) — 데스크톱 후속
- 상태: **수행계획 작성 → 승인 요청**

## 1. 배경 / 증상

창(또는 브라우저 뷰포트)을 문서 페이지보다 좁게 줄이면:

- 페이지가 중앙정렬되지 않고 **오른쪽으로 밀려 짤림**
- **가로 스크롤바도 안 생겨** 가려진 부분에 접근 불가

Windows·macOS 데스크톱 모두 재현(작업지시자 스크린샷). 레이아웃 로직이 공유 프런트엔드(`rhwp-studio`)에 있어 **GitHub Pages(웹)에서도 동일 증상으로 추정**된다.

## 2. 목표 / 수용 기준

- [ ] 창 > 페이지: 페이지 **중앙정렬 유지**
- [ ] 창 < 페이지: **가로 스크롤 생성** → 페이지 전체 접근 가능
- [ ] 여러 줌 배율에서 동일 동작
- [ ] 기존 **그리드 모드**(축소 줌 다중 열) 회귀 0
- [ ] `tsc` 클린

## 3. 원인 가설 (Stage 1 에서 dev 재현으로 확정)

단일 열 배치가 뷰포트 폭과 화해하지 않는 것으로 보인다:

| 위치 | 현재 | 비고 |
|------|------|------|
| `src/view/virtual-scroll.ts` `layoutSingleColumn()` | `totalWidth = maxPageWidth + 40` (뷰포트 무시) | 그리드 경로 `layoutGrid()` 는 `Math.max(gridWidth + margins, viewportWidth)` 로 처리 |
| `src/styles/editor.css` | `#scroll-content { margin: 0 auto }` + 캔버스 `position:absolute; left:50%; transform:translateX(-50%)` | 뷰포트 < (줌 적용) 페이지 폭일 때 정렬·스크롤 영역 붕괴 |

**유력 방향(확정 전)**: 단일 열도 그리드처럼 `totalWidth = Math.max(maxPageWidth + 40, viewportWidth)` 로 잡고 `layoutSingleColumn`에 viewportWidth 를 전달.

> 단, 스크린샷의 "페이지가 우측으로 밀려 큰 빈 좌측 + 짤림" 양상이 이 가설만으로 완전히 설명되지 않는다(콘텐츠 폭이 페이지+40 보다 더 커야 성립하는 그림). 따라서 **Stage 1 에서 dev 서버로 실제 `scrollWidth`/`clientWidth`/`#scroll-content` 폭/캔버스 좌표를 측정해 정확 원인을 못박은 뒤** Stage 2 에서 최소 수정한다(섣부른 한 줄 패치 회피).

## 4. 범위 / 무영향 (하드 제약)

- 변경: `rhwp-studio/src/view/virtual-scroll.ts` (+ 필요 시 `canvas-view.ts`, `styles/editor.css`) — **최소 범위**.
- **공유 프런트 → 웹·데스크톱 양쪽 개선**(웹을 *깨는* 변경이 아니라 *개선*).
- ⚠️ **Pages 배포 영향(사전 고지)**: `deploy-pages.yml` 은 `main` push 트리거 → 본 수정이 main 머지되면 **다음 Pages 배포에 포함**(의도된 개선). "Pages 무영향" 제약은 데스크톱화 작업 한정이었고, 본 건은 공유 프런트의 실제 버그 수정이라 양쪽을 함께 개선한다.
- 무영향: **rhwp 엔진 식별자**, 데스크톱 셸(아이콘 #5 / 메뉴 #7), CI 워크플로.

## 5. 단계

| Stage | 내용 | 게이트 |
|-------|------|--------|
| 1 | dev 서버 재현 + computed 측정 → **정확 원인 확정 + 수정안 도출** | 보고 후 |
| 2 | 최소 수정 적용 + 브라우저 리사이즈 **시각검증** + 그리드 회귀 + `tsc` 클린 | 보고 후 |
| 3 | 최종 보고 + main 머지 PR + (승인) 이슈 클로즈 | 각 행위별 승인 |

## 6. 검증 전략

- **재현/검증**: `cd rhwp-studio && npx vite --host 0.0.0.0 --port 7700`, 샘플 문서 로드 후 뷰포트 폭을 페이지보다 좁게 →
  - (수정 전) 짤림 + 스크롤 부재 재현·측정
  - (수정 후) 중앙정렬 유지 + 좁으면 가로 스크롤 생성 확인
  - 브라우저 자동화(Preview/Chrome MCP)로 리사이즈 + 스크린샷 증빙.
- **그리드 회귀**: 줌 ≤ 0.5 다중 페이지에서 다중 열 배치 정상 확인.
- **빌드**: `npm run build`(studio) `tsc` 클린.

## 7. 위험·주의

- 두 정렬 메커니즘(`margin:0 auto` + 절대 캔버스 `left:50%`) 혼재 → 한쪽만 고치면 불일치. Stage 1 에서 상호작용을 측정해 **일관되게** 수정.
- 데스크톱 반영은 차기 릴리스 CI 빌드 시(리네임 후 `hanpage-desktop-v*`). 웹은 main 머지 → Pages 자동 배포.
- 브랜치/머지 타깃: 최근 데스크톱 과제(#1/#5/#7) 선례대로 `origin/main` 분기 → **main PR**(devel 통합선과 분리). 다른 타깃을 원하면 지시 바람.
