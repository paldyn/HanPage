# Task #156 (M100) — Desktop 문서마다 창 하나 최종 보고서

Issue: #156 · 계획: [task_m100_hp156.md](../plans/task_m100_hp156.md) · base `2b23c21c7` · 코드 커밋 `b608f5508`(1차) + `c83f4e369`(검토 반영)

## 1. 결과 요약

다른 한글 파일을 열면 새 창에서 연다. 지금 창(가장 최근에 포커스한 창)이 손대지 않은 빈 창이면 그 창에서 열고,
이미 어떤 창에 열린 파일이면 그 창을 앞으로 가져온다. Finder·탐색기 더블클릭과 연결 프로그램, macOS 최근 문서,
파일 > 열기, 끌어다 놓기가 같은 규칙을 따른다. 파일 > 새 문서·AI 도우미·문서 비교·자동 저장 복구는 지금 창에서 바꾼다.

창이 여럿이 되면서 함께 필요한 동작도 넣었다: 저장하지 않은 창을 닫거나 Cmd+Q 로 끝낼 때 저장 확인, macOS 메뉴
명령은 가장 최근 창에만, 업데이트 적용 전 다른 창 확인, 업데이트 준비 알림은 앱 전체에서 한 번, 창 제목은 문서 이름,
새 창은 가장 최근 창에서 비스듬히 빈 자리에, 자동 저장 복구 제안과 창 크기·위치 기억은 첫 창만.

## 2. 변경 내역

| 파일 | 내용 |
|---|---|
| `HanPage-Desktop/src-tauri/src/windows.rs`(새) | Tauri 형식에 의존하지 않는 `Registry`: 창 목록·최근 순서, 창별 대기 문서, `route`(`Focus`·`Into`·`New`), 배정 예약(`reserved`, 대기 중일 때만 같은 파일 판단에 사용)과 확정 경로(`path`) 분리, 시작 단계(`Booting`→`Startup`→`Ready`), 닫기 판단(`Allow`·`Ask`·`Wait`·`ConfirmNative`), Cmd+Q 종료 진행(`begin_quit`·`next_quit_step`·`abandon_quit`) |
| `HanPage-Desktop/src-tauri/src/lib.rs` | 앱 전체 큐(`PENDING_DOCUMENTS`)를 레지스트리로 교체. 파일 연결·최근 문서·열기 대화상자·넘겨받은 문서를 `deliver` 하나로 보냄. 새 창은 첫 창 설정 복제·별도 스레드 생성·실제 생성 확인(실패 시 그 문서를 알리고 창이 없으면 종료), 기준 창 대기와 비스듬한 위치(`next_cascade_position`). 메뉴는 가장 최근 창에만(`emit_to`), 닫기 요청은 저장하지 않은 창이면 막고 웹뷰에 묻거나 응답이 없으면 네이티브 확인. macOS 종료 메뉴를 직접 처리. 창 생성 중 마지막 창이 닫혀도 종료하지 않음. 업데이트 사전 확인·적용 전 다른 창 확인, 알림 1회 claim |
| `HanPage-Desktop/src-tauri/capabilities/default.json` | `windows: ["main", "doc-*"]` |
| `rhwp-studio/src/core/desktop-bridge.ts` | 창 레이블, 창 전용 구독(`listen(…, {target})`), 창 상태 보고(빈 창·변경 있음·열린 파일), 빈 창 판단(문서 없음·편집/저장 이력 없음·여는 중 아님·깨끗함), 받을 수 없는 대기 문서는 새 창으로 넘김, 대기 문서 열기 뒤 재보고(실패·취소 시 예약 해제), 닫기 요청 응답(묻는 중 재확인 응답), 창 제목 동기화, 업데이트 사전 확인·알림 claim |
| `rhwp-studio/src/main.ts` | 문서 교체·변경·새 문서·여는 중 훅, 시작 흐름 뒤 준비 보고, 두 번째 이후 창은 자동 저장 복구 생략, 끌어다 놓기 새 창 전환, 닫기 확인(기존 저장 확인 재사용, 저장 안 함이면 그 초안 삭제), 업데이트 적용 실패 시 AI 도우미 복구 |
| `rhwp-studio/src/ui/update-notice.ts` | 알림은 claim 한 창만, 다른 창의 적용 중 상태에 카드·입력 보호, 다른 창 미저장 시 저장 확인 전에 차단, 적용 실패 훅 |
| `rhwp-studio/src/command/file-open-picker.ts` | 데스크톱은 열기 전에 저장 확인을 하지 않음(지금 문서를 바꾸지 않는다) |
| 테스트 | `tests/desktop-bridge.test.ts`·`desktop-multi-window.test.ts`·`file-open-picker-fallback.test.ts`·`native-edit-menu.test.ts`, E2E `desktop-multi-window.test.mjs`(새)·`desktop-update-ui.test.mjs` |
| 문서 | `HanPage-Desktop/CHANGELOG.md`(미출시)·`README.md`("문서마다 창 하나")·`mydocs/manual/desktop_auto_update.md` |

