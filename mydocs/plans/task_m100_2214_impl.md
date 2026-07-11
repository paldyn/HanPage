# 구현 계획서 — Task M100 #2214

## 1. 작업 기준

- 이슈: [#2214](https://github.com/edwardkim/rhwp/issues/2214)
- 수행계획서: `mydocs/plans/task_m100_2214.md`
- 작업 브랜치: `issue-2214-page-local-repaint`
- 작업 worktree: `/private/tmp/rhwp-task2214`
- 기준 브랜치: `upstream/devel@48c33455`
- 작성일: 2026-07-11
- 직접 대상: Canvas2D 기반 Studio page-local text-edit 표시 정확성
- 재현 픽스처:
  - `samples/issue1949_giant_cell_nested_tables_perf.hwp`
  - `samples/issue1949_giant_cell_nested_tables_perf.hwpx`

## 2. 구현 원칙

1. 구현 순서는 **결정적 계측 → 모델 GREEN/화면 RED 계약 → 확정된 단일 경계 수정 →
   브라우저·성능 회귀 → 광역 게이트**로 고정한다.
2. HWP는 Stage 1에서 현재 증상을 결정적으로 재현해야 한다. HWPX는 같은 절차로 현재
   RED 또는 기존 GREEN을 분류하고, 재현되지 않는 형식을 억지로 RED로 만들지 않는다.
   수정 후에는 두 형식 모두 동일한 최종 GREEN 계약을 만족해야 한다.
3. 데이터 보존은 모델·native 저장 재로드를, 표시 정확성은 page text layout·layer tree·
   브라우저 합성 화면을 권위로 삼는다.
4. Enter는 문단 분할과 pagination을 함께 수행하므로 자동 테스트의 정답지로 사용하지 않는다.
   입력 모델을 바꾸지 않는 explicit deferred pagination flush와 full refresh를 대조군으로 둔다.
5. production 수정은 Stage 1의 최초 stale 경계와 Stage 2의 RED assertion이 같은 원인을
   가리킨 뒤에만 수행한다. 여러 후보를 한 번에 수정하지 않는다.
6. 매 입력마다 115쪽 전체 pagination, 전역 cache 무효화, 800ms 검증 timer 무조건 제거,
   Enter 또는 full refresh 자동 주입은 허용하지 않는다.
7. 수행계획의 “입력 중 전체 pagination flush 0회”를 유지한다. Stage 1에서 explicit
   pagination만이 유일한 복구 수단으로 확인되면 Stage 3으로 가지 않고 수행계획의 성능
   가드 변경 승인을 다시 받는다.

## 3. 현재 계약과 구현 시 주의점

### 3.1 page-local 판정과 deferred mutation은 서로 다른 계약이다

현재 짧은 셀 삽입은 refresh 판정보다 먼저
`insertTextInCellDeferredPagination()`을 실행한다. 반면
`deferredPaginationPending=true`는 `afterPageLocalEdit()`에서만 등록된다.

따라서 line-count 변화만 page-local 거부 조건에 추가한 뒤 기존 `afterEdit()`로 보내는
방식은 안전하지 않다. 현재 mutation이 아직 pending으로 등록되지 않아 full 경로에서도
pagination이 flush되지 않을 수 있다. before/after page 변화, 강제 full refresh, redo,
delete undo에서도 같은 계약 누락이 발생할 수 있다.

Stage 3에서 deferred mutation을 full 경로로 재분류해야 하는 결과가 나오면 다음을 먼저
만족해야 한다.

- mutation이 실제로 pagination을 지연했는지 명시적인 effect로 전달
- refresh 판정보다 먼저 deferred pending 등록
- flush 성공 뒤 cursor rect 재계산
- 즉시 pagination된 mutation에는 거짓 pending을 만들지 않음
- flush 실패 시 pending을 보존

다만 이 경로는 전체 pagination 0회라는 현재 수행계획과 충돌하므로, #2214의 승인된
Stage 3 후보에는 바로 포함하지 않는다. explicit pagination만으로 복구되는 경우 별도
범위 승인을 받는다.

### 3.2 기존 E2E helper는 실제 앱 로드 경로를 완전히 재현하지 않는다

기존 `loadHwpFile()`은 `wasm.loadDocument()`와 `canvasView.loadDocument()`만 호출해
문서별 폰트 로딩, 설정 적용, input handler 활성화를 우회한다. #2214는 폰트·입력·Canvas
시간축이 핵심이므로 `open-document-bytes`와 `open-document-bytes:done` 이벤트를 사용한
실제 앱 로드 helper를 둔다.

기존 `captureCanvasScreenshot()`은 main flow canvas 하나만 캡처하므로 `flow-static`,
background/behind/front canvas와 DOM caret을 누락한다. 브라우저가 합성한 페이지 영역을
clip 캡처하고, 구조적 text/run 단언을 pixel 비교보다 우선한다.

### 3.3 800ms 검증은 조건부 경로다

static verification은 static flow 또는 behind/front layer를 실제 재사용한 경우에만
예약된다. Stage 1 trace에서 예약 여부를 확인하기 전 timer 또는 generation 문제로
단정하지 않는다.

## 4. 작업환경 격리

다른 진행 중 작업과 충돌하지 않도록 모든 산출물과 서버를 #2214 worktree 안에 격리한다.

1. `rhwp-studio/node_modules`는 worktree에서 `npm ci`로 별도 준비한다.
2. `.env.docker`가 없을 때만 `.env.docker.example`을 복사하고, 해당 worktree에서 Docker
   WASM을 빌드해 `pkg/`를 생성한다.
3. Vite는 `127.0.0.1:7714 --strictPort`로 실행하고 E2E는
   `VITE_URL=http://127.0.0.1:7714`를 사용한다.
4. headless Chrome은 고정 viewport 1280×900, DPR 1, zoom 100%로 실행하고
   `document.fonts.ready`와 2 rAF를 기다린다.
5. `node_modules`, `.env.docker`, `pkg/`, `output/poc/task2214/`, E2E screenshot은 Git에
   포함하지 않는다.

## 5. 예상 변경 파일

### 공통·테스트 파일

| 파일 | 역할 |
|------|------|
| `rhwp-studio/e2e/issue-2214-page-local-repaint.test.mjs` | 시간축 진단, RED→GREEN, HWP/HWPX 합성 화면 권위 E2E |
| `rhwp-studio/e2e/helpers.mjs` | 필요할 때만 실제 앱 샘플 로드·2 rAF·합성 crop helper 추가 |
| `tests/issue_2214_page_local_repaint.rs` | 모델·`LINE_SEG`·flush·원본 형식 저장 재로드 GREEN 핀 |
| 기존 TS test 한 곳 | Stage 1에서 확정된 경계의 최소 계약 테스트 |
| `mydocs/working/task_m100_2214_stage{N}.md` | 각 Stage 증거와 검증 결과 |
| `mydocs/report/task_m100_2214_report.md` | 최종 원인, 수정, 정확성·성능 결과 |

### Stage 1 결과별 production 후보

아래 행 가운데 Stage 2 승인으로 선택된 한 행만 production 변경 대상으로 삼는다.

| 확정 결과 | 허용 후보 파일 | 최소 방향 |
|-----------|----------------|-----------|
| pagination 없이 fresh full-layer page render로 복구 | `input-handler.ts`, `canvas-view.ts`, 필요 시 `input-edit-invalidation.ts` | line/layout impact 시 해당 page의 static reuse만 끄고 전체 layer를 다시 그림 |
| 잘못된 page 또는 소수 영향 page 누락 | `input-handler.ts`, 필요 시 document-core page query | before/after/affected page만 상한을 두고 invalidation |
| 최신 첫 화면을 오래된 async callback이 덮음 | `canvas-view.ts`, 필요 시 `page-renderer.ts` | page별 edit generation과 callback generation 일치 검사 |
| 원자적 line/layout impact 신호가 필요 | `text_editing.rs`, `wasm_api.rs`, `wasm-bridge.ts`, `command.ts`, `input-handler.ts` 중 최소 파일 | deferred edit 결과에 line count/세로 영향도만 추가하고 page-local render 정책에 사용 |
| explicit pagination으로만 복구 | 현재 승인 범위 밖 | Stage 3 중단, mutation effect·pending·경계 flush 계약을 포함해 계획 재승인 |
| 모델 text 또는 `LINE_SEG`부터 손실 | 현재 표시 이슈 범위 밖 | Studio 수정 중단, document-core 편집/reflow 이슈로 재분리 |

## 6. 구현 단계

### Stage 1. 실제 앱 경로의 결정적 재현과 최초 stale 경계 계측

#### 목표

production 코드를 수정하지 않고 HWP 증상을 자동 재현하고 HWPX의 현재 상태를 분류한다.
모델, page fragment/layer tree, cursor, Canvas 중 최초 불일치 경계를 하나로 좁힌다.

#### 작업

1. `rhwp-studio/e2e/issue-2214-page-local-repaint.test.mjs`에 `--diagnose` 모드를 만든다.
2. 샘플을 `/samples/<filename>`에서 fetch한 뒤 다음 이벤트로 실제 앱 로드 경로를 탄다.
   - `open-document-bytes` emit: `requestId`, `skipUnsavedGuard: true`
   - 대응하는 `open-document-bytes:done` 대기
   - source format, 115쪽, font ready, Canvas, input handler 활성 확인
3. 캐럿을 다음 full cell path에 직접 배치한다.

```text
sectionIndex=0, paragraphIndex=5, charOffset=130
parentParaIndex=0, controlIndex=2, cellIndex=2, cellParaIndex=5
cellPath=[{ controlIndex=2, cellIndex=2, cellParaIndex=5 }]
```

4. `cursor.clearSelection()` → `cursor.moveTo()` → `resetPreferredX()` → `updateCaret()` →
   `focus()` 순서로 입력 상태를 고정한다.
5. 첫 로드에서 `1`을 한 글자씩 입력해 4줄→5줄 전환 문자 수 `N`을 최대 128자 안에서
   찾는다. `N`을 하드코딩하지 않는다.
6. 문서를 다시 로드하고 `N-1`자까지 입력한 뒤 전환 직전 상태를 수집한다.
7. `N`번째 문자는 hidden textarea의 동기 `InputEvent('input')`으로 dispatch해 첫 rAF
   이전 상태를 기록한다. 별도로 `page.keyboard.type('1')` 사용자 경로 smoke도 남긴다.
8. 다음 체크포인트를 수집한다.
   - 동기 input 직후
   - 2 rAF 뒤
   - 100ms 뒤
   - 850ms 뒤
   - 1.6초 뒤
   - 추가 1자와 2자 입력 후 2 rAF
   - explicit flush와 full refresh 뒤
9. E2E 내부 wrapper로 다음 호출과 이벤트를 계측하고 원본 함수로 위임한다.
   - `flushDeferredPagination`
   - `renderPageToCanvasFiltered`
   - `PageRenderer.renderPage`
   - `document-page-invalidated`
   - `document-changed`
10. 각 체크포인트에 다음을 기록한다.
    - exact cell text와 length, cursor position/rect/`cellOverflowed`
    - `getLineInfoInCell()`로 수집한 line start/end/count
    - deferred pending과 page count
    - `getPageTextLayout()`의 대상 cell paragraph run
    - PageLayerTree의 상관 TextRun과 bbox
    - invalidated page, render context/result, filtered layer kind
    - static verification 예약 여부
    - DOM caret 위치와 브라우저 합성 crop
11. 통제 대조를 분리한다.
    - pagination 없이 full-layer page render
    - explicit flush + full refresh
    - verification이 예약된 경우 800ms 직전·직후
12. JSON/PNG는 `output/poc/task2214/stage1/`에 저장하고 커밋하지 않는다.

#### 원인 판정

| 관찰 | 판정 |
|------|------|
| 모델/LINE_SEG는 최신이고 fresh full-layer page render만으로 복구 | static reuse 또는 page-local render context 경계 |
| layer tree는 최신이나 invalidated page/Canvas/caret만 불일치 | page target 또는 합성 경계 |
| 첫 rAF 화면은 최신이나 850ms 뒤 후퇴 | delayed verification/render generation 경계 |
| explicit pagination에서만 복구 | 승인된 0-flush 범위로 해결 불가 — 중단·재계획 |
| 모델 text·저장 데이터부터 손실 | 표시 이슈가 아님 — 중단·재분리 |

#### 검증

```bash
cd /private/tmp/rhwp-task2214/rhwp-studio
npm ci
```

```bash
cd /private/tmp/rhwp-task2214
docker compose --env-file .env.docker run --rm wasm
```

```bash
cd /private/tmp/rhwp-task2214/rhwp-studio
npm test
npx vite --host 127.0.0.1 --port 7714 --strictPort
```

```bash
cd /private/tmp/rhwp-task2214/rhwp-studio
VITE_URL=http://127.0.0.1:7714 \
CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
CHROME_EXTRA_ARGS="--force-device-scale-factor=1" \
node e2e/issue-2214-page-local-repaint.test.mjs --mode=headless --diagnose
```

#### 완료·중단 조건

- HWP에서 동일 체크포인트의 증상이 반복 재현되고, HWPX는 RED 또는 기존 GREEN으로
  결정적으로 분류돼야 한다.
- 최초 불일치 경계가 판정표 한 행으로 좁혀져야 한다.
- 후보 둘 이상이 독립 결함이거나 production 진단 API가 필요하면 Stage를 완료하지 않고
  범위 승인을 요청한다.
- `mydocs/working/task_m100_2214_stage1.md`와 진단 E2E를 같은 커밋에 포함하고 승인받기 전
  Stage 2로 가지 않는다.

커밋 메시지:

```text
Task #2214: Stage 1 - 연속 입력 표시 불일치 경계 계측
```

### Stage 2. native GREEN과 Studio RED 회귀 계약 고정

#### 목표

모델·저장 정확성과 화면 결함을 분리해 자동화한다. 기존 테스트는 모두 GREEN으로 유지하고,
신규 Studio E2E는 Stage 1에서 확인한 정확한 표시 assertion 한 곳만 예상 RED로 고정한다.

#### 작업

1. `tests/issue_2214_page_local_repaint.rs`를 추가해 HWP/HWPX 각각 다음을 확인한다.
   - 원문, 첫 네 줄 시작 `[0, 44, 84, 122]`, 115쪽
   - offset 130부터 deferred API로 한 글자씩 입력
   - line count가 4→5가 될 때까지 최대 128자, 이후 2자 추가
   - 매 입력 exact suffix와 기존 네 line start prefix 보존
   - unflushed line-seg tuple과 다음 문단 vpos 기록
   - explicit flush 후 구조와 115쪽 동등성
   - 원본 형식 저장·재로드 후 text, line-seg tuple, vpos, 115쪽 동등성
2. 기존 `tests/issue_2185_korean_break_unit.rs`는 한 글자
   `[0,44,84,122]`, `vpos=17160` 기준으로 그대로 유지한다.
3. Stage 1 E2E의 진단 레코드를 최종 assertion 계약으로 정리한다.
   - HWP: 현재 결함에 해당하는 assertion 하나만 RED
   - HWPX: Stage 1 결과가 RED면 같은 assertion, GREEN이면 positive guard
   - 모델, 로드, 폰트, cursor path, 115쪽 등 환경 assertion은 모두 GREEN
4. Stage 1 원인 경계에 대응하는 TS 계약 테스트 한 곳만 선택한다.
   - page-local 판정이면 `input-edit-invalidation.test.ts`
   - Canvas/layer 경계면 `render-backend.test.ts` 또는 신규 behavior test 한 곳
5. main canvas 하나가 아닌 `#scroll-container` 합성 screenshot을 target run bbox로 crop한다.
   caret blink는 crop 비교에서 숨기거나 2px 영역을 제외한다.
6. pixel 비교는 같은 세션의 2 rAF/850ms/1.6초/flush 후 상태끼리만 수행하며, 모델·run·
   layer tree assertion을 최우선 권위로 둔다.

#### 검증

```bash
cargo test --profile release-test --test issue_2185_korean_break_unit -- --nocapture
cargo test --profile release-test --test issue_2214_page_local_repaint -- --nocapture
```

```bash
cd rhwp-studio
npm test
VITE_URL=http://127.0.0.1:7714 \
CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
CHROME_EXTRA_ARGS="--force-device-scale-factor=1" \
node e2e/issue-2214-page-local-repaint.test.mjs --mode=headless
```

예상 결과는 native·기존 TS GREEN, 신규 HWP E2E의 지정 assertion RED다. 해당 E2E는
기본 `npm test`에 포함되지 않으며 Stage 보고서에 RED가 의도된 재현 계약임을 명시한다.

#### 완료·중단 조건

- native가 RED이면 모델/reflow 문제가 섞인 것이므로 Stage 3으로 가지 않는다.
- E2E RED가 사용자 증상이 아니라 로드·폰트·좌표 불안정이면 테스트 안정화를 먼저 한다.
- `mydocs/working/task_m100_2214_stage2.md`와 native/E2E/선택 TS test를 같은 커밋에
  포함하고 승인받기 전 Stage 3으로 가지 않는다.

커밋 메시지:

```text
Task #2214: Stage 2 - 연속 입력 표시 회귀 계약 고정
```

### Stage 3. 확정된 단일 표시 경계의 최소 수정

#### 목표

Stage 1·2에서 승인된 한 원인 경계만 production에서 수정하고 신규 E2E를 RED에서 GREEN으로
바꾼다.

#### 작업 원칙

1. Stage 1 판정표에서 선택된 한 행의 최소 파일만 수정한다.
2. pagination 없이 full-layer render로 복구되는 경우를 1순위 구현 후보로 둔다.
   - 입력 전후 line/layout impact 신호가 필요하면 pre/post `LineInfo` 또는 Rust mutation
     result에서 원자적으로 얻는다.
   - layout 영향이 없는 입력은 기존 static reuse fast path를 유지한다.
   - line/layout 경계 입력만 해당 page의 `allowStaticOverlayReuse=false`로 렌더한다.
   - CanvasView는 payload의 reuse 정책을 hard-coded `true` 대신 명시적으로 전달한다.
3. page target 누락이면 before/after/affected page 중 필요한 소수 페이지만 invalidation한다.
   영향 page 상한을 넘으면 임의로 전체 pagination하지 않고 중단한다.
4. stale async callback이면 page별 edit generation을 증가시키고 rAF/verification callback이
   현재 generation과 일치할 때만 렌더하도록 한다.
5. 원자적 mutation effect가 필요하면 `pagination: current|deferred`와 line/layout impact를
   분리해 전달한다. 현재 mutation을 full 경로로 보낼 경우 pending 등록 누락이 없도록 별도
   계약이 필요하지만, 전체 pagination이 발생하는 설계는 이번 승인 범위에서 구현하지 않는다.
6. `cellOverflowed`는 stale page tree의 bbox clamp 결과이므로 line/layout 전환의 단독
   판정값으로 사용하지 않는다.
7. Stage 2 test와 선택 경계의 기존 unit test만 갱신하고 무관한 renderer·pagination 코드는
   건드리지 않는다.

#### 금지 변경

- 매 키 또는 줄 경계 입력의 115쪽 전체 pagination
- 800ms 검증 timer 무조건 삭제·지연
- page-tree 또는 layer cache 전역 무효화
- Enter/full refresh 합성 입력
- parser/serializer, `korean_break_unit`, font metric 변경
- 판정표의 production 후보 두 행 이상 동시 수정

#### 검증

```bash
cargo test --profile release-test --test issue_2214_page_local_repaint -- --nocapture
cargo test --profile release-test --test issue_2185_korean_break_unit -- --nocapture
```

```bash
cd rhwp-studio
npm test
VITE_URL=http://127.0.0.1:7714 \
CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
CHROME_EXTRA_ARGS="--force-device-scale-factor=1" \
node e2e/issue-2214-page-local-repaint.test.mjs --mode=headless
```

필수 GREEN:

- HWP/HWPX 모델·저장 재로드
- 1~2 rAF 안의 text/caret/layer tree와 합성 화면
- 850ms·1.6초 비후퇴
- explicit flush 전후 구조·합성 crop 동등
- explicit 대조 호출 전 full pagination flush 0회
- 115쪽

#### 완료·중단 조건

- RED→GREEN이 확정 경계의 최소 수정과 trace로 설명돼야 한다.
- explicit pagination만이 해결책이거나 production 후보 두 행을 함께 수정해야 하면 Stage를
  완료하지 않고 구현계획 개정 승인을 요청한다.
- `mydocs/working/task_m100_2214_stage3.md`와 production/test 변경을 같은 커밋에 포함하고
  승인받기 전 Stage 4로 가지 않는다.

커밋 메시지:

```text
Task #2214: Stage 3 - page-local 표시 상태 경계 정합
```

### Stage 4. 최신 WASM 브라우저 정확성과 #1918 fast-path 무회귀

#### 목표

최신 production source로 WASM을 다시 빌드하고 #2214 정확성뿐 아니라 #1918의 빠른
연속 입력·정적 레이어 재사용 효과가 유지되는지 검증한다.

#### 작업

1. Docker WASM을 다시 빌드하고 빌드 시각 또는 hash를 Stage 보고서에 기록한다.
2. HWP/HWPX #2214 E2E를 같은 고정 환경에서 반복 실행한다.
3. 같은 E2E trace에 안정적인 줄 내부 입력 시나리오를 두어 다음을 확인한다.
   - 기존 page-local/static reuse 경로 유지
   - full pagination flush 0회
   - filtered static/overlay render 호출 수가 Stage 1 기준보다 증가하지 않음
4. 줄 경계 입력도 Stage 3에서 선택한 국소 render만 발생하고 pagination이 없음을 확인한다.
5. key handler median/p95와 render 호출 수를 Stage 1과 같은 방법으로 기록한다. 환경 의존
   절대 시간 하나만 완료 기준으로 삼지 않는다.
6. renderer contract, 일반 edit pipeline과 #1949/#2185 인접 회귀를 실행한다.
7. 필요하면 E2E assertion과 테스트 helper만 안정화한다. 새 production 수정이 필요하면
   Stage 3으로 돌아가 승인을 다시 받는다.

#### 검증

```bash
cd /private/tmp/rhwp-task2214
docker compose --env-file .env.docker run --rm wasm
cargo test --profile release-test --test issue_1949_giant_cell_render_perf -- --nocapture
cargo test --profile release-test --test issue_2185_korean_break_unit -- --nocapture
cargo test --profile release-test --test issue_2214_page_local_repaint -- --nocapture
```

```bash
cd rhwp-studio
npm test
npm run build
npm run e2e:renderer-contract
VITE_URL=http://127.0.0.1:7714 \
CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
CHROME_EXTRA_ARGS="--force-device-scale-factor=1" \
node e2e/issue-2214-page-local-repaint.test.mjs --mode=headless
```

#### 완료·중단 조건

- 정확성 GREEN과 함께 stable-line fast path 호출 수·median/p95가 구조적으로 퇴행하지 않아야
  한다.
- Canvas2D는 통과하지만 renderer contract, 정적 layer 표시 또는 rapid input이 퇴행하면
  Stage 3으로 돌아가 재승인한다.
- `mydocs/working/task_m100_2214_stage4.md`와 최종 E2E/test 보강을 같은 커밋에 포함하고
  승인받기 전 Stage 5로 가지 않는다.

커밋 메시지:

```text
Task #2214: Stage 4 - WASM 브라우저 회귀 검증 보강
```

### Stage 5. 광역 게이트와 최종 보고

#### 목표

전체 Rust·Studio 게이트를 통과시키고, 원인·수정·정확성·성능 결과와 후속 범위를 문서화한다.

#### 작업

1. 전체 Rust test, clippy, fmt check를 실행한다.
2. Studio unit/build, renderer contract와 #2214 E2E를 다시 실행한다.
3. Stage 4 이후 production source가 바뀌지 않았고 동일 WASM을 검증했는지 hash/시각으로
   확인한다. 바뀌었다면 WASM을 다시 빌드하고 브라우저 검증을 반복한다.
4. `mydocs/working/task_m100_2214_stage5.md`와
   `mydocs/report/task_m100_2214_report.md`를 작성한다.
5. `mydocs/orders/20260711.md`의 #2214 상태와 검증 요약을 갱신한다.
6. #2193 종합 성능과 #2215 드래그 selection은 별도 후속임을 최종 보고서에 명시한다.
7. 검증 실패로 production 수정이 필요하면 Stage 5에 섞지 않고 해당 Stage로 돌아가
   변경 계획을 다시 승인받는다.

#### 검증

```bash
cargo fmt --check
cargo test --profile release-test --tests
cargo clippy --all-targets --all-features -- -D warnings
```

```bash
cd rhwp-studio
npm test
npm run build
npm run e2e:renderer-contract
VITE_URL=http://127.0.0.1:7714 \
CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
CHROME_EXTRA_ARGS="--force-device-scale-factor=1" \
node e2e/issue-2214-page-local-repaint.test.mjs --mode=headless
```

```bash
git diff --check
git status --short
```

`cargo fmt` 실행은 금지하고 `cargo fmt --check`만 사용한다. 기능과 무관한 전체 포맷 diff를
만들지 않는다.

#### 완료·승인 조건

- 모든 게이트가 GREEN이고 계획된 tracked 산출물 외 변경이 없어야 한다.
- Stage 5 보고서, 최종 보고서와 오늘할일 갱신을 같은 커밋에 포함한다.
- 최종 승인 전 이슈 close, push, PR, 통합을 수행하지 않는다.

커밋 메시지:

```text
Task #2214: Stage 5 - 전체 회귀 검증 및 결과 보고
```

## 7. 전역 검증 계약

### 모델·저장

- 매 입력 뒤 exact suffix와 cursor offset이 증가한다.
- #2185 한 글자 `[0,44,84,122]`, `vpos=17160`, 115쪽 기준을 유지한다.
- 반복 입력 뒤 기존 네 line start는 prefix로 남고, 새 줄·다음 문단 vpos는 explicit flush와
  재로드 결과에 일치한다.
- HWP/HWPX 원본 형식 저장·재로드 뒤 text, line tuple, vpos, 115쪽이 동일하다.

### 화면·시간축

- 1~2 rAF 안에 대상 TextRun, 합성 화면과 caret 위치가 최신 입력과 일치한다.
- 850ms와 1.6초 뒤에도 최신 상태가 되돌아가지 않는다.
- caret opacity는 blink 때문에 실패 조건으로 사용하지 않고 display와 위치를 확인한다.
- 동일 세션의 pre/post flush 합성 crop은 허용 오차 안에서 동등하다.

### 성능·호출 구조

- explicit 대조 전 전체 pagination flush 호출은 0회다.
- stable-line 입력은 기존 page-local/static reuse 경로를 유지한다.
- 줄 경계 입력은 확정 원인에 필요한 최소 page/layer 작업만 수행한다.
- render 호출 수와 median/p95가 Stage 1 기준 대비 구조적으로 퇴행하지 않는다.

## 8. 전역 중단 조건

- HWP에서 결정적 재현이 되지 않거나 재현 위치·폰트 상태가 매 실행 달라진다.
- 모델 text·`LINE_SEG` 또는 원본 형식 저장 데이터가 시간 경과로 손실된다.
- explicit pagination만이 복구 수단이다.
- production 후보 두 행 이상을 동시에 수정해야 한다.
- 전체 renderer/paginator 또는 일반 증분 pagination 설계가 필요하다.
- HWP/HWPX parser·serializer, `korean_break_unit`, font metric 변경이 필요하다.
- #1918 fast path를 전면 철회하거나 매 입력 전체 refresh가 필요하다.

중단 조건이 발생하면 해당 Stage 보고서에 증거를 남기고 수행계획 또는 구현계획 개정 승인을
받는다.

## 9. 단계별 커밋과 승인 원칙

- 각 Stage의 source/test와 `mydocs/working/task_m100_2214_stageN.md`를 같은 커밋에 포함한다.
- 커밋 메시지는 `Task #2214: ...` 형식을 사용한다.
- 각 Stage 완료 후 작업지시자 승인 없이 다음 Stage로 넘어가지 않는다.
- 기능에 무관한 포맷, 다른 worktree 산출물, `scripts/frontend-metrics/`는 포함하지 않는다.
- 최종 보고서와 오늘할일 상태는 Stage 5 커밋에 포함한다.
- 이슈 close, 원격 push, PR 생성, 브랜치 통합은 별도 승인 전 수행하지 않는다.

본 구현계획 승인 전에는 소스 코드와 테스트를 수정하거나 런타임 의존성을 설치하지 않는다.
