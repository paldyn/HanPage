# Task #170 (M100) — Desktop 릴리스 검증 강화 최종 보고서

Issue: #170 · 계획: [task_m100_hp170.md](../plans/task_m100_hp170.md) · 워크플로 `.github/workflows/desktop-release.yml` · 스크립트 `HanPage-Desktop/scripts/release-audit.mjs`

## 1. 결과 요약

0.8.12 공개 전 독립 검증이 짚은 릴리스 검증 도구의 빈틈을 메웠다.

- `Verify macOS signature and notarization` 은 서명 시크릿이 아니라 고정한 배포 Team ID(`8L78W6D8XF`)와 비교하고, 실제로 올리는
  updater 아카이브를 풀어 같은 검사를 하며, 번들·아카이브·DMG 세 `.app` 의 CDHash 가 같아야 통과한다. 아카이브 항목과 DMG
  컨테이너 서명·볼륨 구성도 확인한다. 태그 빌드는 tauri-action 이 올린 릴리스가 아직 초안인지, 그 macOS 자산의 digest 가 검증한
  파일과 같은지도 본다. 실패하면 원인별 `::error` 를 남긴다.
- 태그 빌드는 번들 전에 그 태그의 공개 릴리스가 있는지 보고 있으면 멈춘다(공개 뒤 재실행이 공개 자산을 지우고 다시 올리는 것을 막음).
- macOS 14 러너 이미지가 2026-11-02 에 은퇴해, `desktop-release.yml`·`desktop-check.yml` 의 macOS job 을 `macos-15` 로 옮겼다.
  `macos-15` 빌드도 지원 최소 macOS 는 같고(LSMinimumSystemVersion 10.13, 실행 파일 minos 11.0) SDK 만 14.5 → 15.5 다.
- 러너에서 DMG 를 만드는 `bundle_dmg.sh` 가 가끔 실패해(0.8.12 태그 빌드 첫 시도, 이 PR 의 첫 수동 실행), tauri-action 단계가 실패하면
  같은 설정으로 한 번 더 시도하고 macOS 번들 로그를 `--verbose` 로 남긴다. 원인은 tauri-action 이 CI 에서도 켜 두는 Finder AppleScript
  (DMG 창 배치) 단계로 보인다.
- 릴리스마다 0.8.11 증적 폴더에서 가져다 쓰던 감사 스크립트 두 개를 `HanPage-Desktop/scripts/release-audit.mjs` 하나로 합쳤다.
  VS_VERSIONINFO 파싱, 공개키 출처·이전 릴리스와의 연속성, updater 와 같은 엄격한 서명 디코딩, `latest.json` 형태, 여러 위치 변조,
  DMG 앱·updater 앱의 동일 빌드, DMG 컨테이너·볼륨, 아카이브 경로, GitHub 릴리스 digest 대조(`--release`)를 더했다.
- 운영 가이드의 릴리스 절차에 0.8.10~0.8.12 에서 실제로 밟은 단계(태그 전 시험, 병합 tree 확인, 실패 job 재실행, 감사와 대조군,
  공개 직전 재감사, 공개 뒤 updater 매니페스트 확인)를 넣었다.

## 2. GitHub 운영 변경 기록

- 등급: O4(runner·Release 게시 게이트) + O3(job 명령)
- 근거: #170, 작업지시자 지시(2026-11-03) "필요한거면해"
- 기준 시각 / SHA: 2026-11-03, `origin/devel` `144377b688`(PR base)
- 대상 workflow·설정: `.github/workflows/desktop-release.yml` 의 macOS 러너·tauri-action 단계(`id`·`continue-on-error`·재시도 단계)·
  공개 릴리스 확인 단계·macOS 검증 단계, `.github/workflows/desktop-check.yml` 의 macOS 러너. 저장소 설정·branch protection·secret 은
  바꾸지 않는다
- 적용 전(base `144377b688`): macOS job `macos-14`, 검증 단계는 번들 `.app`·DMG 안 `.app` 2개를 시크릿에서 읽은 Team ID 로 확인, 번들
  단계 한 번(실패하면 job 실패), 공개 릴리스 확인 없음, `Desktop check (macOS)` `macos-14`
- 적용 후(이 PR): 1절과 같다
- 영향 event·branch·required check: `hanpage-desktop-v*` 태그 push, workflow_dispatch, 데스크톱 크레이트를 바꾼 PR(`Desktop check`).
  required check 없음
