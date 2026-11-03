# Task #170 (M100) — Desktop 릴리스 검증 강화 최종 보고서

Issue: #170 · 계획: [task_m100_hp170.md](../plans/task_m100_hp170.md) · 워크플로 `.github/workflows/desktop-release.yml` · 스크립트 `HanPage-Desktop/scripts/release-audit.mjs`

## 1. 결과 요약

0.8.12 공개 전 독립 검증이 짚은 릴리스 검증 도구의 빈틈을 메웠다.

- `Verify macOS signature and notarization` 은 서명 시크릿이 아니라 고정한 배포 Team ID(`8L78W6D8XF`)와 비교하고, 실제로 올리는
  updater 아카이브를 풀어 같은 검사를 하며, 번들·아카이브·DMG 세 `.app` 의 CDHash 가 같아야 통과한다. 아카이브 항목과 DMG
  컨테이너 서명·볼륨 구성도 확인한다. 태그 빌드는 tauri-action 이 올린 릴리스가 아직 초안인지, 그 macOS 자산의 digest 가 검증한
  파일과 같은지도 본다. 실패하면 원인별 `::error` 를 남긴다.
- macOS 14 러너 이미지가 2026-11-02 에 은퇴해, `desktop-release.yml`·`desktop-check.yml` 의 macOS job 을 `macos-15` 로 옮겼다.
- 릴리스마다 0.8.11 증적 폴더에서 가져다 쓰던 감사 스크립트 두 개를 `HanPage-Desktop/scripts/release-audit.mjs` 하나로 합쳤다.
  VS_VERSIONINFO 파싱, 공개키 출처·이전 릴리스와의 연속성, updater 와 같은 엄격한 서명 디코딩, 여러 위치 변조, DMG 앱·updater 앱의
  동일 빌드, DMG 컨테이너·볼륨, 아카이브 경로, GitHub 릴리스 digest 대조(`--release`)를 더했다.
- 운영 가이드의 릴리스 절차에 0.8.10~0.8.12 에서 실제로 밟은 단계(태그 전 시험, 병합 tree 확인, 실패 job 재실행, 감사와 대조군,
  공개 직전 재감사, 공개 뒤 updater 매니페스트 확인)를 넣었다.

## 2. GitHub 운영 변경 기록

- 등급: O3(job 명령·matrix 러너)
- 근거: #170, 작업지시자 지시(2026-11-03) "필요한거면해"
- 기준 시각 / SHA: 2026-11-03, `origin/devel` `144377b688`(PR base)
- 대상 workflow·설정: `.github/workflows/desktop-release.yml` 의 macOS 검증 단계·tauri-action 단계 `id`·macOS 러너,
  `.github/workflows/desktop-check.yml` 의 macOS 러너. 저장소 설정·branch protection·secret 은 바꾸지 않는다
- 현재 상태: 계획서 1절
- 기대 상태: 태그·수동 실행의 macOS job 이 `macos-15` 에서 고정 Team ID·세 `.app`·CDHash·아카이브·DMG 컨테이너를 검사하고, 태그
  빌드는 초안 여부와 자산 digest 까지 대조한다. `Desktop check (macOS)` 도 `macos-15` 에서 돈다
- 영향 event·branch·required check: `hanpage-desktop-v*` 태그 push, workflow_dispatch, 데스크톱 크레이트를 바꾼 PR(`Desktop check`).
  required check 없음
- 변경 파일: `.github/workflows/desktop-release.yml`, `.github/workflows/desktop-check.yml`, `HanPage-Desktop/scripts/release-audit.mjs`
  (새 파일), `mydocs/manual/desktop_auto_update.md`, `HanPage-Desktop/README.md`, `mydocs/README.md`, 계획서·이 보고서
- 로컬 검증: 3절
- 적용 승인: 작업지시자 승인 "응"(2026-11-03) — 이슈 생성, 브랜치 푸시·PR 생성, PR 브랜치의 workflow_dispatch
- 적용 결과와 run URL: 4절
- 관찰 창과 완료 조건: 이 PR 의 workflow_dispatch 실행과 다음 태그 빌드(0.8.13)에서 새 검사가 실행·통과한다. 태그 경로의 초안 digest 대조는
  다음 태그 빌드가 첫 실제 실행이다
- rollback 조건과 명령: 새 검사가 정상 빌드를 막거나 오탐이 확인되면 검증 단계 변경을 revert 하는 PR. 러너는 되돌리지 않는다
  (macOS 14 이미지 은퇴로 다시 실패한다)

## 3. 로컬 검증

검증 단계는 YAML 에서 `run` 블록을 그대로 꺼내, 공개된 0.8.12 자산을 러너와 같은 `bundle/` 경로에 놓고 macOS 에서 실행했다
(`bash --noprofile --norc -eo pipefail`, 러너와 같은 방식). 초안 응답이 필요한 경우는 실제 릴리스 응답을 초안으로 바꿔 돌려주는 `gh`
대역을, DMG 구성 검사까지 가야 하는 경우는 `.dmg` 경로에만 정상 DMG 의 서명 정보를 돌려주는 `codesign` 대역을 썼다.

