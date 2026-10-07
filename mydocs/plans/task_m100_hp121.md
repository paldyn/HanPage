# Task #121 (M100) — Windows 파일 연결 첫 실행 문서 열기 수행계획서

- **이슈**: [paldyn/HanPage#121](https://github.com/paldyn/HanPage/issues/121)
- **브랜치**: `claude/desktop-file-open-coldstart` (`origin/devel` `5685104a56a6502b57a8f210f5ea14e8d45b6e88` 분기)
- **작업지시자 보고(2026-10-07)**: "한글 파일을 이 앱으로 열면 앱이 열리고 파일이 바로 보여야하는데 파일이 안떠서 앱에서 파일을 다시 선택해야해"
- **승인**: 같은 날 "둘다 응응" — 이슈·PR 진행 승인과 Windows에서 본 증상임을 확인

## 1. 원인

| 경로 | 동작 | 판정 |
|---|---|---|
| macOS 파일 연결 | LaunchServices → `RunEvent::Opened` → 전역 `PENDING_DOCUMENTS` → 웹뷰 drain(#50) | 정상. 진단 빌드로 콜드/웜·보호 폴더·한글 공백 파일명에서 문서 렌더(캔버스 잉크)까지 확인 |
| Windows/Linux 두 번째 실행 | `tauri-plugin-single-instance` 콜백이 argv 를 큐에 넣음 | 정상 |
| **Windows/Linux 첫 실행** | 문서 경로가 첫 프로세스 argv 로 오지만 읽는 코드 없음 | **결함** — 빈 문서만 열림 |
| 읽기 실패 | `eprintln` 후 폐기 | 사용자에게 아무 안내 없음 |

근거: Tauri NSIS `APP_ASSOCIATE` 명령 `$INSTDIR\HanPage.exe "%1"`(tauri-bundler `installer.nsi`),
single-instance 콜백은 두 번째 인스턴스에서만 호출. macOS 에서 실행 파일을 경로 인자와 직접 실행하면
같은 첫 실행 argv 경로가 재현된다(수정 전 `take_pending -> 0`, 빈 문서).

## 2. 범위

1. `setup()`에서 `std::env::args_os()`의 `.hwp`/`.hwpx` 경로를 큐에 넣는다. single-instance 와 같은
   `document_paths_from_args` 를 쓰고 상대 경로는 인자를 넘긴 프로세스의 작업 디렉터리 기준으로 푼다.
2. 시작 문서보다 먼저 도착한 single-instance 경로(WebView2 생성 중 메시지 처리)를 `StartupGate`로 모아
   시작 문서 뒤에 넣는다.
3. 업데이트 설치 프로그램은 새 버전을 처음 argv 로 다시 띄운다(NSIS `/ARGS`, macOS `restart()`).
   적용 직전 `RelaunchMarker`(인자·목표 버전·시각)를 store 에 남기고 시작 시 한 번 확인해 다시 열지 않는다.
   NSIS 가 따옴표를 벗겨 공백 경로가 쪼개져도 공백으로 이은 키로 비교한다.
4. 읽지 못한 파일은 같은 큐의 실패 항목(`error`)으로 넣어 웹뷰가 안내 토스트를 띄운다.

비범위: 다른 확장자 연결, Linux 패키지, macOS 동작 변경.

## 3. 검증 계획

- 데스크톱 크레이트 `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, 단위 테스트.
- studio `tsc --noEmit`, 전체 `npm test`, 새 desktop-bridge 테스트의 수정 전 FAIL/후 PASS.
- macOS 실제 앱: argv 첫 실행 수정 전후, single-instance 상대 경로, 읽기 불가·없는 파일 안내(웜/콜드),
  LaunchServices 회귀, 업데이트 재실행 표식(일치·만료·버전 불일치·쪼개진 인자).
- Windows 실기기 실행은 이 환경에서 불가하므로 미검증으로 기록한다.
