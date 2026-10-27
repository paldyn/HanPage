# Task #154 — upstream 통합 검증 기록 (2026-10-27)

- 이슈: [HanPage #154](https://github.com/paldyn/HanPage/issues/154) · 계획: [task_m100_hp154](../plans/task_m100_hp154.md)
- 브랜치: `claude/upstream-sync-20261027`
- 기존 HanPage: `9b4f20a69c5eec46ce8b3f2235b8ecc0b93375a0` (Desktop 0.8.11)
- 지난 upstream 고정점: `02530b9ed567a44663edb26c65fb565c4a79f00d` (0.8.6)
- 가져온 upstream/devel: `faa885e8b54ffc7cdf4196cd43a3103f362499a2` (엔진·Studio 0.8.7, 커밋 3,741개, 병합 PR 55개)
- 분류: [ownership](../working/task_m100_hp154_ownership.json), 적용: [sync manifest](../working/task_m100_hp154_sync_manifest.json)

## 1. 통합

#110 과 같은 방식이다. upstream 이력을 merge 하지 않고, 기존 devel 을 부모로 최신 upstream 트리를 수용한 뒤 지난 고정점 대비
HanPage 델타만 이식했다. 분류는 지난 고정점(P) 과 devel(D) 의 차이만 HanPage 델타로 보므로, upstream 이 지운 475경로를
되살리지 않는다.

| 분류 | 경로 | 결과 |
| --- | --- | --- |
| HanPage 추가 / HanPage만 수정 | 490 / 22 | D 그대로. blob·mode 512경로 일치 확인 |
| 양쪽 수정 | 33 | 3-way 28 자동, 5 수동: `Cargo.toml`(저장소 URL만 HanPage, upstream `readme` 유지), Studio `package.json`(upstream 버전 + cross-env), lock(upstream lock 에 cross-env 7.0.3 항목만 추가 — npm 재생성은 `libc` 필드를 지워 쓰지 않음), `ci.yml`, 오늘할일 |
| 같은 날짜 오늘할일 | 6 | upstream 파일 유지, HanPage 기록은 `mydocs/orders/archives/<날짜>_hanpage.md` 로 이전·상대 링크 보정 |

엔진 `src/`·`crates/`·`Cargo.lock` 은 upstream 과 같다. `tests/` 는 HanPage 3경로만 다르다: Desktop 업데이트 상태 테스트
(`tests/cases/hanpage_desktop_update_state.rs`, #115), 그 suite 정책 항목, #110 의 genuine-hidden 코퍼스 보정. 루트
`Cargo.toml` 은 저장소 URL 한 줄만 다르다. `rhwp-studio/public/rhwp.js` 는 upstream 커밋본을 그대로 두었다 — upstream 이
#8228 뒤 추가한 export 를 반영하지 않은 생성물이며, 로컬 검증은 fresh wasm-pack 빌드로 했다.

## 2. 의미 검토와 보정

글자 충돌 없이 합쳐진 파일을 다섯 영역(Studio 파일 I/O·PDF, 폰트·빌드·UI, CI·정책·테스트, 패키지·문서, upstream 신규 기능
겹침)으로 독립 검토하고 주장마다 반박 검증했다. 확인된 문제와 보정:

| 문제 | 보정 | 검증 |
| --- | --- | --- |
| CI 정책 bootstrap 이 지난 동기화 값에 고정돼 이번 PR 에선 비활성인데, 그 계약 테스트가 코퍼스 파일 해시를 비교해 Lint 를 항상 실패시킴 | bootstrap 은퇴: upstream `ci.yml`·manifest 테스트 사용, helper·테스트 삭제. 일반 경로(base=devel)로 정책 검사 | 로컬 manifest `--check`·unit-tier `--check` (base `9b4f20a69`) |
| Studio 폰트 시간 제한 테스트 기대값이 upstream 의 새 필수 글꼴(옛한글, #8191) 을 몰라 실패 | 기대 집합에 CRITICAL_FONTS 반영 | 2/2 |
| 8초 예산 뒤 늦게 끝난 CDN face 가 그사이 로컬로 공급된 글꼴(upstream `registerLoadedLocalFontFace`) 을 덮음 | 늦은 face 는 로컬 공급 family 면 등록하지 않음 | 새 테스트: 수정 전 FAIL → 후 PASS |
| 데스크톱 열기 대화상자가 PDF(upstream 가져오기 기본 활성)·HML 을 받지 않음. 드롭은 열리는데 메뉴·최근 문서 재열기는 불가 | WASM 의 PDF 지원 여부를 `cmd_open_document` 까지 전달, 필터 hwp/hwpx/hml(+pdf). Windows `연결 프로그램` argv 도 hml·pdf 수용(파일 연결 등록은 그대로) | Rust 단위 테스트, desktop-bridge·picker 테스트 |
| 데스크톱 저장 필터가 `.hwp` 고정 — HWPX 저장 시 macOS 가 `.hwp` 확장자를 강제(HWPX 원본에도 있던 결함, PDF 는 HWPX 가 기본) | 제안 파일명 확장자로 필터 결정, PDF 경로 저장 거부 | Rust 단위 테스트 |
| 데스크톱 PDF 문서는 handle 이 없어 Ctrl+S 마다 형식 선택을 다시 물음 | 같은 문서 인스턴스(`documentGeneration`) 단위로 변환 형식 기억 | 새 테스트 2 |
| macOS 네이티브 편집 메뉴가 upstream AI 패널 입력창에 커서가 있어도 문서에 적용 | AI 패널·대화상자 입력창이면 문서 명령으로 보내지 않음 | 새 테스트 3 |
| AI 자동 적용 편집이 업데이트 적용 중에도 이어질 수 있음 | 저장 확인 통과 뒤 AI 도우미를 내리고 그사이 편집을 다시 확인 | 소스 계약 테스트 |
| README 엔진 버전 v0.8.6, vscode README 엔진 링크가 HanPage | v0.8.7, edwardkim/rhwp | — |

반박된 주장: AI 시스템 프롬프트 문구, upstream 의 코퍼스 보정 흡수 여부(흡수 안 됨, 중복 없음 — U 엔진에서 코퍼스 테스트 6/6),
CodeQL actions 경고(merge 차단 아님, 기존 상태), 업데이트 알림과 AI 패널 겹침, `view:zoom-100`(이번 동기화와 무관).

## 3. 실행한 검증

모두 통합 작업 트리(엔진·테스트·Studio 소스가 최종 커밋과 같은 상태)에서 실행했다. 고정 target `target/pr-review`,
build jobs 6, nextest test threads 10. 로그는 커밋하지 않는 scratch 경로에 남겼다.

| 검사 | 결과 |
| --- | --- |
| fresh WASM (`scripts/wasm-pack-locked.sh --target web --out-dir pkg`) | 성공(20분). `pkg`·Studio `public` 의 `rhwp_bg.wasm` SHA-256 `3783801092cf2ddcebc17383f21755e8e967d46af6db5f59fc044aa0a6eb07ff` 일치 |
| Studio `tsc --noEmit` | 통과 |
| Studio `npm test` (npm/editor·rhwp-shared/pdf 포함) | 2,605 중 2,603 PASS / 0 FAIL / 2 SKIP |
| E2E `desktop-update-ui` | 140 PASS / 0 FAIL, 브라우저 예외 0 |
| E2E `e2e:responsive` | 2,666 PASS / 0 FAIL |
| E2E `e2e:document-title` | 13 PASS / 0 FAIL |
| 웹 / Desktop frontend 빌드 | 둘 다 성공. 웹은 `sw.js`·manifest·CNAME(`hanpage.paldyn.com`), Desktop 은 SW 없음·상대 경로 |
| HanPage-Desktop crate | `cargo fmt --check`, `clippy --all-targets -D warnings` 경고 0, `cargo test` 8 PASS |
| Rust lint 묶음 | `--prepare`, `fmt --all --check`, native clippy, WASM clippy, workspace build, workspace all-targets clippy 모두 통과 |
| 테스트 정책 | `rust-test-suite-manifest --check`·`rust-unit-test-tiers --check` (base `9b4f20a69`) 통과 |
| 전체 nextest (`--cargo-profile release-test --workspace`) | 11,707 PASS / 0 FAIL / 45 SKIP (테스트 실행 1,290초). 보안 코퍼스는 devel 대비 추가된 samples 212개를 `RHWP_SECURITY_SWEEP_SAMPLES_JSON` 으로 넣어 실제 실행 |
| CI 계약 (Node 9개 파일 245, Python promotion 29) | 모두 PASS. 처음 3건은 `rhwp-vscode`·`frontend-metrics` 의존성 미설치였고 설치 후 PASS |

새 회귀 테스트 중 수정 전 실패를 확인한 것: 늦은 CDN face 테스트(수정 전 FAIL). 데스크톱 열기 PDF 전달·저장 형식 기억·
네이티브 편집 메뉴 테스트는 새 동작을 고정하는 계약 테스트다.

## 4. 미검증·한계

- 실제 Desktop 앱의 OS 열기·저장 대화상자, PDF 열기·HWPX 저장 확장자, macOS 편집 메뉴는 실행하지 않았다. Tauri 명령은
  단위 테스트·대역으로 확인했다. PR 에서 Desktop Release dispatch 로 양 플랫폼 번들 빌드를 확인한다.
- 한컴 PDF 대비 렌더링 fidelity 는 이번 통합의 검증 범위가 아니다(엔진은 upstream 그대로).
- CI 의 신규 sample 보안 스윕은 변경 파일이 GitHub listFiles 상한(3,000)을 넘어 목록이 비게 된다. 로컬 nextest 의 212개
  실행으로 대신했다.
- 작업 트리를 메인 체크아웃 안에 두면 `cargo fmt --all` 이 vendor 크레이트에서 바깥 `Cargo.toml` 을 workspace 로
  오인한다. 저장소 밖 경로로 옮겨 실행했다(코드 문제 아님).
