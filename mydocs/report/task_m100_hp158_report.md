# Task #158 (M100) — 데스크톱 앱 크레이트 PR 검사 최종 보고서

Issue: #158 · 계획: [task_m100_hp158.md](../plans/task_m100_hp158.md) · 워크플로 `.github/workflows/desktop-check.yml`

## 1. 결과 요약

데스크톱 앱 크레이트(`HanPage-Desktop/src-tauri`)를 바꾼 PR 에서 macOS·Windows 각각 Clippy(`-D warnings`)·단위 테스트를,
macOS 에서 포맷을 검사하는 `Desktop check` 워크플로를 더했다. 이 크레이트는 루트 workspace 밖이라 지금까지 어떤 PR 검사도
실행하지 않았다.

## 2. GitHub 운영 변경 기록

- 등급: O3(job 명령·matrix·action)
- 근거: #158, 작업지시자 지시(2026-10-28) "제안된 작업 2개도 처리하자"
- 기준 시각 / SHA: 2026-10-28, `origin/devel` `2b23c21c7`(PR base), 병합 2026-11-03 `ac205a6aa`
- 대상 workflow·설정: 새 `.github/workflows/desktop-check.yml`. 저장소 설정·branch protection·secret 은 바꾸지 않는다
- 현재 상태: 데스크톱 크레이트의 fmt·Clippy·테스트를 실행하는 PR 검사 없음(`ci.yml` 은 루트 workspace, `desktop-release.yml` 은 태그·수동 번들)
- 기대 상태: 데스크톱 크레이트 또는 이 워크플로를 바꾼 PR 에서 `Desktop check (macOS)`·`Desktop check (Windows)` 가 실행된다. 다른 PR 에서는 실행하지 않는다
- 영향 event·branch·required check: `pull_request`(base `main`·`devel`), `workflow_dispatch`. `devel` 에 보호 규칙이 없어 required check 는 없다
- 변경 파일: `.github/workflows/desktop-check.yml`, `mydocs/plans/task_m100_hp158.md`, 이 보고서
- 로컬 검증: 3절
- 적용 승인: 작업지시자 지시(위)
- 적용 결과와 run URL: 4절
- 관찰 창과 완료 조건: 이 PR 과 다음 데스크톱 PR(#159)에서 두 job 이 실행되고 테스트 건수가 로그에 나온다
- rollback 조건과 명령: 워크플로가 관련 없는 PR 에서 돌거나 runner 비용이 문제가 되면 `.github/workflows/desktop-check.yml` 을 삭제하는 PR

## 3. 로컬 검증

| 명령 | 결과 |
|---|---|
| YAML 구조(`ruby -ryaml`) | jobs `check`, on `pull_request`·`workflow_dispatch` |
| 워크플로 정책 테스트 20개(`scripts/tests` 의 `ci-impact-*`·`ci-workflow-evidence`·`issue-form-labels` node 테스트, `test_*workflow*`·`test_workflow_promotion_*`·`test_release_channel_policy_workflow`·`test_postmerge_duration_workflow` 등 python unittest) | 모두 통과 |
| `scripts/check_e2e_manifest.py` | rc=1 — 이 워크플로가 없어도 같은 출력(upstream e2e 표의 `issue_8160_password_open` 행). CI 에서 실행하지 않는 로컬 스크립트라 이번 변경과 무관 |
| 깨끗한 checkout(`rhwp-studio/dist` 없음)에서 `cargo clippy --locked --all-targets -- -D warnings`·`cargo test --locked` (macOS) | 통과 — 프런트엔드 빌드 없이 컴파일된다 |

## 4. PR 에서의 실제 실행

PR #160 head `198b185dd` 의 `Desktop check` run 40648268643(`pull_request`):

| job | 결과 | 시간 | 단계 |
|---|---|---|---|
| Desktop check (macOS), `macos-14` | 성공 | 7분 50초 | Format·Clippy·Unit tests 성공 |
| Desktop check (Windows), `windows-latest` | 성공 | 8분 46초 | Format 건너뜀(설계), Clippy·Unit tests 성공 |

- 두 job 모두 `running 8 tests … 8 passed`(PR base 의 데스크톱 크레이트 — #157 병합 전)를 로그에 남겼다. 0건 실행이 아니다.
- 두 job 모두 `toolchain '1.93.1-…' is currently in use (overridden by '<repo>/rust-toolchain.toml')` — 툴체인을 루트 파일이 정한다는 설계(독립 검토 반영)와 일치한다.
- 캐시는 복원할 항목이 없어 처음부터 빌드했다(PR 은 저장하지 않음).
- 같은 head 의 다른 검사(CI·CodeQL·adapter·proptest 등) 32 성공·4 건너뜀. 2026-11-03 작업지시자 승인 뒤 squash 병합(`ac205a6aa`).
- 다음 데스크톱 PR(#168, #159)에서 #157 이후의 데스크톱 크레이트 테스트로 다시 실행된다.

## 5. 한계

- `devel` 보호 규칙이 없어 이 검사는 병합을 기계적으로 막지 않는다. 메인테이너가 PR 에서 결과를 확인한다.
- workflow_dispatch 는 워크플로가 기본 브랜치(`main`)에 있어야 쓸 수 있다. 다음 `devel → main` 승격 전까지 캐시를 저장할 경로가
  없어 PR 실행은 캐시 없이 돈다.
- `scripts/workflow_promotion_policy.json` 은 upstream 워크플로만 등록한다. HanPage 전용 워크플로(`desktop-release.yml`·이 파일)의
  승격 처리는 다음 `devel → main` 승격 때 함께 다룬다.