공통 결과: 문서를 받을 창은 Rust 레지스트리의 `route` 한 곳에서만 정한다. 웹뷰는 상태를 보고하고, 받을 수 없게 된
문서는 `Target::NewWindow` 로 돌려보낼 뿐 다른 창을 직접 고르지 않는다. 파일 > 열기도 대기 문서 경로로 바꿔
파일 연결과 같은 `deliver`·같은 웹뷰 열기(`loadBytes`)를 쓴다.

## 3. 검증

### 3.1 자동 검사 (코드 head `c83f4e369`)

| 명령 | 결과 |
|---|---|
| `cargo fmt --check` (HanPage-Desktop/src-tauri) | 통과 |
| `cargo clippy --locked --all-targets -- -D warnings` (`CARGO_TARGET_DIR=target/pr-review`) | 통과 |
| `cargo test --locked` | 26 PASS (`windows.rs` 17, `lib.rs` 9) |
| `npx tsc --noEmit` (rhwp-studio) | 통과 |
| `npm test` (rhwp-studio) | 2,619 중 2,617 PASS / 0 FAIL / 2 SKIP |
| E2E `desktop-multi-window` (실제 Studio + 모의 Tauri, Chrome headless) | 14 PASS / 0 FAIL |
| E2E `desktop-update-ui` | 149 PASS / 0 FAIL |
| E2E `document-title-issue6566` | 13 PASS / 0 FAIL |
| E2E `responsive` | 2,666 PASS / 0 FAIL |

루트 엔진 크레이트는 바꾸지 않아 엔진 Rust lint 묶음은 비해당이다. 조판·렌더링 변경이 없어 Visual Sweep 도 비해당이다.

### 3.2 변형 검사

검토 반영 동작을 하나씩 되돌려 새 검사가 잡는지 확인했다(실행 뒤 원본 복구를 바이트 비교로 확인).

| 되돌린 동작 | 결과 |
|---|---|
| 빈 창 판단의 편집·저장 이력(`touched`) 조건 | 단위 FAIL("한 번 편집하거나 저장한 창, 문서를 여는 중인 창은 손대지 않은 창이 아니다") |
| 빈 창 판단의 여는 중(`loading`) 조건 | 같은 검사 FAIL |
| 저장할 때 `touched` 설정 | 같은 검사 FAIL |
| 편집할 때 `touched` 설정 | 같은 검사 FAIL |
| 대기 문서 열기 뒤 재보고 | 단위 FAIL("대기 문서를 열지 못하면(암호 취소·읽기 실패) 이 창의 예약을 풀도록 다시 보고한다") |
| 업데이트 사전 확인(`updatePreflight`) | E2E `desktop-update-ui` 가 새 케이스("다른 창의 미저장 문서는 이 창의 저장 확인·적용보다 먼저 막는다")의 대기에서 시간 초과로 중단(rc=1) |

레지스트리 단위 검사와 `desktop-multi-window` E2E 는 이 작업에서 새로 만든 형식·창 전용 구독·상태 보고를 검사하므로
base 에는 검사 대상이 없다.

### 3.3 실기 확인 (macOS, 테스트 전용 식별자 `com.paldyn.hanpage.devtest` debug 번들)

| 시나리오 | 관측 |
|---|---|
| 콜드 스타트 문서 2개(`open -a … A B`) | A → `Into("main")`, B → `New("doc-1")`, 새 창 생성 확인 통과, 두 창 모두 대기 문서를 가져감. 18회 정상(새 프로필 11회, 진단 로그를 넣은 빌드 13회 포함) |
| 같은 파일 다시 열기 | A → `Focus("main")` |
| 세 번째 문서 | C → `New("doc-2")`, 위치 (480,144): 기준 main (424,88)에서 doc-1 자리 (452,116)를 한 칸 더 비킴 |
| 1회 멈춤(첫 실행) | 두 번째 창이 대기 문서를 가져가지 않음. 시스템 로그 근거: WindowServer 가 터미널에서 띄운 실행의 활성화를 거부("CPS: Denying HanPage the right to be in front")해 창들이 다른 앱 뒤에 열렸고, 가려진 두 번째 창의 웹뷰는 생성부터 `viewIsBecomingInvisible` 상태로 남아 WebKit 이 웹 프로세스를 일시 정지(`prepareToSuspend` → `ready to suspend`)했다. 새 프로필의 첫 실행이라 네트워크 리소스 두 개(9.7·11.6 MB, 웹 글꼴로 보인다)를 내려받느라 시작이 길어 정지 전에 대기 문서까지 가지 못했다. 사용자가 Finder 등에서 직접 열면 활성화가 허용되어 창이 앞에 뜬다. 가려진 창이 다시 보이면 이어서 연다는 점은 WebKit 동작으로 설명되지만 이번에 직접 재현하지는 못했다 |

