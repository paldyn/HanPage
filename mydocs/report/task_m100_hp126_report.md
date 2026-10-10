# Task #126 (M100) — Desktop 업데이트 준비 완료 작은 알림 최종 보고서

Issue: #126 · 계획: [task_m100_hp126.md](../plans/task_m100_hp126.md) · base `93b0a4a48` · 코드 커밋 `770f82561`

## 1. 결과 요약

업데이트 준비가 끝나면 화면 오른쪽 아래에 "새 버전 X 준비됨" 작은 알림을 버전마다 한 번 띄운다.
편집 포커스를 옮기지 않고 8초 뒤 자동으로 닫히며, 큰 업데이트 카드는 여전히 자동으로 열지 않는다.

| 밝은 테마 | 어두운 테마 |
|---|---|
| ![준비 완료 알림 밝은 테마](../pr/assets/task_m100_hp126_update_nudge/after-ready-nudge-light.png) | ![준비 완료 알림 어두운 테마](../pr/assets/task_m100_hp126_update_nudge/after-ready-nudge-dark.png) |

## 2. 변경 내역

| 파일 | 내용 |
|---|---|
| `rhwp-studio/src/ui/update-notice.ts` | `#desktop-update-nudge`(role=status, aria-live=polite), `maybeNudge`(버전당 1회·카드 확인 버전 억제·모달 미루기), `hideNudge`(포커스 안에서 닫힐 때만 들어온 위치로 복원, 모달 중 비복원), 8초 타이머(실제 mousemove hover·내부 포커스 시 유지), 모달 감시 `MutationObserver`, window capture 키 처리(Esc 닫기, Enter/Space/Tab 전파 차단) |
| `rhwp-studio/src/styles/update-notice.css` | `.dialog-update-nudge`(fixed, 오른쪽 16px·아래 36px, z-index 8900, 폭 `min(320px, 100vw - 32px)`, 등장 애니메이션·reduced-motion 해제), 버튼 색은 카드 버튼과 공유 |
| `rhwp-studio/e2e/desktop-update-ui.test.mjs` | `nudgeState` 도우미, 초기 ready 검사, 새 시나리오 "준비 완료 작은 알림 · 버전당 1회 · 포커스 유지 · 자동 닫힘 · 버튼" |
| `HanPage-Desktop/CHANGELOG.md` · `README.md` · `mydocs/manual/desktop_auto_update.md` | 미출시 항목, 사용자 경험 설명 |

공통 결과: 알림은 기존 `receiveStatus` 상태 전이 하나에서만 결정한다. `ready` 가 아닌 상태로 바뀌면 미루기·감시·알림을
모두 정리하므로 카드·진입점·알림이 서로 다른 상태를 보이지 않는다.

## 3. 검증

### 3.1 자동 검사 (코드 head `770f82561`)

| 명령 | 결과 |
|---|---|
| `npx tsc --noEmit -p tsconfig.json` (rhwp-studio) | 통과 |
| `npm test` (rhwp-studio) | 1,837 중 1,835 PASS / 0 FAIL / 2 SKIP |
| `CHROME_PATH=… node e2e/run-with-vite.mjs -- node e2e/desktop-update-ui.test.mjs --mode=headless` | 126 PASS / 0 FAIL, 처리되지 않은 브라우저 예외 0 |
| 같은 E2E 를 변경 전 `update-notice.ts`/`.css` 로 실행 | 새 알림 검사 FAIL("초기 ready 조회는 … 작은 준비 알림을 한 번 띄운다" 등) |
| 같은 E2E 를 검토 반영 전 구현으로 실행 | 3 FAIL: 모달 중 미루기, 팝오버보다 아래 쌓임, 자동 닫힘 시 모달 뒤 편집기 포커스 비이동 |

데스크톱 크레이트(`HanPage-Desktop/src-tauri`)와 루트 엔진 크레이트는 바꾸지 않아 Rust lint 묶음은 비해당이다.

### 3.2 E2E 시나리오와 관측

| 주장 | 검사 |
|---|---|
| 다운로드 중에는 띄우지 않는다 | downloading 상태에서 알림 없음 |
| 준비 완료 시 1회, 포커스 유지 | ready 0.9.0 → 표시, `document.activeElement` 불변, 위치(오른쪽 아래·상태 표시줄 위·화면 안) |
| 같은 버전은 다시 띄우지 않는다 | [나중에] 후 같은 0.9.0 재수신 → 미표시 |
| 자동 닫힘 | 0.9.1 을 멈춘 커서 아래에 띄워도 약 8초 뒤 닫힘 |
| hover·Esc | 실제 마우스 이동으로 올리면 유지, Esc 로 닫고 포커스 복원 |
| [업데이트] | 카드 + 저장 확인, 취소 시 적용 안 함, 카드가 열려 있으면 알림 없음 |
| 모달 미루기·쌓임 | 정보 모달 중 0.9.5 수신 → 미룸, Esc 로 모달 닫으면 표시, z-index < 9000 |
| 모달 뒤 포커스 | 모달 열린 채 자동 닫힘 → 편집기로 포커스 이동 없음 |
| 플랫폼 문구 | Windows 본문에 "설치 프로그램", 웹 빌드는 알림 요소 없음 |

### 3.3 독립 검토

구현 뒤 독립 검토에서 확인된 결함(모달 뒤 편집기로의 포커스 이동,
모달 아래에서 사라지는 알림, 멈춘 커서 밑의 영구 표시, 찾기 대화상자의 Esc 가로채기, 명령 팔레트보다 위 쌓임)을
모두 반영하고 위 E2E 로 고정했다.

## 4. 미검증·한계

- 실제 Tauri 앱의 업데이트 경로에서 표시되는 모습은 확인하지 못했다. 이 기능은 앱 안에 들어 있으므로 이 변경을
  포함한 버전(0.8.11 예정)을 설치한 뒤 다음 업데이트부터 보인다. 0.8.10 → 0.8.11 은 기존 방식으로 안내된다.
- E2E 는 모의 Tauri 브리지와 Chrome headless 에서 실행했다. WebKit(macOS)·WebView2(Windows) 실기기 표시는 미검증이다.
