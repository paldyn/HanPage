# task m100 1686 stage1: pr-1674 page3 co-anchored RowBreak 순서 보정

## 기준선

- GitHub Issue: #1686
- 기준 브랜치: `upstream/devel`
- 대상 샘플:
  - `samples/hwpx/pr-1674.hwpx`
  - `samples/pr-1674.hwp`
- 시각 기준:
  - HWPX: `pdf-2020/pr-1674-2020.pdf` (한컴 2020 PDF, 35쪽)
  - HWP: `pdf/pr-1674-2024.pdf` (한컴 2024 PDF)
- page count 기준:
  - HWPX: 35쪽
  - HWP: 36쪽

## 재현

`devel` 기준 page 3에서 `0.27` 문단의 두 번째 co-anchored 표(`[응시자격요건 고려사항]`)가
`0.28`의 `다. 우대요건 등 [원서접수 마감일 기준]`보다 먼저 배치됐다.

- 시작 page count:
  - HWPX: 36쪽
  - HWP: 37쪽
- RED:
  - `cargo test --profile release-test --test issue_1686 -- --nocapture`
  - HWPX/HWP 모두 page 3에서 `다.우대요건등[원서접수마감일기준]` 누락으로 실패

## 구현

- `TypesetState`에 같은 문단의 후행 표를 임시 보류하는 `deferred_table_controls` 큐를 추가했다.
- 빈 host 문단의 비-TAC `TopAndBottom`/`Para` 기준 `RowBreak` 표가 분할 continuation을 만들면,
  같은 문단의 후행 양수 offset `RowBreak` 표를 즉시 배치하지 않고 보류한다.
- 보류된 표는 뒤쪽 표 문단 처리 후 또는 문서 마지막 flush 직전에 다시 조판한다.
- 적용 범위는 아래 조건으로 제한했다.
  - host 문단에 visible text 없음
  - 비-TAC 표
  - `TextWrap::TopAndBottom`
  - `VertRelTo::Para`
  - `TablePageBreak::RowBreak`
  - 보류 대상 후행 표는 `vertical_offset > 0`

## 결과

패치 후 `release-test` binary 기준 page 3:

```text
PartialTable   pi=27 ci=0  rows=4..7  cont=true
FullParagraph  pi=28  "다. 우대요건 등 [원서접수 마감일 기준]"
Table          pi=29 ci=0
PartialTable   pi=27 ci=1  rows=0..3  cont=false
```

최종 page count:

- HWPX: 35쪽 (PDF 오라클 35쪽과 일치)
- HWP: 36쪽 (HWP 기준값과 일치)

## 검증

- `cargo fmt`
- `/usr/bin/time -p env CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/Users/tsjang/rhwp/target cargo test --profile release-test --test issue_1686 -- --nocapture`
  - 2 passed
  - real 35.18s
- `/usr/bin/time -p env CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/Users/tsjang/rhwp/target cargo build --profile release-test --bin rhwp`
  - real 42.84s
- `target/release-test/rhwp info samples/hwpx/pr-1674.hwpx`
  - 35쪽
- `target/release-test/rhwp info samples/pr-1674.hwp`
  - 36쪽
- `pdfinfo pdf/pr-1674-2024.pdf`
  - Creator: `Hwp 2024 13.0.0.3622`
  - Pages: 35쪽
  - page 3에 `다. 우대요건 등 [원서접수 마감일 기준]`와 우대요건 표가 먼저 배치되는 것을 확인
- `target/release-test/rhwp dump-pages samples/hwpx/pr-1674.hwpx -p 2`
  - page 3 순서 정상
- `target/release-test/rhwp dump-pages samples/pr-1674.hwp -p 2`
  - page 3 순서 정상
- `/usr/bin/time -p env CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/Users/tsjang/rhwp/target cargo test --profile release-test --test issue_1510 --test issue_1535 --test issue_1639 --test issue_1086 --test issue_1156_rowbreak_fragment_fit --test issue_1488_rowbreak_empty_overlay_pages -- --nocapture`
  - 16 passed
  - real 37.45s
- `/usr/bin/time -p wasm-pack build --target web --out-dir pkg`
  - 통과
  - real 104.15s
- rhwp-studio WASM 시각 검증
  - Browser plugin 연결 도구가 현재 세션에 노출되지 않아 headless Puppeteer 경로로 검증했다.
  - 기존 `localhost:7700` Vite dev server에 최신 `pkg` 산출물을 `rhwp-studio/public/`로 임시 동기화했다.
  - `samples/hwpx/pr-1674.hwpx` page 3 캔버스 캡처: `pdf-2020/pr-1674-2020.pdf`와 동일하게 `다. 우대요건...`/우대요건 표가 `[응시자격요건 고려사항]`보다 먼저 표시됨.
  - `samples/pr-1674.hwp` page 3 캔버스 캡처: `pdf/pr-1674-2024.pdf` 기준으로 동일 순서 확인.
  - WASM render tree 순서 확인:
    - HWPX: `다.우대요건등` index 1343 < `[응시자격요건고려사항]` index 2187
    - HWP: `다.우대요건등` index 1343 < `[응시자격요건고려사항]` index 2187
  - console error/pageerror: 없음
  - 캡처 산출물: `/tmp/rhwp-1686-visual/`
