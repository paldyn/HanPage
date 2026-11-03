# Task #158 (M100) — 데스크톱 앱 크레이트 PR 검사 수행계획서

- **이슈**: [paldyn/HanPage#158](https://github.com/paldyn/HanPage/issues/158)
- **브랜치**: `claude/desktop-check-ci` (`origin/devel` 기준)
- **작업지시자 지시(2026-10-28)**: #156 의 2차 검토에서 남긴 후속 제안에 "제안된 작업 2개도 처리하자"
- **등급**: [GitHub 운영 매뉴얼](../manual/github_operations.md) O3(job 명령·matrix·action). 일반 이슈·계획 절차로 진행한다.

## 1. 현재 상태 (2026-10-28 live 조회)

| 항목 | 값 |
|---|---|
| 저장소 | `paldyn/HanPage`, public, 기본 브랜치 `main`, 통합 브랜치 `devel` |
| `devel` 보호 규칙 | 없음(`GET …/branches/devel/protection` → 404 Branch not protected). required check 영향 없음 |
| `ci.yml` | 루트 workspace 의 Rust·WASM·Studio 검사. `HanPage-Desktop/` 는 다루지 않는다(upstream 동기화 대상) |
| `desktop-release.yml` | `hanpage-desktop-v*` 태그 push 와 workflow_dispatch 에서 macOS·Windows 번들만 만든다 |
| 데스크톱 크레이트 단위 테스트 | 기준 `devel` 8개, PR #157 병합 뒤 35개 — 어떤 PR 검사도 실행하지 않는다 |

기존 실행 경로로 해결할 수 없는 이유(매뉴얼 5.3.1): 데스크톱 크레이트는 루트 workspace 밖이라 `ci.yml` 의 `cargo
clippy --workspace`·nextest suite 가 포함하지 않는다. `desktop-release.yml` 은 PR 에서 돌지 않고 테스트를 실행하지 않는다.

## 2. 설계

- 새 워크플로 `.github/workflows/desktop-check.yml`.
- 트리거: `pull_request`(base `main`·`devel`, `HanPage-Desktop/src-tauri/**`, 저장소 루트의 `rust-toolchain.toml`·`rustfmt.toml`,
  이 워크플로 변경) + `workflow_dispatch`. 루트 두 파일은 이 크레이트의 툴체인과 포맷 규칙을 정한다(작업 디렉터리에서 위로 찾는다).
  push 트리거는 두지 않는다 — `scripts/tests/test_postmerge_duration_workflow.py` 가 devel push 구독을 허용 목록으로 막는다.
- matrix: macOS(`macos-14`, 번들과 같은 러너)·Windows(`windows-latest`). 각 플랫폼 전용 cfg 코드를 Clippy 로 본다.
- 단계: `cargo fmt --check`(macOS 만), `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`.
  로컬 검증 게이트와 같은 명령이다.
- 프런트엔드 빌드 없음: 개발 빌드(`custom-protocol` 기능 없음)는 `tauri.conf.json` 의 `devUrl` 을 쓰고 `frontendDist` 를
  포함하지 않는다. `rhwp-studio/dist` 가 없는 깨끗한 checkout 에서 Clippy·테스트가 통과함을 확인했다.
- 보안: top-level `permissions: contents: read`, secret 미사용, checkout `persist-credentials: false`, 외부 action 은
  기존 워크플로와 같은 full commit SHA(`actions/checkout` v7.0.1, `dtolnay/rust-toolchain` stable 브랜치, `Swatinem/rust-cache`
  v2.9.3).
- 캐시: PR 은 복원만, 저장은 `devel` 에서 수동 실행할 때만(기존 신뢰 경계). workflow_dispatch 는 워크플로가 기본 브랜치(`main`)에
  있어야 쓸 수 있으므로, 다음 `devel → main` 승격 전까지 PR 실행은 캐시 없이 돈다.
- concurrency: 같은 PR 의 새 push 가 오면 진행 중인 실행을 취소한다. 그룹은 PR 번호로 묶는다(브랜치 이름은 fork PR 끼리 겹친다).
- 툴체인: `dtolnay/rust-toolchain` 은 `RUSTUP_TOOLCHAIN` 을 두지 않으므로 루트 `rust-toolchain.toml` 이 정한다. action 은 같은 버전을 미리 설치한다.

## 3. 영향 분석

| 질문 | 답 |
|---|---|
| 실행 event | 데스크톱 크레이트를 바꾼 PR, 수동 실행. 다른 PR 에서는 실행하지 않는다 |
| required check | `devel` 에 보호 규칙이 없어 병합을 기계적으로 막지는 않는다. 메인테이너가 PR 에서 결과를 확인한다 |
| 비용 | public 저장소의 GitHub-hosted runner. 데스크톱 크레이트를 바꾼 PR 마다 macOS·Windows 1회씩 |
| 신뢰 경계 | fork PR 코드도 읽기 권한·secret 없이 빌드·테스트만 한다. PR 은 캐시를 저장하지 않는다 |
| promotion | `scripts/workflow_promotion_policy.json` 은 upstream(edwardkim/rhwp) 워크플로만 등록한다. HanPage 전용인 `desktop-release.yml` 처럼 등록하지 않으며, 다음 `devel → main` 승격 때 함께 다룬다 |
| rollback | 워크플로 파일 삭제 PR. 다른 워크플로·설정에 의존하거나 쓰지 않는다 |

## 4. 검증 계획

- YAML 구조, 저장소의 워크플로 정책 테스트(`scripts/tests` 의 workflow·ci-impact·promotion 계열)를 로컬에서 실행한다.
- 로컬에서 같은 명령(macOS)과 깨끗한 checkout 컴파일을 확인한다.
- PR 에서 실제 실행: 두 job 의 step run/skip(Format 은 macOS 만), 로그의 테스트 건수(35)로 0건 실행이 아님을 확인하고 소요 시간을 기록한다.
- 엔진·Studio·데스크톱 소스는 바꾸지 않아 해당 제품 회귀는 비해당이다.
