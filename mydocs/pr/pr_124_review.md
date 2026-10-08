# PR #124 리뷰 — 서명·공증 검증 보강과 Windows 미서명 결정 (#122)

## 판정

**잠정 — 갱신된 head 의 GitHub CI 대기.** 2026-10-08 작업지시자 결정("1로하자")으로 Windows 는 미서명 배포를 유지하고
Azure 서명 연동을 제거했다. 로컬 검증에서 차단 결함은 없다. macOS 검증·Windows 보고 단계의 실제 실행은 다음 desktop-release 실행에서 확인한다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#124](https://github.com/paldyn/HanPage/pull/124) / enxec / paldyn/HanPage devel |
| 고정 base | `5685104a56a6502b57a8f210f5ea14e8d45b6e88` |
| 코드 head·문서 head | `39710e814bbb938df679517d6b6ae3ede51e32c3` / `947d1d4ea34dc9efb722ff175363491a997f1229` |
| 관련 이슈 | [#122](https://github.com/paldyn/HanPage/issues/122) (서명 개시 후 close) |
| 경로 | collaborator self-PR, 작업지시자 지시 "윈도우도 설치파일 정식으로 할수있게 진행하자", 승인 "둘다 응응"(2026-10-07) |

## 구현 주장과 검증

[결과 보고](../report/task_m100_hp122_report.md)를 정본으로 한다.

| 주장 | 독립 근거 | 실제 검사 | 판정 |
| --- | --- | --- | --- |
| 개인은 Azure Artifact Signing 불가, PALDYN 게시자는 사업자등록 필요 | Microsoft Learn quickstart(2026-09-29), CA/B 기준 | 조사 + 반박 검증 | 충족 |
| macOS 검증 단계가 0.8.9 수동 감사와 같은 검사를 한다 | `macos-audit.py` | 코드 대조 | 코드 검토 충족, 실행 미검증 |
| dispatch 는 태그에서 실행해도 릴리스를 만들지 않는다 | GitHub `GITHUB_REF` 의미 | 식 검토(검토 지적 반영) | 충족 |
| updater 키 없으면 빌드 실패(주석 정정) | — | 로컬 재현 | 충족 |
| CLI 2.11.5 회귀 없음 | — | macOS 번들 빌드 | macOS 충족, Windows 미검증 |
| 문서 정합 | — | 링크 오류 0, 메타데이터 base 동일 | 충족 |

## 조판 원칙·시각 증적

비해당 — 엔진·조판·렌더링 변경 없음.

## 실제 CI

PR 생성 직후 대기. 결과 확인 후 기록한다.
