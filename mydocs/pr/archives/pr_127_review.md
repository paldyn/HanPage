# PR #127 리뷰 — Desktop 업데이트 준비 완료 작은 알림 (#126)

## 판정

**승인·병합 완료.** 검토 head `5c5dc79bcd767b2bf2d434491a2cb5ae39350220` 의 GitHub CI 30 성공·4 건너뜀, MERGEABLE/CLEAN 를 확인하고
2026-10-10 작업지시자 승인("응")으로 squash 병합했다(merge `6dbff3c92cef39c000d5f5c40404012fbc7a81d7`). #126 은 병합으로 닫혔다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#127](https://github.com/paldyn/HanPage/pull/127) / enxec / paldyn/HanPage devel |
| 고정 base | `93b0a4a4880fe5ff3e7195cb6e0fb08ccb0e0161` |
| 코드 커밋 | `770f825610f7dcc4df165926afe72f510a962795`(구현) → `2f66ea7e17ba6f52a2cf908e080fa1a0cdf41f89`(2차 검토 반영) → `5c5dc79bcd767b2bf2d434491a2cb5ae39350220`(재개 경로 보완·보고서) |
| 관련 이슈 | [#126](https://github.com/paldyn/HanPage/issues/126) |
| 경로 | collaborator self-PR, 작업지시자 선택 "작은 알림 1회 (추천)"(2026-10-08), 진행 승인 "응"(2026-10-10) |

## 구현 주장과 검증

[결과 보고](../../report/task_m100_hp126_report.md)의 표를 정본으로 한다. 요약:

| 주장 | 적용 경로 | 독립 기대값 | 실제 검사 | 판정 |
| --- | --- | --- | --- | --- |
| 준비 완료 시 버전당 1회, 포커스 비이동 | `receiveStatus` → `maybeNudge` | 0.8.9 편집 흐름 보호 규칙(카드 자동 표시 금지) 유지 | E2E: 표시·위치·`activeElement` 불변, 마우스 클릭 중 `focusin` 0회, 같은 버전 재수신 미표시 | 충족 |
| 8초 자동 닫힘, hover·내부 포커스 유지, Esc | `armNudgeTimer`·`handleNudgeKeys` | 작업지시자 선택안 | E2E: 멈춘 커서 아래 약 8초 닫힘, 실제 mousemove 유지, Esc 후 포커스 복원, 찾기 창이 열린 채 알림만 닫힘 | 충족 |
| 모달·창 비주시 중 미루기 | `modalOpen`·`windowAttended` → `resumeDeferredNudge` | 버전당 1회 알림을 아무도 못 본 채 소진하지 않는다 | E2E: 모달 중 미룸·닫힌 뒤 표시, 다른 탭 전면(`hasFocus` false·hidden)에서 미룸·복귀 시 표시, 창 이탈 중 정지·복귀 후 약 8초, 복귀 때 모달이면 그 모달 닫힌 뒤 표시 | 충족(WebKit·WebView2 창 이벤트 미검증) |
| 사용자가 연 팝업·모달 아래 | CSS z-index 10 | 사용자가 연 화면을 가리지 않는다 | E2E: 실제 표 메뉴·`#scroll-content` 양식 목록·body 비교 창을 알림 자리로 옮긴 `elementFromPoint` | 충족 |
| 모달 안에서 들어온 포커스 복귀 | `hideNudge` → `topModal` | 원래 위치로 복귀, 모달 뒤 편집기 금지 | E2E: 글자 모양 닫기 버튼 복귀·다음 Esc 로 대화상자 닫힘, 모달 뒤 자동 닫힘 시 편집기 비포커스 | 충족 |
| [업데이트]·[나중에] | 카드·`primaryAction` | 기존 저장 확인 흐름 | E2E: 카드+저장 확인, 취소 시 미적용 | 충족 |

자동 검사(코드 head `5c5dc79bc`): rhwp-studio tsc 통과, `npm test` 1,835 PASS/0 FAIL/2 SKIP, desktop-update-ui E2E 140 PASS/0 FAIL·브라우저 예외 0.
수정 전 검출: 변경 전 코드에서 새 알림 검사 FAIL, 1차 반영 전 3 FAIL, 2차 반영 전(`770f82561`) 8 FAIL, 재개 보완 전(`2f66ea7e1`) 1 FAIL.
데스크톱·엔진 Rust 코드 미변경.

## 독립 검토

1차 검토 결함 5건과, 1차 반영 코드에 대한 2차 검토(포커스·키보드 / 상태 전이·타이머 / 배치·접근성·테스트 타당성, 주장별 반박 검증)의
확인 결함 3건을 반영했다. 반영 뒤 자체 점검에서 찾은 재개 경로 빈틈 1건도 고쳤다. 반박된 주장과 근거는 결과 보고 3.3 에 있다.

## 조판 원칙·시각 증적

비해당 — 엔진·조판·렌더링 변경 없음. Visual Sweep 비해당. UI 표시는 PR 본문의 밝은·어두운 테마 캡처(코드 head 의 E2E 캡처와 바이트 동일)로 확인한다.

## 실제 CI

head `5c5dc79bc` 기준 30 성공·4 건너뜀·실패 0. Lint(fmt·clippy·WASM check)·archive 테스트 4샤드·Native Skia·Canvas visual diff·
Frontend package gates·CodeQL(rust·python·js/ts)·adapter inter-diff·prop roundtrip 이 성공했다. 건너뜀은 `Frontend unit gates`·
`WASM Build`·`Workflow promotion preflight`·`cancel-stale-runs` 다.
studio 단위 테스트·E2E 는 CI 에서 실행하지 않으므로 로컬 결과를 근거로 한다.

## 병합 후

- 이 알림은 앱 안의 기능이라 이 변경을 포함한 Desktop 0.8.11 을 설치한 다음 업데이트부터 보인다.
- 남은 검증: 실제 Tauri 업데이트 경로, macOS WebKit·Windows WebView2 의 창 전환·최소화 때 동작.
- 본인 PR 이며 별도 댓글 지시가 없어 PR 댓글은 게시하지 않았다.
