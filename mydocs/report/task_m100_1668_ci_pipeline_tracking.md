# Task M100 #1668 CI 파이프라인 추적 기록

## 목적

이 문서는 #1668의 하위 이슈인 #1664, #1666, #1667, #1665 사이의 정책 결정, 측정 기준,
변경 전/후 기준선, PR 상태를 연결해 추적하는 장기 보관 문서다.

각 하위 이슈의 원천 측정값은 개별 측정 문서에 두고, 이 문서는 하위 이슈 간 해석,
의존성, 다음 진행 판단을 기록한다.

관련 문서:

- #1664 원천 측정 문서: `mydocs/report/task_m100_1664_measurement.md`
- #1664 cache 정책: `mydocs/tech/ci_cache_policy_1664.md`

문서 PR #1701은 정책/측정 기록만 포함한다. 실제 CI workflow 변경은 후속 코드 PR #1702에서 다루며, #1702
관측값은 draft 코드 PR run 기준 비교 자료다. #1702가 merge되기 전에는 workflow 변경이 `devel`에 반영된
것으로 해석하지 않는다.

2026-07-01 후속 문서 PR에서는 #1702 merge 이후 cleanup, trusted branch save, 후속 exact-hit 결과를
부모 추적 문서에도 반영한다.

## 메인테이너 결정사항

### 회귀 가드 1:1 추적성 보존

CI 단축은 회귀 가드 구조를 보존하면서 프로필, 캐시, 병렬화 축에서만 추진한다.

수용 불가 항목:

- 통합 테스트 파일 통합
- 회귀 가드 명명 규칙 변경
- `tests/golden_svg/issue-NNN/` 자산 구조 변경

`tests/*.rs` 162개와 issue 계열 1:1 회귀 가드 131개의 추적성이 PR마다 유지되는지 확인한다.

### 정책 판단 4건 답변

| 항목 | 메인테이너 결정 | 적용 이슈 |
|------|----------------|----------|
| PR 릴리스 검증 정책 | PR은 `release-test` 중심, 릴리스 수준 검증은 `devel` push / tag workflow로 이동 | #1666 |
| 캐시 저장 정책 | PR은 복원 전용, `devel` / `main` push에서만 저장 | #1664 |
| job 병렬화 | #1666 적용 후 효과를 재평가한 뒤 결정 | #1665 |
| Rust 캐시 액션 | `Swatinem/rust-cache` 검토 허용. 단 SHA-pinned, 별도 PR, #1664 안정화 측정 후 판단 | #1667 |

### 문서 / 코드 PR 분리

메인테이너 요청에 따라 PR 단위는 다음처럼 분리한다.

- `mydocs/`: 정책 / 의사결정 / 측정 기록
- `.github/workflows/ci.yml`: 실제 CI 변경
- `Cargo.toml` profile 변경: 필요 시 단독 PR

## 확정 진행 순서

| 순서 | 이슈 | 범위 | 현재 판단 |
|------|------|------|-----------|
| 1 | #1664 | 캐시 정책 정리 | 코드 PR #1702 merge 완료. PR save 차단, cleanup 후 trusted branch save, 후속 exact-hit 확인 완료 |
| 2 | #1666 | PR `release-test` 프로필 전환 | #1702의 PR/devel step 시간과 compile 잔존 현상을 변경 전 기준으로 사용 |
| 3 | #1667 | Rust 캐시 전략 검토 | #1664 Build & Test cargo cache 안정화 확인 후 CodeQL/Rust cache 범위 분리 검토 |
| 4 | #1665 | Build & Test job 병렬 분리 | #1664/#1666/#1667 측정 후 전체 소요 시간이 남으면 재평가 |

## 공통 측정 기준

각 하위 이슈의 before/after 보고에는 다음 항목을 포함한다.

- PR checks 완료 시간 (P50, P90)
- `CI / Build & Test` job 시간
- 주요 step 시간
  - Build
  - Native Skia tests
  - Run lib tests
  - Run integration tests
- 캐시 hit/miss/save 성공 여부
- 캐시 크기
- 실패 시 원인 가시성
- runner-minutes 변화
- branch protection / required check 변경 여부
- 회귀 가드 162개가 PR마다 모두 실행되는지 확인

샘플 수가 1개뿐인 경우 P50/P90은 산출하지 않고 단일 관측값으로만 기록한다.

## #1664 관측 요약

### 후속/draft 코드 PR #1702 변경 후 표본

- PR: #1702
- Run: <https://github.com/edwardkim/rhwp/actions/runs/28430353568>
- Build & Test job: <https://github.com/edwardkim/rhwp/actions/runs/28430353568/job/84243307175>
- head SHA: `69229e7937dc08fb94bf5d6530f205de77c15fe4`
- 결론: 성공
- 해석 범위: #1702 draft 코드 PR 기준 관측값이며, #1702 merge 전에는 `devel` 반영 사실로 단정하지 않는다.

핵심 관측:

- `Restore cargo registry & build cache`: 정확히 적중
- 캐시 크기: 약 1476 MB
- `Save cargo registry & build cache`: PR에서 skipped
- 캐시 예약 / 읽기 전용 / 저장 실패 경고: 관측되지 않음
- `Build`, `Native Skia tests`, `Run lib tests`, `Run integration tests`, `Clippy` 모두 실행
- `Run integration tests` 기준 issue 계열 131/131 실행 확인

