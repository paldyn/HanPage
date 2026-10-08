# Task #121 (M100) — Windows 파일 연결 첫 실행 문서 열기 최종 보고서

Issue: #121 · 계획: [task_m100_hp121.md](../plans/task_m100_hp121.md) · base `5685104a56a6502b57a8f210f5ea14e8d45b6e88`

## 1. 결과 요약

Windows 에서 앱이 꺼져 있을 때 파일 연결로 연 문서가 빈 문서로 대체되던 결함을 고쳤다. 첫 실행 argv 를
읽고, 업데이트 재실행·시작 경합·읽기 실패의 경계를 함께 정리했다. macOS 실제 앱에서 Windows 와 같은
argv 경로를 재현해 수정 전후를 확인했으며, Windows 실기기 실행은 미검증이다.

## 2. 변경 내역

| 파일 | 내용 |
|---|---|
| `HanPage-Desktop/src-tauri/src/lib.rs` | `document_paths_from_args`(argv[0] 제외·확장자 필터·상대 경로→보낸 쪽 cwd), `setup()` 첫 실행 argv, `StartupGate`, `RelaunchMarker`(store `relaunch.json`, TTL 600초), 읽기 실패 `error` 항목, 단위 테스트 6개 |
| `rhwp-studio/src/core/desktop-bridge.ts` | 큐 항목의 `error` 를 `notifyOpenFailure` 로 전달하고 열지 않음 |
| `rhwp-studio/src/main.ts` | `notifyOpenFailure` → "파일을 열 수 없습니다: 이름\n사유" 토스트 |
| `rhwp-studio/tests/desktop-bridge.test.ts` | 실패 항목 알림·뒤 문서 순서 보존 테스트 |
| `HanPage-Desktop/CHANGELOG.md` | 미출시 항목 |

공통 결과: 문서와 실패는 같은 전역 FIFO 큐를 쓰므로 콜드 스타트(웹뷰 생성 전)에도 유실되지 않고 도착 순서가 유지된다.
기존 drain 계약(`cmd_take_pending_documents` 1개 명령)은 바꾸지 않았다.

## 3. 검증

### 3.1 자동 검사

| 명령 | 결과 |
|---|---|
| `cargo fmt -- --check` (src-tauri) | 통과 |
| `cargo clippy --locked --all-targets -- -D warnings` (src-tauri, `target/pr-review`) | 경고 0 |
| `cargo test --locked` (src-tauri) | 6 PASS |
| `npx tsc --noEmit -p tsconfig.json` (rhwp-studio) | 통과 |
| `npm test` (rhwp-studio) | 1,837 중 1,835 PASS / 0 FAIL / 2 SKIP |
| 새 desktop-bridge 테스트를 수정 전 `desktop-bridge.ts` 로 실행 | FAIL(실패 항목을 빈 문서로 열려 함) → 수정 후 PASS |

루트 엔진 크레이트(`src/`, 루트 `Cargo.toml`)는 바꾸지 않아 루트 Rust lint 묶음은 실행하지 않았다.

### 3.2 macOS 실제 앱

검증 빌드는 최종 코드에 웹뷰 콘솔·토스트를 파일로 내보내는 임시 코드만 더한 것이다(커밋하지 않음).
근거는 Rust `recent.json`(네이티브 읽기 성공 시 기록)과 웹뷰 로그의 `문서 로드`·토스트다.

| 경우 | 수정 전 | 수정 후 |
|---|---|---|
| 실행 파일 + 절대 경로 인자(Windows 첫 실행과 같은 경로) | `take_pending -> 0`, 빈 문서 | 문서 로드 1쪽, recent 최상단 |
| 실행 중 두 번째 실행, 다른 폴더의 상대 경로 | — | single-instance 로 열림, 보낸 쪽 cwd 기준 경로 |
| 읽기 권한 없는 파일(웜) / 없는 파일(콜드) | 조용히 무시 | 안내 토스트, 콜드는 이어서 빈 문서 |
| LaunchServices 콜드 스타트(한글·공백 파일명, `~/Downloads`) | 정상 | 정상(회귀 없음) |
| 재실행 표식 일치 / 쪼개진 인자 일치 | — | 다시 열지 않음, 표식 즉시 삭제 |
| 표식 없음 / 만료(700초) / 버전 불일치 | — | 정상적으로 엶 |
| 표식 없이 쪼개진 인자(참고) | — | "최종.hwp 없음" 토스트 — 표식이 막는 현상 |

수정 전 진단(진단 빌드)에서 macOS 는 콜드·웜·보호 폴더·한글 공백 파일명 모두 캔버스 잉크·상태 표시줄 파일명까지
확인되어 macOS 경로는 원인이 아님을 확인했다.

### 3.3 독립 검토

Rust 수명주기·프런트 계약·Windows 실행 경로 3관점 검토와 반박 검증을 거쳐 확인된 결함 2건
(업데이트 재실행 시 재열기·공백 경로 분할, 시작 경합)을 `RelaunchMarker`·`StartupGate`로 반영했다.
macOS 이중 열기 등 4건은 실행·소스 근거로 기각됐다.

## 4. 미검증·한계

- Windows 실기기에서 파일 연결 첫 실행, NSIS 업데이트 재실행, Explorer 다중 선택 열기는 실행하지 못했다.
  NSIS 템플릿·tauri-plugin-updater 소스 대조와 macOS 동일 경로 재현으로 대신했다.
- 시작 경합(`StartupGate`)은 단위 테스트로만 확인했다.
- 테스트 중 사용자 macOS 앱의 Rust 최근 문서 목록이 테스트 파일로 채워져 원래 항목 일부가 밀려났다.
  확인된 2개만 복원했고, 웹뷰 최근 목록은 앱 메뉴로 비우도록 안내했다.
