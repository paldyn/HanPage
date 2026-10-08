# PR #123 리뷰 — Windows 파일 연결 첫 실행 문서 열기 (#121)

## 판정

**승인·병합 완료.** 검토 head `d298a41cbfd65696fcb0615e8f3acc0124a69b19` 의 GitHub CI 30 성공·4 건너뜀, MERGEABLE/CLEAN 을 확인하고
2026-10-08 작업지시자 승인("응")으로 squash 병합했다(merge `003431134da12a463907c2a0721d70e8b539b5c8`).
#121 은 Windows 실기기 확인 전까지 열어 둔다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#123](https://github.com/paldyn/HanPage/pull/123) / enxec / paldyn/HanPage devel |
| 고정 base | `5685104a56a6502b57a8f210f5ea14e8d45b6e88` |
| 코드 head·문서 head | `0e384c1d23272a6802dffb68c29373b11d2f22af` / `2f752eb1e6119549fbefe08c1707f7f44670b313` |
| 관련 이슈 | [#121](https://github.com/paldyn/HanPage/issues/121) (Windows 실기기 확인 후 close) |
| 경로 | collaborator self-PR, 작업지시자 승인 "둘다 응응"(2026-10-07) |

## 구현 주장과 검증

[결과 보고](../../report/task_m100_hp121_report.md)의 표를 정본으로 한다. 요약:

| 주장 | 적용 경로 | 독립 기대값 | 실제 검사 | 판정 |
| --- | --- | --- | --- | --- |
| 첫 실행 argv 문서를 연다 | `setup()` → `document_paths_from_args` → `open_path` → 큐 → 웹뷰 drain | NSIS 연결 명령 `HanPage.exe "%1"` | macOS 실행 파일+경로 인자: 수정 전 빈 문서, 수정 후 1쪽 로드 | 충족(Windows 실기기 미검증) |
| 상대 경로는 보낸 쪽 cwd 기준 | single-instance 콜백 | 플러그인이 보낸 쪽 cwd 전달 | 다른 폴더에서 두 번째 실행 | 충족 |
| 읽기 실패 안내 | `queue_failure` → `error` 항목 → `notifyOpenFailure` | 조용한 폐기 금지 | 권한 없음(웜)·없음(콜드) 토스트, bridge 테스트 수정 전 FAIL/후 PASS | 충족 |
| 업데이트 재실행 시 재열기 방지 | `cmd_update_apply` 표식 → `setup()` 1회 확인 | updater `/ARGS`·NSIS 따옴표 제거(소스) | 표식 일치·쪼개진 인자·만료·버전 불일치 6경우 | 충족(실제 업데이트 경로 미실행) |
| 시작 경합 순서 | `StartupGate` | WebView2 생성 중 메시지 처리 | 단위 테스트 | 미검증(실행 경합 재현 없음) |
| macOS 회귀 없음 | LaunchServices `Opened` | #50 동작 | 콜드 스타트 한글 공백 파일명 | 충족 |

자동 검사: src-tauri fmt·clippy(경고 0)·test 6 PASS, studio tsc·npm test 1,835 PASS/0 FAIL/2 SKIP. 루트 엔진 크레이트 미변경.

## 조판 원칙·시각 증적

비해당 — 문서 엔진·조판·렌더링 변경 없음. Visual Sweep 비해당.

## 실제 CI

head `d298a41cb` 기준 30 성공·4 건너뜀·실패 0. Lint(fmt·clippy·WASM check)·Build & Test·archive 테스트 4샤드·Native Skia·
Canvas visual diff·Frontend package gates·CodeQL(rust·python·js/ts)·adapter inter-diff·prop roundtrip 이 성공했다.
건너뜀은 `Frontend unit gates`·`WASM Build`·`Workflow promotion preflight`·`cancel-stale-runs` 다. studio 단위 테스트는 CI 에서
건너뛰었으므로 로컬 `npm test`(1,835 PASS/0 FAIL/2 SKIP)와 desktop-bridge 수정 전 FAIL/후 PASS 를 근거로 한다.
데스크톱 크레이트(src-tauri)는 CI 대상이 아니어서 로컬 fmt·clippy·test 6 PASS 를 근거로 한다.

## 병합 후

- 남은 검증: Windows 실기기에서 파일 연결 첫 실행·NSIS 업데이트 재실행·Explorer 다중 선택. 다음 Desktop 릴리스 설치 후 확인하고 #121 을 닫는다.
- 본인 PR 이며 별도 댓글 지시가 없어 PR 댓글은 게시하지 않았다.