- 변경 파일: `.github/workflows/desktop-release.yml`, `.github/workflows/desktop-check.yml`, `HanPage-Desktop/scripts/release-audit.mjs`
  (새 파일), `mydocs/manual/desktop_auto_update.md`, `HanPage-Desktop/README.md`, `mydocs/README.md`, 계획서·이 보고서
- 로컬 검증: 3절
- 적용 승인: 작업지시자 승인 "응"(2026-11-03) — 이슈 생성, 브랜치 푸시·PR 생성, PR 브랜치의 workflow_dispatch. 병합은 별도 승인
- 적용 결과와 run URL: 4절
- 관찰 창과 완료 조건: 이 PR 의 workflow_dispatch 실행(`macos-15`)에서 새 검사가 실행·통과한다(확인). 태그 경로(공개 릴리스 확인,
  초안 여부·digest 대조)와 재시도 경로는 다음 태그 빌드(0.8.13)가 첫 실제 실행이다
- rollback 조건과 명령: 새 검사가 정상 빌드를 막거나 오탐이 확인되면, 병합 commit 을 `git revert` 한 뒤 두 workflow 의
  `runner: macos-15` 만 다시 적용하는 PR 을 연다. 러너는 되돌리지 않는다(macOS 14 이미지 은퇴로 다시 실패한다)

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

공개 릴리스 확인 단계(같은 방식): 공개된 0.8.12 → `공개 릴리스` 로 멈춤, 초안(v0.8.6, `releases/tags` 404) → 통과, API 계속 실패 →
세 번 시도 뒤 `공개 릴리스 확인` 으로 멈춤. 재시도 단계는 tauri-action 단계와 `uses`·`env`·`with` 가 같음을 YAML 로 대조했다.

감사 스크립트:

| 검사 | 결과 |
|---|---|
| 공개된 0.8.12·0.8.11 자산(`--release` 432269742·408924819) | 각 61/61 통과. 키 `835d6b3831e133aa`(minisign 표기 `AA33E131386B5D83`), 이전 릴리스(0.8.11·0.8.10)와 같은 공개키 |
| 음성 26가지 | 해당 검사만 실패 — 설치 파일 1바이트 변조, `latest.json` 버전·서명 끝 줄바꿈·`pub_date` 숫자·`pub_date` 범위 밖·플랫폼 여분 키·`name` 키, `.sig` 접두어·비정규 base64·끝 `\r`·잘못된 UTF-8·맞바꿈, 다른 공개키(이전·현재), 0.8.11 DMG·EXE, 서명 없는 DMG·볼륨 여분 항목·앱 링크, 여분 파일·하위 디렉터리·`latest.json` 누락, 경로 이탈·읽기 전용 아카이브, 릴리스 자산 변조·다른 릴리스 |
| 양성 대조 2가지 | 원본 DMG 가 이미 마운트된 상태(복제본 마운트), `--release` 정상 — 통과 |
| 인자 7가지 | `--release=ID` 는 대조 수행(61개), 값 누락·오타·여분 인자·빈 값·숫자 아닌 ID·버전 누락은 사용법 오류(종료 2). `--release` 없이 실행하면 결과에 `release: null` |
| Ctrl-C(프로세스 그룹 SIGINT, 6초·25초) | JSON 을 남기고 마운트·임시 폴더 없음. 6초는 `hdiutil attach` 가 스스로 취소해 그 검사가 실패(종료 1), 25초는 끊긴 검사 없이 통과. 신호로 죽은 자식이 있으면 남은 검사를 건너뛰고 종료 130 |

| 기타 | 결과 |
|---|---|
| YAML 파싱(ruby), 검증·공개 릴리스 확인 단계 `bash -n` | 통과 |
| 저장소 워크플로 정책 테스트 29개(`scripts/tests`) | 모두 통과 |
| 문서 링크(`scripts/check_markdown_links.py`) | 이상 없음 |
| 문서 메타데이터(`scripts/check_document_metadata.py`) | 오류 20건 — 모두 `mydocs/tech/` 의 기존 파일이며 변경 전과 같은 수 |
| §8 과 워크플로 `::error` 제목 대조 | 14개 모두 §8 에 있음 |

## 4. PR 에서의 실제 실행

