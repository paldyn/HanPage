# Task M100 #2164 최종 보고서

- 이슈: [#2164](https://github.com/edwardkim/rhwp/issues/2164)
- 브랜치: `codex/task_m100_2164`
- 재현 원본: `samples/issue2164/의견제출서(양식).hwp`
- 원본 SHA-256: `a8f1874850d9970ba2f9cff868ada599081c349d71fe3c4a3e7a1023e399c98c`
- 작성일: 2026-07-10

## 결과

Chrome 확장 v0.2.8에서 보고된 표 셀 Enter 문단 겹침을 실제 제보 HWP로 재현해
정정했다. 셀 문단을 분할하거나 병합할 때 이후 문단의 세로 좌표를 다시 연결하고,
프론트 커서가 새 셀 문단을 가리키도록 flat 위치와 `cellPath`를 같이 갱신한다.

## 변경 내용

1. 셀 구조 편집 뒤 문단별 `LINE_SEG.vertical_pos`와 캐럿 y를 순서대로 재계산한다.
2. 신규 빈 문단은 자신의 활성 글자 모양 높이를 사용한다.
3. 실제 `RowBreak`의 저장된 vpos 원점은 유지하되, 방금 Enter로 삽입된 문단의 임시
   원점은 재계산을 막는 경계로 오인하지 않는다.
4. 셀 Enter/Backspace 뒤 `paragraphIndex`, `cellParaIndex`, `cellPath`의 마지막
   문단 인덱스, 캐럿 rect 캐시를 함께 갱신한다.
5. 실제 제보 원본으로 첫 Enter, 연속 Enter, Backspace 후 재Enter, HWP 저장 후
   재로드를 검증하는 Rust 회귀 테스트를 추가했다.

## 시각 확인

`문단 부호` 표시 상태에서 `1111`, Enter, `2222`, Enter를 수행했을 때 두 번째 Enter
직후 캐럿은 첫 줄이 아니라 새 세 번째 문단에 위치한다.

```text
cellParaIndex / paragraphIndex: 2 / 2
path cursor y: 445.3px
DOM caret top: 455.3px
```

작업지시자가 실제 화면에서도 수정 동작을 검증했다.

## 검증 기록

- Stage 2 완료 시 `cargo test --profile release-test --lib`: 2190 passed, 0 failed,
  7 ignored
- Stage 2 완료 시 focused Rust tests, studio/Chrome extension build, WASM build: 통과
- Stage 3: `wasm-pack build --target web --out-dir pkg`: 통과
- Stage 3: 기존 `localhost:7700`에서 실제 원본·실제 `cellPath` 연속 Enter: 통과

작업지시자 지시에 따라 Stage 3 변경 후 전체 Cargo test, Clippy, 전체 프론트 빌드는
PR 준비 전에 다시 실행하지 않았다. PR의 최신 head CI에서 전체 회귀 상태를 확인한다.
