# Task M100 #1664 measurement 기록

## 목적

이 문서는 #1664 적용 전후 CI 측정값을 누적 기록하는 장기 보관 문서다. 단일 작업 완료 보고서가 아니라,
PR run과 `devel` / `main` push run이 쌓일 때마다 값을 추가하는 measurement 원천 로그로 사용한다.

정책/의사결정 원천 문서:

- `mydocs/tech/ci_cache_policy_1664.md`

## 기록 원칙

- GitHub Actions run URL 또는 run id를 함께 기록한다.
- PR run과 trusted branch push run을 분리해 기록한다.
- P50/P90은 샘플 1개로 의미 있게 해석하지 않는다.
- 샘플이 적을 때는 관측값으로만 기록하고, 분포 요약은 보류한다.
- workflow 변경 외 요인이 섞인 run은 비고에 명시한다.

샘플 수 해석 기준:

| 샘플 수 | 해석 |
|---------|------|
| 1-4 | 단일/소수 관측값. P50/P90 판단 보류 |
| 5-9 | 참고값. 방향성만 관찰 |
| 10-19 | 제한적 P50/P90 참고 가능 |
| 20+ | P50/P90을 추세 지표로 사용 가능 |

## 측정 항목

메인테이너 요청 기준:

- PR checks 완료 시간 (P50, P90)
- `CI / Build & Test` job 시간
- 주요 step 시간
  - build
  - lib test
  - integration test
  - native-skia
- cache hit/miss/save 성공 여부
- cache 크기
- 실패 시 원인 가시성
- runner-minutes 변화
- branch protection / required check 변경 여부
- 회귀 가드 162개가 PR마다 모두 실행되는지 확인

## 로컬 정적 검증

| 날짜 | 브랜치 | 항목 | 결과 | 비고 |
|------|--------|------|------|------|
| 2026-06-30 | `local/task1664` | `git diff --check` | 통과 | whitespace 문제 없음 |
| 2026-06-30 | `local/task1664` | `actionlint .github/workflows/ci.yml` | 통과 | workflow 문법 오류 없음 |
| 2026-06-30 | `local/task1664` | 변경 범위 확인 | 통과 | `Cargo.toml`, `tests/` 변경 없음 |
| 2026-06-30 | `local/task1664` | required check 표면 | 통과 | `Build & Test` job 이름 유지 |
| 2026-06-30 | `local/task1664` | 회귀 가드 구조 | 통과 | 테스트 파일/자산 구조 변경 없음 |

## PR run 측정 로그

PR run에서는 cache save가 skipped 되어야 한다.

| 날짜 | PR | run | head SHA | PR checks 완료 시간 | Build & Test 시간 | build | native-skia | lib test | integration test | restore hit/miss | save 상태 | 회귀 가드 162개 | 비고 |
|------|----|-----|----------|---------------------|-------------------|-------|-------------|----------|------------------|------------------|-----------|----------------|------|
| TBD | TBD | TBD | TBD | TBD | TBD | TBD | TBD | TBD | TBD | TBD | skipped 기대 | TBD | PR 생성 후 기록 |

## trusted branch push 측정 로그

`devel` / `main` push run에서는 exact cache hit가 아니면 cache save가 실행되어야 한다.

| 날짜 | branch | run | SHA | Build & Test 시간 | build | native-skia | lib test | integration test | restore hit/miss | save 상태 | cache 크기 | read-only 경고 | 비고 |
|------|--------|-----|-----|-------------------|-------|-------------|----------|------------------|------------------|-----------|------------|----------------|------|
| TBD | `devel` | TBD | TBD | TBD | TBD | TBD | TBD | TBD | TBD | success/skipped/failure | TBD | 없음 기대 | merge 후 기록 |

## P50/P90 요약

샘플 수가 충분할 때 갱신한다.

| 구간 | 대상 | 샘플 수 | P50 | P90 | 비고 |
|------|------|---------|-----|-----|------|
| before | PR checks 완료 시간 | TBD | TBD | TBD | 기존 run 수집 필요 |
| after | PR checks 완료 시간 | TBD | TBD | TBD | #1664 반영 후 누적 |
| before | `CI / Build & Test` job 시간 | TBD | TBD | TBD | 기존 run 수집 필요 |
| after | `CI / Build & Test` job 시간 | TBD | TBD | TBD | #1664 반영 후 누적 |

## cache 상태 요약

| 날짜 | 총 cache 크기 | 주요 key | PR ref cache 상태 | read-only 여부 | 비고 |
|------|---------------|----------|-------------------|----------------|------|
| TBD | TBD | TBD | TBD | TBD | GitHub cache 목록 확인 후 기록 |

## 진행 중 관측 메모

### 2026-06-30 — PR #1702 Build & Test 중간 관측

- PR: #1702 `Task #1664: cargo cache save를 trusted branch로 제한`
- Run: <https://github.com/edwardkim/rhwp/actions/runs/28430353568/job/84243307175?pr=1702>
- 상태: CI 진행 중 관측. 최종 결과와 step별 시간은 run 완료 후 위 표에 정식 반영한다.

관측 로그:

```text
Native Skia tests:
Dirty rhwp v0.7.17 (/home/runner/work/rhwp/rhwp): the file `src/parser/hwp3/mod.rs` has changed
(1782807499.561337796s, 21h 2m 26s after last build at 1782731753.488450083s)
   Compiling rhwp v0.7.17

Run lib tests:
Dirty rhwp v0.7.17 (/home/runner/work/rhwp/rhwp): the file `src/model/footnote.rs` has changed
(1782807499.556391453s, 20h 58m 35s after last build at 1782731984.950573421s)
   Compiling rhwp v0.7.17
```

임시 해석:

- `native-skia` feature가 켜진 lib test와 일반 lib test는 Cargo feature set이 달라 `rhwp` crate 산출물을
  각각 컴파일할 수 있다. 이 부분은 일부 정상 비용이다.
- 다만 restored `target` cache 이후에도 `Dirty rhwp ... has changed` 판정으로 local crate가 다시 컴파일되는
  현상은 target cache 실효성 문제일 수 있다.
- 이 관측은 #1666의 `--release` / `release-test` profile 전환 검토와 #1667의 Rust cache 전략 검토 근거로
  후속 정리한다.
- 단일 run 중간 관측이므로 #1666/#1667 이슈 코멘트는 CI 완료 후 전체 시간, step별 시간, cache restore/save
  상태와 함께 남긴다.

## 관찰 메모

- #1664 적용 전 read-only 경고가 관측됐다.
- #1664의 1차 목표는 PR save 차단과 trusted branch save 정착이다.
- P50/P90 개선은 #1666 profile 전환 전에는 제한적일 수 있다.
- #1664 안정화 측정 후 #1667 진행 여부를 판단한다.