| 경우 | 결과 |
|---|---|
| 정상(태그·초안) | 통과 — 세 `.app` CDHash `260caa81…`, 초안의 아카이브·DMG digest 일치 |
| digest 없음 | 통과 — 자산을 받아 계산한 SHA-256 일치 |
| 수동 실행 | 통과 — 업로드 대조 생략 |
| 미서명 수동 실행 / 미서명 태그 | 경고 후 통과 / `macOS 미서명` 실패 |
| 다른 Team ID | `macOS Team ID` |
| 0.8.11 아카이브(다른 빌드) | `macOS 빌드 불일치` |
| 같은 앱을 다시 압축한 아카이브 | `초안 자산 불일치`(업로드 파일과 다름) |
| 아카이브에 `HanPage.app` 밖 항목 | `updater 아카이브 구조` |
| 서명 없는 DMG / 볼륨 여분 항목 / 볼륨의 앱이 링크 / DMG 없음 | `DMG 서명` / `DMG 구성` / `macOS 서명`(디렉터리 아님) / `DMG` |
| 다른 릴리스의 자산 | `초안 자산 불일치` |
| 공개된 릴리스(실제 API) | `초안 자산`(초안이 아님) |
| 자산 없는 초안 / 릴리스 ID 없음 / 다른 태그의 릴리스 | `초안 자산` |
| API 계속 실패 | 세 번 시도 뒤 `초안 자산 조회` |

감사 스크립트:

| 검사 | 결과 |
|---|---|
| 공개된 0.8.12·0.8.11 자산(`--release` 432269742·408924819) | 각 57/57 통과. 키 `835d6b3831e133aa`(minisign 표기 `AA33E131386B5D83`), 이전 릴리스(0.8.11·0.8.10)와 같은 공개키 |
| 음성 26가지 | 해당 검사만 실패 — 설치 파일 1바이트 변조, `latest.json` 버전·서명 끝 줄바꿈·`pub_date` 숫자·`name` 키, `.sig` 접두어·비정규 base64·끝 `\r`·잘못된 UTF-8·맞바꿈, 다른 공개키(이전·현재), 0.8.11 DMG·EXE, 서명 없는 DMG·볼륨 여분 항목·앱 링크, 여분 파일·하위 디렉터리·`latest.json` 누락, 경로 이탈·읽기 전용 아카이브, 릴리스 자산 변조·다른 릴리스 |
| 원본 DMG 가 이미 마운트된 상태 | 통과(복제본 마운트) |
| Ctrl-C(프로세스 그룹 SIGINT, 6초·20초) | JSON 을 남기고 마운트·임시 폴더 없음. 끊긴 검사는 실패로 남는다 |

| 기타 | 결과 |
|---|---|
| YAML 파싱(ruby), 검증 단계 `bash -n` | 통과 |
| 저장소 워크플로 정책 테스트 29개(`scripts/tests`) | 모두 통과 |
| 문서 링크(`scripts/check_markdown_links.py`) | 이상 없음 |
| 문서 메타데이터(`scripts/check_document_metadata.py`) | 오류 20건 — 모두 `mydocs/tech/` 의 기존 파일이며 변경 전과 같은 수 |
| §8 과 워크플로 `::error` 제목 대조 | 12개 모두 §8 에 있음 |

## 4. PR 에서의 실제 실행

(진행 후 기록)

## 5. 독립 검토

읽기 전용 반박 검토를 두 차례 했다(영역마다 검토자 1명, 중간 이상 발견마다 검증자 2명).

- 1차(에이전트 11): 확인 4건 — 감사 스크립트가 DMG 컨테이너 서명·볼륨 내용을 보지 않아 서명 없는 재포장 DMG 가 통과(high),
  서명 문자열을 updater 보다 관대하게 디코딩(medium), 문서의 `gh` 명령에 `--repo` 누락(medium), 태그 전 시험 순서가 실제 절차보다
  약함(medium). low 7건(arch 이름, CI 의 아카이브 구조 검사 누락, API 재시도·annotation, 출력 잘림, 키 ID 표기, 문서 앞뒤 불일치 등).
  모두 반영했다.
- 2차(에이전트 9): 1차 수정이 의도대로 동작하고 우회 경로가 없음을 확인. 확인 3건 — `macos-14` 러너 은퇴(변경 전부터 있던 문제,
  medium), 문서에 공개본과 감사본 대조·대조군·공개 승인이 빠짐(medium), 원본 DMG 가 이미 마운트돼 있으면 정상 릴리스가 이유 없이
  실패(medium). low 7건(공개 뒤 재실행 감지, `fail` 밖 실패, UTF-8·마지막 `\r`, `latest.json` 키·`pub_date`, 요약의 CLI 확인 방법,
  재시도 표현, 계획서의 옛 설계). 모두 반영했다.

## 6. 남은 확인

- 태그 빌드의 초안 digest 대조는 dispatch 에서 돌지 않아 로컬 재현(공개된 0.8.12 릴리스 대상)으로만 확인했다. 다음 태그 빌드에서 실제 결과를 본다.
- DMG 컨테이너 공증·스테이플은 비범위(계획서 2절).
