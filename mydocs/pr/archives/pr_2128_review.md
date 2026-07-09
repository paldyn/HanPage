# PR #2128 리뷰 문서

## 1. 메타

| 항목 | 내용 |
|---|---|
| PR | #2128 |
| 제목 | task 2099: U+F53A 아래아 렌더 복구 |
| 작성자 | jangster77 |
| base | devel |
| head | task/m100-2099-araea-pua |
| 관련 이슈 | Closes #2099 |
| 작성 시점 참고값 | mergeable=MERGEABLE, 코드 검증 기준 SHA=dfdfaf689d7607bc77fe1b438f12bb858177a918 |
| 규모 | 3 files, +100 / -21 |

최종 merge 조건은 PR head 최신 커밋 기준 GitHub Actions 통과와 작업지시자 승인이다.
위 mergeable, SHA, 규모 값은 작성 시점 참고값이며 merge 전 재확인한다.

## 2. 관련 이슈 요약

#2099는 Linux 환경에서 `한글문서파일형식_5.0_revision1.3` 문서의 `아래아`가 포함된 `글` 표기가
깨지거나 보이지 않는 문제다.

로컬에서 대응 가능한 기준 자료는 다음과 같다.

- `samples/hwpspec.hwp`: 178쪽 HWP5 배포용 원본
- `pdf/hwpspec-2024.pdf`: 기존 178쪽 기준 PDF
- `samples/pua-test.hwp` / `samples/pua-test.pdf`: PUA 회귀 검증 fixture

## 3. 변경 범위

핵심 변경은 다음과 같다.

- `src/renderer/pua_oldhangul.rs`에서 U+F53A를 `ᄒᆞᆫ` 자모 시퀀스로 다시 매핑한다.
- 이전 #615의 "매핑 표 외 공백" 판단은 보존된 `samples/pua-test.pdf`와 hwpspec 실제 본문 확인 결과에 맞춰
  #2099 기준으로 정정한다.
- `tests/issue_2099_araea_pua.rs`를 추가해 `samples/hwpspec.hwp`와 `samples/pua-test.hwp` SVG 출력에
  raw U+F53A가 남지 않고 `ᄒᆞᆫ`으로 확장되는지 고정한다.
- `mydocs/working/task_m100_2099_stage1.md`에 분석 및 검증 계획을 기록한다.

## 4. 렌더 영향 및 시각 검증

이 PR은 페이지 배치나 pagination이 아니라 PUA 옛한글 텍스트 확장 경로를 바꾸는 PR이다. 따라서 HWP 2020 MCP로
새 기준 PDF를 만들지는 않았다. 기존 저장소 기준 PDF와 fixture PDF를 근거로 U+F53A가 실제 `` 글리프로 보여야
함을 확인했고, SVG 렌더 결과에서 raw PUA가 사라지는지 테스트로 고정했다.

검증 fixture SHA-256은 다음과 같다.

| 파일 | SHA-256 |
|---|---|
| `samples/hwpspec.hwp` | `64df877f3c4accda0111ecf39d837da889741bb821b220cf61b6c126dc88364d` |
| `samples/pua-test.hwp` | `592e00e3e8bc72afef829fd71a13cbc340bff3770425b9d1ee025afc1e1649fa` |

## 5. 검증 결과

최종 확인한 로컬 검증은 다음과 같다.

- `cargo test --test issue_2099_araea_pua` 통과
- `cargo test --lib pua` 통과
- `cargo test --test issue_1086` 통과
- `cargo fmt --check` 통과
- `git diff --check` 통과
- `cargo clippy --all-targets -- -D warnings` 통과
- `wasm-pack build --target web --out-dir pkg` 통과

WASM build 후 추적 대상 `pkg/` 변경은 없었다.

## 6. 리스크 및 잔여 확인

- 이번 PR은 U+F53A 단일 코드포인트의 옛한글 확장 복구에 한정한다.
- 이슈 본문에 함께 언급된 영문 글자 간격 문제는 별도 폰트/조판 fidelity 축일 수 있으므로, #2099 close 전에는
  maintainer 시각 확인 또는 후속 이슈 분리가 필요한지 판단한다.
- merge 전 GitHub Actions 최신 head 결과를 재확인한다.

## 7. 최종 권고

로컬 집중 테스트, 관련 PUA 회귀 테스트, 기존 hwpspec 페이지 수 회귀, clippy, WASM build를 통과했고,
#2099의 아래아 표시 누락 핵심 원인을 U+F53A raw PUA 출력으로 좁혀 수정했다. PR #2128은 옵션 1 경로로
review 문서와 오늘할일 갱신을 같은 PR head에 포함한 뒤 GitHub Actions 통과와 작업지시자 승인 조건으로
merge 가능 후보로 본다.
