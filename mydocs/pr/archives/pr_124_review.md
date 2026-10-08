# PR #124 리뷰 — 서명·공증 검증 보강과 Windows 미서명 결정 (#122)

## 판정

**승인·병합 완료.** 검토 head `9f80c2143b13d9b9f5ef16bfc1cb30432b1dd87f` 의 GitHub CI 28 성공·4 건너뜀, MERGEABLE/CLEAN,
병합 전 Desktop Release dispatch 시험 성공을 확인하고 2026-10-08 작업지시자 승인("응")으로 squash 병합했다
(merge `c629b7ed7ea47a317f74f9708ee7feba7fead377`). Windows 는 작업지시자 결정("1로하자")에 따라 미서명 배포를 유지한다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#124](https://github.com/paldyn/HanPage/pull/124) / enxec / paldyn/HanPage devel |
| 고정 base | `5685104a56a6502b57a8f210f5ea14e8d45b6e88` |
| 코드 head·문서 head | `39710e814bbb938df679517d6b6ae3ede51e32c3` / `947d1d4ea34dc9efb722ff175363491a997f1229` |
| 관련 이슈 | [#122](https://github.com/paldyn/HanPage/issues/122) (서명 개시 후 close) |
| 경로 | collaborator self-PR, 작업지시자 지시 "윈도우도 설치파일 정식으로 할수있게 진행하자", 승인 "둘다 응응"(2026-10-07) |

## 구현 주장과 검증

[결과 보고](../../report/task_m100_hp122_report.md)를 정본으로 한다.

| 주장 | 독립 근거 | 실제 검사 | 판정 |
| --- | --- | --- | --- |
| 개인은 Azure Artifact Signing 불가, PALDYN 게시자는 사업자등록 필요 | Microsoft Learn quickstart(2026-09-29), CA/B 기준 | 조사 + 반박 검증 | 충족 |
| macOS 검증 단계가 0.8.9 수동 감사와 같은 검사를 한다 | `macos-audit.py` | dispatch run 37724295849 | 충족 |
| dispatch 는 태그에서 실행해도 릴리스를 만들지 않는다 | GitHub `GITHUB_REF` 의미 | 식 검토(검토 지적 반영) | 충족 |
| updater 키 없으면 빌드 실패(주석 정정) | — | 로컬 재현 | 충족 |
| CLI 2.11.5 회귀 없음 | — | 로컬 macOS 번들, dispatch run 의 macOS·Windows 번들 | 충족 |
| 문서 정합 | — | 링크 오류 0, 메타데이터 base 동일 | 충족 |

## 조판 원칙·시각 증적

비해당 — 엔진·조판·렌더링 변경 없음.

## 실제 CI

PR head `9f80c2143` 기준 28 성공·4 건너뜀·실패 0.

병합 전 Desktop Release dispatch 시험 [run 37724295849](https://github.com/paldyn/HanPage/actions/runs/37724295849)
(ref `claude/desktop-windows-signing` @ `9f80c2143`, 릴리스 없음):

| job | 결과 |
| --- | --- |
| Bundle macOS (aarch64) | 성공. `Verify macOS signature and notarization` 실행 — `macos/HanPage.app` 과 DMG 안 `HanPage.app` 모두 codesign strict 통과, Gatekeeper `accepted` / `source=Notarized Developer ID`, `stapler validate` 성공, DMG checksum 정상 |
| Bundle Windows (x64) | 성공. CLI 2.11.5 NSIS 빌드, `Report Windows installer signature status` 실행 |

## 병합 후

- 다음 태그 릴리스부터 macOS 검증 단계가 게시 전 게이트로 동작하고, 릴리스 노트에 Windows 설치 안내를 넣는다.
- #122 는 미서명 유지 결정과 재개 조건을 남기고 닫는다.
- 본인 PR 이며 별도 댓글 지시가 없어 PR 댓글은 게시하지 않았다.
