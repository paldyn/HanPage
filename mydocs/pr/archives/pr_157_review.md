# PR #157 리뷰 — 문서마다 창 하나 (#156)

## 판정

**승인·병합 완료.** 검토 head `6edb59a0e810a88c3af375329a53be9f18579d66` 의 GitHub CI 33 성공·5 건너뜀, MERGEABLE(CLEAN),
Desktop Release dispatch(run 40639106674) macOS·Windows 번들 성공을 확인하고 2026-11-03 작업지시자 승인("응")으로 squash
병합했다(merge `97c8614cf36d59ebed79c944d73995d566196da4`). #156 은 devel push 워크플로가 닫았다. 2026-10-28 첫 병합 시도는
자동 모드 분류기가 명시 승인 없는 병합으로 막아 작업지시자에게 승인을 다시 받았다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#157](https://github.com/paldyn/HanPage/pull/157) / enxec / paldyn/HanPage devel |
| 고정 base | `2b23c21c75cf8a9f3bfb7d18bdc5fec06f7e59df` |
| 커밋 | `b608f5508` 1차 구현, `c83f4e369` 1차 검토 반영, `f6e4d19ce`·`f8900614a` 문서, `98bbfa766` 2차 검토 반영, `6edb59a0e` 문서 |
| 관련 이슈 | [#156](https://github.com/paldyn/HanPage/issues/156) |
| 경로 | maintainer 본인 작업. 작업지시자 요청(2026-10-28) "또다른 한글파일 열면 또다른 한페이지창으로", 이슈·push·PR 승인 "응응" |

## 구현 주장과 검증

[결과 보고서](../../report/task_m100_hp156_report.md)를 정본으로 한다. 요약:

| 주장 | 근거 | 판정 |
| --- | --- | --- |
| 빈 창만 재사용, 아니면 새 창, 열렸거나 열리는 중인 파일은 그 창으로 | 레지스트리 단위 21, macOS 실기(콜드 스타트 18회, 같은 파일·여는 중 같은 파일 `Focus`, 세 번째 창 위치) | 충족 |
| 웹뷰 상태 보고·넘기기(원시 본문)·실패 알림 뒤 재보고 | 브리지 단위 16, E2E `desktop-multi-window` 14, 실기 넘기기(임시 패치, 한글 이름) | 충족 |
| 닫기·Cmd+Q 저장 확인, 응답 없는 창 네이티브 확인 | 레지스트리 단위, 웹뷰 E2E(취소·저장 안 함) | 충족(실기 대화상자 미검증) |
| 업데이트 여러 창(사전 차단·알림 1회·적용 중 보호) | E2E `desktop-update-ui` 151 | 충족(실제 업데이트 경로 미검증) |
| Windows 시작 중 두 번째 실행, 창 생성 실패 처리 | 순수 함수 단위(`plan_relaunch`, `no_windows_left`) | 충족(Windows 실기 미검증) |
| 새 검사가 결함을 잡는다 | 변형 검사 1차 6건·2차 14건 모두 FAIL 검출 | 충족 |
| Windows 동작 | Desktop Release Windows 번들 성공(컴파일·번들) | 실기 미검증 |

독립 검토 2회(2차는 에이전트 58: 6관점 탐색, 발견마다 검증자 3명). 2차 확인 10건 중 9건 반영, Windows 혼합 배율 새 창
위치는 #159 로 분리했다.

## 조판 원칙·시각 증적

비해당 — 렌더링·조판 코드를 바꾸지 않았다.

## 실제 CI

head `6edb59a0e` 기준 33 성공·5 건너뜀. Desktop Release dispatch: 1차 반영 head `f8900614a` 의 run 40627099903 은 Windows 성공·
macOS DMG 생성 단계 실패(러너, 코드와 무관), 최종 head 의 run 40639106674 은 macOS(서명·공증 포함)·Windows 모두 성공.

## 병합 후

- 후속: #158(데스크톱 크레이트 PR 검사, [PR #160](pr_160_review.md) 병합), #159(Windows 혼합 배율 새 창 위치, PR #168).
- 남은 확인: 다음 Desktop 릴리스에서 macOS·Windows 실기의 닫기·Cmd+Q 확인창, Windows 다중 창, 실제 업데이트 경로.
- 본인 PR 이며 별도 댓글 지시가 없어 PR 댓글은 게시하지 않았다.
