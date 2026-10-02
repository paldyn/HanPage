# Desktop 업데이트 UI 개선 로컬 검증

- 요청: 앱에 맞는 알림 디자인, `업데이트` 버튼, 나중에 이후 재진입, 다운로드·적용 진행 표시.
- 선행 Issue: [#59](https://github.com/paldyn/HanPage/issues/59). 같은 증상 선행 검색과 열린 PR 확인을 수행했다. 새 공개 이슈 생성·담당자 지정은 자동 승인 검토가 명시적 외부 게시 승인이 없다는 이유로 거절했다. 새 이슈·댓글·push·PR·release는 수행하지 않았다. 이 기록은 이슈 번호를 임의로 부여하지 않은 로컬 후속 작업이다.
- 브랜치: `codex/desktop-update-ui-20261002`, 실제 fork 대상 base `origin/devel` = `71f327f2ffdbc20bc3b8926d136ead049caeeb68`.
- 원본 checkout의 `codex/upstream-sync-20261001`/HEAD와 로고 등 사용자 변경 39개는 그대로 보존했다. 별도 clean managed worktree에서 수정했다.

## 구현과 검증 근거

| 요구 | 실제 경로·검사 | 판정 |
| --- | --- | --- |
| 알림·버튼 | 전용 카드와 테마 토큰, Windows/macOS `업데이트` 버튼 | 충족 |
| 나중에 재진입 | 기존 macOS 메뉴와 모든 Desktop의 상태 표시줄 버튼, 같은 ready 파일 재사용 | 충족 |
| 다운로드 진행 | SDK 청크 누산 → `UpdateSlot` 저장 → 최대 100ms 간격 `update-status` → bridge → 실제 progressbar. 41/100 MiB = 41%, Content-Length 미상은 용량·불확정 표시만 | 충족 |
| 검증·적용 | SDK 완료 callback의 서명 검증 직전 `verifying`, 검증 성공 후 `ready`; 선택 후 `applying` 즉시 표시하고 blocking worker에서 교체 | 충족 |
| 중복·오류 | 상태와 ready 파일을 같은 락에서 claim. 실패 시 원본 Arc/파일을 복원, 다시 업데이트로 재시도 | 충족 |
| 문서 보존 | 기존 저장 확인 재사용. 취소·저장 취소·실패 시 적용하지 않음. 적용 중 root inert·window capture·Native 메뉴/파일 callback 잠금, 실패 후 입력 복구 | 충족 |
| 일반 웹 | Desktop 전용 카드·진입점·IPC가 생성되지 않음 | 충족 |
| 문서 조판 | Rust 엔진·조판·golden·래칫·입력 문서 미변경. 조판 Visual Sweep은 비해당; 실제 앱 UI 캡처로 검증 | 비해당 |

원본 `71f327f`의 업데이트 모듈 두 개를 실제 브라우저에 주입하면 Windows 버튼은 `지금 설치`, 상시 진입점과 적용 막대가 없다. 새 카드에서 세 계약이 통과한다. baseline 테스트는 새 main의 `isApplyingUpdate` import에 false 호환 export만 붙이며 원본 업데이트 동작을 바꾸지 않는다.

## 실행 결과

- Node 22.22.1: `npm test` — 1,834 PASS / 2 SKIP / 0 FAIL.
- `npm run build:desktop` — TypeScript와 Vite PASS.
- 실제 Studio·WASM·문서 입력·직렬화 저장 + Tauri IPC/상태 이벤트 대역 브라우저 검사 — 55 PASS / 0 FAIL. 설치·재시작은 실행하지 않았다. 최종 source commit에서 같은 검사와 대표 PNG를 다시 고정한다.
- 제품 상태 모듈의 회귀 7개 — PASS. 호스트에 `cargo nextest`가 없어 focused 검사에 한해 `cargo test --locked --profile release-test --test regression_suite_003 hanpage_desktop_update_state:: -- --nocapture`를 사용했다. 전체 nextest 대체로 보고하지 않는다.
- `cargo fmt --all`, fmt check, root native Clippy, WASM32 Clippy, workspace build, workspace all-targets Clippy — 순차 PASS.
- Desktop 독립 crate fmt check / `cargo clippy --locked --manifest-path HanPage-Desktop/src-tauri/Cargo.toml --all-targets -- -D warnings` / build — PASS.
- `rust-test-suite-manifest --prepare` 및 base `71f327f` 고정 `--check` — 48/48 integration target PASS. 새 회귀 원본이 제품 파일을 path import하므로 실제 모듈 harness 실행을 먼저 확인하고, 해당 원본의 `path_attr`·`root_mod`만 허용했다. 예산·기준값은 늘리지 않았다. generated harness·manifest는 커밋하지 않는다.
- fresh WASM은 저장소 루트 wrapper의 기본 `--out-dir pkg`로 생성했다. 루트 pkg와 Studio public glue/WASM SHA가 각각 일치한다. 엔진은 0.8.6 그대로이며 모든 Cargo 작업은 기존 공유 `target/pr-review`를 순차 재사용했다.
- 정상 제품 7 PASS와 두 개의 격리 변이 음성 대조: Applying 기록 제거/파일 복원 제거 모두 정상 컴파일 후 실제 의미 assertion이 FAIL했다. 최초 사본은 Cargo binary 재사용 때문에 검사로 인정하지 않고, 고유 test target으로 재실행한 관측만 검출 근거로 사용한다.

## 남은 범위

로컬 source와 UI 검증 완료. 실제 서명된 새 버전 다운로드·앱 교체·Windows 설치·새 공개 릴리스는 미실행이다. 앱 버전은 0.8.7을 유지했다. 다음 업데이트 배포는 신규 승인과 정확한 버전·PR head·필수 CI·서명/공증/공개 산출물 검증을 거쳐야 한다. 이 UI는 변경을 포함한 버전을 설치한 이후의 알림부터 적용된다.

최종 code SHA, 명령 로그, source hash, 대표 PNG, 음성 대조와 사용자 변경 보존 기록은 같은 이름의 `assets/desktop_update_ui_20261002/`에 연결한다.