시간:

| 항목 | 시간 |
|------|------|
| PR checks 완료 시간 | 약 19m23s |
| `CI / Build & Test` job | 19m08s |
| Build | 3m33s |
| Native Skia tests | 3m57s |
| Run lib tests | 3m46s |
| Run integration tests | 4m51s |
| Check WASM target | 15s |
| Install native Skia runtime packages | 18s |
| Clippy | 21s |

### #1702 merge 후 trusted branch 표본

- #1664 상세 보고: <https://github.com/edwardkim/rhwp/issues/1664#issuecomment-4853263686>
- #1668 롤업: <https://github.com/edwardkim/rhwp/issues/1668#issuecomment-4853268643>

cleanup:

- closed/merged `refs/pull/*` cache 총 21개 삭제
- `refs/heads/devel`, `refs/heads/main`, OPEN PR cache 유지
- 최종 cache 총량: 7,154,189,707 B, 약 6.66 GiB / 7.15 GB
- 10GB budget 아래로 회복

trusted branch run:

| 구분 | run | SHA | Build & Test | restore | save | read-only 경고 |
|------|-----|-----|--------------|---------|------|----------------|
| cleanup 직후 rerun | <https://github.com/edwardkim/rhwp/actions/runs/28505355210/attempts/2> | `5e3b1ec652fda14a74af7cf9afd77962e3bb7903` | 22m53s | fallback `Linux-cargo-`, 426,792,350 B | success, 1,637,296,893 B | 없음 |
| 후속 devel run | <https://github.com/edwardkim/rhwp/actions/runs/28507949075> | `150ca316ee557d6bf95928302166e037d7467b03` | 18m02s | exact hit `Linux-cargo-6a1af...`, 1,637,296,893 B | skipped | 없음 |

주요 step 시간:

| run | Build | Native Skia tests | Run lib tests | Run integration tests | Clippy |
|-----|-------|-------------------|---------------|-----------------------|--------|
| `28505355210` attempt 2 | 4m43s | 4m55s | 3m44s | 6m23s | 46s |
| `28507949075` | 3m32s | 4m00s | 3m50s | 4m41s | 25s |

해석:

- #1664의 핵심 정책인 “PR에서는 save 차단, trusted branch에서 save 허용”이 end-to-end로 확인됐다.
- cleanup 전 cache budget 초과 상태가 trusted branch save를 막고 있었고, stale PR ref cache cleanup 후 새 exact
  cargo cache key 저장이 가능해졌다.
- 후속 `devel` run에서 exact-hit restore와 save skipped가 확인되어, save 조건도 기대대로 동작했다.
- 이 관측은 #1666의 profile 전환 전 기준선과 #1667의 cache 전략 재평가 기준선으로 사용한다.

### #1666 / #1667로 이관할 관측

캐시 복원은 성공했지만 PR merge ref와 프로필/feature 조합 때문에 최종 `rhwp` crate compile은 계속 발생했다.

관측된 compile:

- Build: `Compiling rhwp`, `release` 프로필 3m32s
- Native Skia tests: `Compiling rhwp`, `release` 프로필 3m56s
- Run lib tests: `Compiling rhwp`, `release` 프로필 3m34s
- Run integration tests: `Compiling rhwp`, `release-test` 프로필 3m32s

해석:

- 이는 #1664 실패가 아니다. #1664의 목표는 PR cache save 차단과 trusted branch save 정책 정착이다.
- `native-skia` feature, `release`, `release-test`, PR merge ref 조합 때문에 local crate 산출물이 별도로 컴파일될 수 있다.
- 이 값은 #1666 `release-test` 프로필 전환과 #1667 Rust 캐시 전략 비교의 변경 전 기준으로 사용한다.

## branch protection / required check 영향

#1664 PR #1702 및 merge 후 관측 기준:

- `Build & Test` job 이름 유지
- `CI / Build & Test` check 표면 유지
- branch protection / required check 변경 없음
- job 병렬화 없음
- runner-minutes 변화는 PR 표본 1개와 trusted branch 표본 2개뿐이므로 장기 증감 판단은 보류

## P50/P90 상태

| 구간 | 대상 | 샘플 수 | 판단 |
|------|------|---------|------|
| #1664 변경 후 | PR checks 완료 시간 | 1 | P50/P90 산출 보류 |
| #1664 변경 후 | `CI / Build & Test` job 시간 | 1 | P50/P90 산출 보류 |
| #1664 merge 후 | trusted branch `Build & Test` job 시간 | 2 | P50/P90 산출 보류. 22m53s / 18m02s 관측 |
| #1666 변경 전 | PR #1702 관측값 | 1 | `release-test` 전환 전 비교 기준으로 보존 |

## 다음 확인 항목

1. #1664는 메인테이너 완료 판단을 기다린다. 이슈 close는 작업지시자 승인 후에만 수행한다.
2. #1666 작업 전 #1702의 PR/devel Build / Native Skia / lib test / integration test 시간을 before 기준으로 참조한다.
3. #1666 적용 후 같은 측정 항목으로 after sample을 누적한다.
4. #1667에서는 Build & Test cargo cache와 CodeQL Rust analyze cache 범위를 분리해 판단한다.
5. OPEN PR cache는 계속 생성될 수 있으므로, cleanup은 closed/merged PR ref만 대상으로 유지한다.