정리: 테스트 번들 등록 해제·삭제, 테스트 데이터 삭제. `.hwp` 기본 앱은 `/Applications/HanPage.app` 그대로이고 실제
HanPage 데이터는 건드리지 않았다.

### 3.4 독립 검토

1차 구현을 라우팅·창 수명, 닫기·종료, 웹뷰 상태, 업데이트 관점에서 독립 검토하고 각 지적을 반박 검증했다.
확인된 결함을 `c83f4e369` 에 반영했다.

| 지적 | 반영 |
|---|---|
| 열기에 실패·취소한 문서의 예약이 남아 그 창이 빈 창으로 돌아오지 않고, 같은 파일은 계속 그 창으로만 감 | 예약과 확정 경로 분리, 대기 문서 열기 뒤 재보고 |
| 새 문서를 저장해 깨끗해지면 다시 빈 창으로 보여 다음 파일이 그 창을 차지 | 편집·저장 이력(`touched`) |
| 문서를 여는 중인 창이 다른 문서도 받음 | 여는 중(`loading`) |
| 창이 받을 수 없어 돌려준 문서가 같은 창으로 `Focus` 됨 | `Target::NewWindow` 는 항상 새 창(이미 다른 창에 열린 파일만 그 창으로) |
| Cmd+Q 를 다시 누르면 묻는 중인 창의 상태를 덮어씀 | 묻는 중인 창이 있으면 새로 묻지 않음 |
| 응답하지 않는 창은 영영 닫을 수 없음 | 받았다는 응답이 8초 없으면 다음 닫기에서 네이티브 확인창 |
| 종료 중 남은 창이 모두 깨끗해져도 종료가 끝나지 않음, 문서를 새로 열어도 종료가 이어짐 | `next_quit_step`, 새 창·생성 중 종료 요청 시 종료 중단 |
| 다른 스레드에서 만든 창의 실패를 감지하지 못함 | 생성 확인, 실패 시 그 문서를 최근 창에 알림, 창이 없으면 종료 |
| 창이 하나도 없을 때 다시 실행하면 아무 일도 없음(Windows) | 빈 창을 띄움 |
| 최소화 창(Windows -32000)을 위치 기준으로 씀, 작업 영역 왼쪽·위 넘침 | 기준에서 제외, 넘치면 되돌림 |
| 최소화된 창에 닫기 확인이 묻힘 | 앞으로 가져온 뒤 묻기 |
| 업데이트: 다른 창 확인보다 먼저 AI 도우미를 내리고, 적용이 실패해도 다시 올리지 않음 | 사전 확인을 먼저, 적용 실패 시 복구 |

수정하지 않고 한계로 남긴 지적: 자동 저장 초안은 창을 구분하지 않는 저장소 하나를 쓰므로, 첫 창이 다시 시작하며
복구를 제안할 때 다른 창의 초안과 구분하지 않는다. 두 번째 이후 창이 비정상 종료돼 다시 불러와도 그 창에서는 복구를
제안하지 않는다. macOS Dock 종료·로그아웃과 Windows 로그오프는 창에 묻지 않는다. 설정은 마지막으로 바꾼 창의 값이 남는다.

## 4. 미검증·한계

- Windows 실기(WebView2): 창 생성 스레드, 닫기·Alt+F4, 단일 인스턴스 두 번째 실행의 창 선택. 데스크톱 번들은 PR CI 에
  없으므로 `desktop-release` workflow_dispatch 로 컴파일·번들만 확인한다.
- macOS 실기의 닫기 확인·Cmd+Q·응답 없는 창 네이티브 확인: 이 환경에 보조 접근 권한이 없어 대화상자 조작을 자동화하지
  못했다. 레지스트리 단위 검사와 웹뷰 E2E(취소·저장 안 함)로만 확인했다.
- 파일 > 열기·끌어다 놓기 실기: 네이티브 열기 대화상자와 실제 드래그는 조작하지 못했다. 열기는 파일 연결과 같은
  `deliver`(실기 확인)와 단위 검사, 끌어다 놓기는 E2E 의 합성 `DragEvent` 로 확인했다.
- 실제 업데이트 경로에서 여러 창의 알림 1회·사전 차단: 이 변경을 포함한 버전을 설치한 다음 업데이트부터 확인할 수 있다.
