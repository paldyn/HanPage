# PR #124 리뷰 — Windows Authenticode 서명 연동과 서명·공증 검증 (#122)

## 판정

**잠정 — GitHub CI 대기.** 로컬 검증에서 차단 결함을 발견하지 않았다. Windows 서명 실행은 Artifact Signing 계정이 없어 미검증이며, 병합해도 설정 전에는 기존처럼 미서명으로 빌드된다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#124](https://github.com/paldyn/HanPage/pull/124) / enxec / paldyn/HanPage devel |
| 고정 base | `5685104a56a6502b57a8f210f5ea14e8d45b6e88` |
| 코드 head·문서 head | `39710e814bbb938df679517d6b6ae3ede51e32c3` / `947d1d4ea34dc9efb722ff175363491a997f1229` |
| 관련 이슈 | [#122](https://github.com/paldyn/HanPage/issues/122) (서명 개시 후 close) |
| 경로 | collaborator self-PR, 작업지시자 지시 "윈도우도 설치파일 정식으로 할수있게 진행하자", 승인 "둘다 응응"(2026-10-07) |

## 구현 주장과 검증

[결과 보고](../report/task_m100_hp122_report.md)의 표를 정본으로 한다.

| 주장 | 독립 근거 | 실제 검사 | 판정 |
| --- | --- | --- | --- |
| 서명이 updater `.sig` 보다 먼저 끝난다 | tauri-cli 2.11 `bundle_project` → `sign_updaters` 순서(소스) | 검증 단계가 `.sig` 시각 ≥ setup.exe 시각 확인 | 소스 근거 충족, 실행 미검증 |
| `signCommand` 객체 조각을 설정으로 받는다 | tauri 설정 스키마 | CLI 2.11.5 macOS 빌드에 샘플 조각 주입 | 충족 |
| 설정 없음/일부/전부 동작과 릴리스 조건 | workflow 식 | YAML 파싱, 검토 반박 검증 | 코드 검토 충족, 실행 미검증 |
| 클라이언트 무결성 | NuGet 카탈로그 `packageHash` | SHA-512 값 대조 | 충족 |
| updater 키 없으면 빌드 실패(주석 정정) | — | 로컬 재현 | 충족 |
| macOS 검증 단계 | 0.8.9 수동 감사 스크립트 | 다음 릴리스에서 첫 실행 | 미검증 |
| CLI 2.11.5 macOS 번들 회귀 없음 | — | 로컬 `--bundles app` 빌드 | 충족 |

문서: 링크 검사 오류 0, 메타데이터 검사 base 와 동일.

## 조판 원칙·시각 증적

비해당 — 엔진·조판·렌더링 변경 없음.

## 실제 CI

PR 생성 직후 대기. 결과 확인 후 기록한다.