| 실행 | head | 결과 |
|---|---|---|
| desktop-release 수동 실행 [41600452028](https://github.com/paldyn/HanPage/actions/runs/41600452028) | `55bb75695e` | Windows 성공. macOS(`macos-15-arm64` 20261028.0489.1)는 공증 `Accepted` 뒤 `bundle_dmg.sh` 가 6초 만에 실패해 검증 단계에 닿지 못함 |
| desktop-release 수동 실행 [41605141433](https://github.com/paldyn/HanPage/actions/runs/41605141433) | `a159d19950`(재시도·`--verbose` 추가) | 양 플랫폼 성공. tauri-action 첫 시도 통과(재시도 생략), `bundle_dmg.sh` 28초. 검증 단계(`macos-15`) 통과 — 번들·아카이브·DMG `.app` 모두 `valid on disk`·`Notarized Developer ID`, `The validate action worked!` 3회, DMG 컨테이너 서명·체크섬 VALID. annotation 은 Node 20·러너 용량 안내뿐(`macOS 미서명` 없음). 산출 DMG: LSMinimumSystemVersion 10.13, minos 11.0, SDK 15.5, Team `8L78W6D8XF`, 볼륨 `HanPage.app`·`Applications` |
| PR CI(`a159d19950`) | | 32 통과·실패 0(나머지 진행 중). `Desktop check (macOS)` 가 `macos-15` 에서 통과 |

verbose 로그로 tauri-action 이 `TAURI_BUNDLER_DMG_IGNORE_CI=true` 를 기본으로 둬 CI 에서도 Finder AppleScript(창 배치)를 실행함을
확인했다(`--skip-jenkins` 없음). 같은 source 가 실패했다가 통과해 일시 실패로 보고, 창 배치를 유지한 채 재시도를 둔다.

## 5. 독립 검토

읽기 전용 반박 검토를 세 차례 했다(영역마다 검토자 1명, 중간 이상 발견마다 검증자 2명).

- 1차(에이전트 11): 확인 4건 — 감사 스크립트가 DMG 컨테이너 서명·볼륨 내용을 보지 않아 서명 없는 재포장 DMG 가 통과(high),
  서명 문자열을 updater 보다 관대하게 디코딩(medium), 문서의 `gh` 명령에 `--repo` 누락(medium), 태그 전 시험 순서가 실제 절차보다
  약함(medium). low 7건(arch 이름, CI 의 아카이브 구조 검사 누락, API 재시도·annotation, 출력 잘림, 키 ID 표기, 문서 앞뒤 불일치 등).
  모두 반영했다.
- 2차(에이전트 9): 1차 수정이 의도대로 동작하고 우회 경로가 없음을 확인. 확인 3건 — `macos-14` 러너 은퇴(변경 전부터 있던 문제,
  medium), 문서에 공개본과 감사본 대조·대조군·공개 승인이 빠짐(medium), 원본 DMG 가 이미 마운트돼 있으면 정상 릴리스가 이유 없이
  실패(medium). low 7건(공개 뒤 재실행 감지, `fail` 밖 실패, UTF-8·마지막 `\r`, `latest.json` 키·`pub_date`, 요약의 CLI 확인 방법,
  재시도 표현, 계획서의 옛 설계). 모두 반영했다.
- 3차(에이전트 4): 2차 수정 확인. 확인 1건 — `--release` 를 `=` 형식·값 누락·오타로 주면 조용히 무시돼 릴리스 대조 없이 통과
  (medium). low 9건(macos-15 검증 미실행, 공개 뒤 재실행이 macOS job 에서만·교체 뒤에 드러남, `pub_date` 범위·플랫폼 항목, 신호 처리기
  미동작, annotation 제목 확인 방법, 음성 대조 수, 계획서 rollback, 운영 등급). 모두 반영했다(공개 릴리스 확인 단계 추가 포함).

## 6. 남은 확인

- 태그 경로(공개 릴리스 확인, 초안 여부·digest 대조)는 workflow_dispatch 에서 돌지 않아 로컬 재현으로만 확인했다. 재시도 단계도 이번
  실행에서는 첫 시도가 통과해 실행되지 않았다(설정이 같음은 YAML 로 확인). 다음 태그 빌드(0.8.13)에서 실제 결과를 본다.
- DMG 컨테이너 공증·스테이플은 비범위(계획서 2절).
