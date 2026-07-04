# 최종 보고서 - Task #1918

**이슈**: #1918 표 입력/삭제 시 워터마크·정적 이미지 레이어 포함 페이지가 매 입력마다 고비용 재렌더링됨
**브랜치**: `local/task1918`
**작성일**: 2026-07-05

---

## 1. 문제

워터마크, 배경 이미지, OLE/RawSvg 같은 정적 이미지성 요소가 있는 문서에서 표 셀에 글자를 입력하거나
삭제하면 편집 명령 자체보다 편집 후 페이지 재렌더링이 지배적인 비용을 만들었다.

Stage 1 계측으로 두 병목을 분리했다.

| 샘플 | 병목 |
|------|------|
| `samples/복학원서.hwp` | text edit refresh마다 `flow/background/behind/front` overlay가 모두 반복 렌더링됨 |
| `samples/253E164F57A1BC6934-empty.hwp` | behind/front overlay는 없고 flow 내부 정적 이미지 채움이 반복 렌더링됨 |
| `samples/143E433F503322BD33.hwp` | flow 내부 OLE/RawSvg 계열 정적 객체 비용 |

## 2. 구현 요약

### Stage 1 - 재현 계측

- 편집 명령 비용과 렌더 비용을 분리했다.
- `복학원서.hwp`의 셀 편집 명령은 1ms 미만이지만 렌더 후처리는 약 679ms였다.
- Studio text-edit refresh 기준 호출 수를 계측했다.

### Stage 2 - overlay summary 경량화

- `PageRenderer`가 전체 `PageLayerTree`보다 먼저 `getPageOverlayImages` summary를 사용하도록 바꿨다.
- Rust summary API가 replay-plane 기준으로 `hasBehind`, `hasFront`, `imageCount`, `rawSvgCount`를 계산하도록 보정했다.
- 전체 tree fallback은 유지했다.

### Stage 3 - text edit overlay canvas 재사용

- `CanvasView`가 `reason: 'text-edit'`를 `PageRenderer`까지 전달한다.
- text-edit fast path에서 background/behind/front overlay canvas key가 같으면 재사용한다.
- 일반 refresh, zoom, resize, full document refresh, CanvasKit 경로는 기존 동작을 유지한다.

### Stage 4 - flow 내부 정적 이미지 분리

- WASM Canvas2D 필터에 `flow-dynamic`, `flow-static`을 추가했다.
- text-edit fast path에서 조건이 맞으면 동적 본문과 정적 flow 이미지/RawSvg를 분리한다.
- 기존 WASM 산출물에서 새 필터가 없으면 기존 `flow`로 fallback한다.
- image/rawSvg delayed rerender 정책이 static flow와 overlay reuse 정책을 보존하도록 분리했다.

### Stage 5 - 회귀 검증과 보고

- 전체 Rust/Studio 테스트와 E2E를 통과시켰다.
- compact overlay summary 테스트를 새 flow count 필드 포함 계약에 맞게 보정했다.
- Docker daemon 부재로 새 WASM runtime 성능 probe는 완료하지 못했고, 제한 사항으로 남겼다.

## 3. 주요 변경 파일

| 파일 | 내용 |
|------|------|
| `rhwp-studio/src/view/canvas-view.ts` | text-edit invalidation context 전달 |
| `rhwp-studio/src/view/page-renderer.ts` | overlay summary 우선 사용, overlay canvas 재사용, static flow 분리, delayed rerender policy 분리 |
| `rhwp-studio/src/core/wasm-bridge.ts` | `flow-dynamic`, `flow-static` bridge 계약 추가 |
| `src/document_core/queries/rendering.rs` | overlay summary에 flow image/rawSvg count 추가 |
| `src/renderer/web_canvas.rs` | `LayerFilter::FlowDynamic`, `LayerFilter::FlowStatic` 추가 |
| `src/wasm_api.rs` | `renderPageToCanvasFiltered` layer kind 확장 |
| `tests/issue_850_answer_sheet_name_hit_test.rs` | compact summary 계약 보정 |
| `tests/issue_938.rs` | overlay summary flow count 회귀 가드 추가 |
| `rhwp-studio/tests/render-backend.test.ts` | PageRenderer static overlay/static flow 정책 정적 계약 테스트 추가 |

## 4. 검증

통과한 명령:

```bash
cargo test
```

```bash
cd rhwp-studio
npm test
npm run build
npm run e2e:renderer-contract
```

```bash
CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
VITE_URL="http://127.0.0.1:7700" \
node e2e/issue-1456-chart-rerender.test.mjs --mode=headless
```

```bash
target/debug/rhwp bench \
  samples/253E164F57A1BC6934-empty.hwp \
  samples/143E433F503322BD33.hwp \
  samples/복학원서.hwp \
  samples/table-001.hwp \
  -n 5
```

Native bench 결과는 Stage 1과 같은 범위였고, 일반 표 샘플 `table-001.hwp`는 여전히 render 3.4ms 수준이다.

## 5. 성능 관찰

브라우저 probe 기준:

| 샘플 | Stage 1 filtered render | Stage 5 filtered render |
|------|-------------------------|-------------------------|
| `복학원서.hwp` | `flow/background/behind/front` 4회 | `flow` 1회 |
| `253E164F57A1BC6934-empty.hwp` | `flow` 1회 | `flow-dynamic` 시도 후 기존 `flow` fallback |
| `143E433F503322BD33.hwp` | `flow` 1회 | `flow` 1회 |
| `table-001.hwp` | `flow` 1회 | `flow` 1회 |

현재 로컬 `pkg/`가 새 Rust WASM 변경을 포함하지 않으므로 `flow-dynamic`/`flow-static`의 실제 runtime 성능은
아직 측정하지 못했다. 다만 fallback 동작은 확인됐다.

## 6. 남은 확인

Docker daemon이 실행되는 환경에서 다음을 수행해야 한다.

```bash
docker-compose --env-file .env.docker run --rm wasm
```

그 후 같은 브라우저 probe로 다음을 확인한다.

- `253E164F57A1BC6934-empty.hwp`가 text-edit refresh에서 `flow-dynamic` + 재사용 `flow-static` 경로를 타는지
- `143E433F503322BD33.hwp` OLE/RawSvg 문서가 image decode 재렌더 회귀 없이 동작하는지
- zoom 변경 후 static layer box가 어긋나지 않는지

## 7. 판단

이슈의 핵심 병목 중 overlay 반복 렌더는 해결했다.
flow 내부 정적 이미지/OLE 분리도 소스와 테스트 계약은 구현됐지만, 새 WASM 산출물 성능 검증은 Docker daemon 부재로
완료하지 못했다. PR에는 이 제한을 명시하고, WASM 빌드 가능한 환경에서 최종 runtime probe를 추가 확인해야 한다.
