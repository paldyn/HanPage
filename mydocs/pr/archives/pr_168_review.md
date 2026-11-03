# PR #168 리뷰 — Windows 혼합 배율 모니터의 새 창 위치 (#159)

## 판정

**승인·병합 완료.** 검토 head `5b561f5b1b5ef2f94136cf6656ec62d11fd62316` 의 GitHub CI 32 성공·4 건너뜀(새 `Desktop check` macOS·Windows 포함), MERGEABLE 을 확인하고 2026-11-03
작업지시자 승인("응" — CI 가 모두 통과하면 병합)으로 squash 병합했다(merge `9fd1cba24072f6b2ea191b2a112f90d8e29e47b7`). #159 는 devel push 워크플로가 닫는다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#168](https://github.com/paldyn/HanPage/pull/168) / enxec / paldyn/HanPage devel |
| 고정 base | `ac205a6aac17587c67f4683c7f0f2dbec8f45306` |
| 커밋 | `081e6952a` 구현·수행계획서, `5b561f5b1` 결과 보고서 |
| 관련 이슈 | [#159](https://github.com/paldyn/HanPage/issues/159) |
| 경로 | maintainer 본인 작업. #156 2차 검토에서 분리한 후속 → 작업지시자 "제안된 작업 2개도 처리하자"(2026-10-28) |

## 구현 주장과 검증

[결과 보고서](../../report/task_m100_hp159_report.md)를 정본으로 한다.

| 주장 | 근거 | 판정 |
| --- | --- | --- |
| 위치를 플랫폼 전역 좌표계로 계산(macOS point, Windows 물리) | `plan_cascade` 단위 테스트(혼합 배율·넘침 되돌림·허용 오차·macOS 계산 유지) | 충족 |
| Windows 는 새 창을 처음부터 그 자리의 모니터에서 만든다(겹치면 그 모니터 안에서 만든 뒤 옮김) | `windows_placement`·`tao_creation_display` 단위 테스트(겹치는 배치는 `Moved`, 200% 노트북 왼쪽 100% 모니터 등은 `Direct`, 나열 순서 두 가지) | 충족 |
| 새 검사가 결함을 잡는다 | 변형 검사 7건 모두 FAIL 검출 | 충족 |
| Windows 에서 컴파일·Clippy·테스트 | PR 의 `Desktop check`(run 41551781680): Windows 36 통과(유닉스 전용 테스트 제외)·macOS 37 통과 | 충족 |
| Windows 혼합 배율 실기 배치 | 이 환경은 macOS | 미검증 |

독립 검토(3관점, 발견마다 검증자 3명)에서 첫 구현(위치 없이 숨겨 만든 뒤 옮기기)의 회귀(주 모니터 배율로 크기를 정해 두 배 크기
창)와 Windows 10 크기 미조정을 확인해 설계를 바꿨다.

## 조판 원칙·시각 증적

비해당 — 데스크톱 창 위치만 바꿨다.

## 병합 후

- 남은 확인: Windows 혼합 배율 실기(보조 모니터의 앞 창, 최대화된 앞 창, 넘쳐 되돌아가는 경우, 주 모니터 왼쪽·위의 낮은 배율 모니터).
- 본인 PR 이며 별도 댓글 지시가 없어 PR 댓글은 게시하지 않았다.
