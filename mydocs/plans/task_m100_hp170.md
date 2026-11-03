# Task #170 (M100) — Desktop 릴리스 검증 강화 수행계획서

- **이슈**: [paldyn/HanPage#170](https://github.com/paldyn/HanPage/issues/170)
- **브랜치**: `claude/desktop-release-hardening` (`origin/devel` `144377b688` 기준)
- **작업지시자 지시(2026-11-03)**: 0.8.12 공개 승인과 함께, 공개 전 독립 검증에서 나온 후속 제안("Harden HanPage Desktop release
  verification")에 "필요한거면해"
- **등급**: [GitHub 운영 매뉴얼](../manual/github_operations.md) O3(job 명령·matrix 러너). 저장소 설정·secret·branch protection 은 바꾸지 않는다

## 1. 현재 상태 (2026-11-03)

| 항목 | 값 |
|---|---|
| `desktop-release.yml` 검증 단계 | `bundle/macos/HanPage.app` 과 DMG 안의 `.app` 에 codesign strict·Team ID·Developer ID·Gatekeeper 공증·stapler, DMG `hdiutil verify` |
| Team ID 비교 기준 | 서명에 쓰는 `APPLE_SIGNING_IDENTITY` 에서 읽은 값 — 다른 인증서가 시크릿에 섞여도 통과한다 |
| updater 아카이브 | 실제로 올리는 `HanPage.app.tar.gz` 안의 `.app` 은 확인하지 않는다 |
| 업로드 순서 | tauri-action 이 초안에 자산을 올린 뒤 검증 단계가 돈다. 올라간 자산이 검증한 파일과 같은지 대조하지 않는다 |
| macOS 러너 | `desktop-release.yml`·`desktop-check.yml` 이 `macos-14` 를 쓴다. GitHub 는 macOS 14 이미지를 2026-11-02 에 은퇴시키고 그 뒤 실행은 오류로 끝난다고 공지했다(actions/runner-images#13518). 0.8.12 태그 빌드(11-03)는 아직 배정됐지만 다음 릴리스는 보장되지 않는다 |
| 초안 감사 | 0.8.11 증적 폴더의 `audit-draft.mjs`·`macos-local.sh` 를 릴리스마다 재사용. setup.exe 버전은 UTF-16 문자열 포함 여부, 공개키 출처·이전 릴리스와의 연속성 미기록, 변조 시험은 마지막 1바이트, DMG 앱과 updater 앱의 동일 빌드 미확인 |
| 운영 가이드 §4 | 태그 전 dispatch 시험·감사 스크립트·실패 job 재실행·초안 API 수정 때의 `tag_name`·공개본과 감사본 대조·updater 전환 확인이 실제 절차(0.8.10~0.8.12 기록)와 달리 빠져 있다 |

근거: [0.8.12 배포 검증](../working/desktop_release_v0812_20261103.md)의 "독립 검증"·"게이트 5" 행. 0.8.12 자체는 로컬 대조
(CDHash·CRC·VS_VERSIONINFO·공개키 연속성)로 확인했다.

## 2. 설계

**검증 단계**(`desktop-release.yml`)

- `EXPECTED_TEAM_ID: 8L78W6D8XF` 를 두고, 서명 ID 에서 읽은 Team ID 와 각 `.app`·DMG 의 `TeamIdentifier` 를 이 값과 비교한다.
- 번들 `.app`, `HanPage.app.tar.gz` 를 `$RUNNER_TEMP` 에 푼 `.app`, DMG 안의 `.app` 에 같은 검사(디렉터리 여부 포함)를 하고
  세 CDHash 가 같아야 한다. 아카이브는 풀기 전에 `HanPage.app` 밖 항목·상위 경로가 없는지 본다(updater 가 첫 경로만 떼고
  모든 항목을 풀기 때문). DMG 는 컨테이너의 Developer ID 서명과 볼륨 최상위(`HanPage.app`, `/Applications` 링크, 숨김 항목 제외)를 본다.
- 태그 빌드(`REQUIRE_SIGNED=true`)는 tauri-action 의 `releaseId` 출력(단계 `id: tauri`)으로 그 릴리스 하나만 읽어 태그 일치와
  초안 여부를 확인하고, `HanPage_<arch>.app.tar.gz`·DMG 의 `digest` 를 검증한 파일의 SHA-256 과 비교한다. arch 는 DMG 이름에서
  얻어 tauri-action 의 이름 규칙(aarch64·x64)을 따른다. `digest` 가 없으면 자산을 받아 계산한다. API 는 일시 오류에 대비해
  세 번까지 시도한다. 모든 실패는 원인별 `::error` 를 남긴다. `GH_TOKEN` 은 기존 workflow 권한(`contents: write`)의
  `github.token` 을 읽기에만 쓴다.
- workflow_dispatch(초안 없음)는 업로드 대조를 건너뛰고 나머지는 같다.
- macOS 러너를 `macos-15` 로 올린다(`desktop-check.yml` 도 같이). 같은 저장소의 `release-binary.yml` 이 이미 쓰는 이미지다.

**감사 스크립트**(`HanPage-Desktop/scripts/release-audit.mjs`)

- HanPage 전용 디렉터리에 둔다. 루트 `scripts/` 는 upstream 동기화 대상이라 피한다. 0.8.11 증적의 두 스크립트는 보존한다.
- 자산 6개(일반 파일), `latest.json`(최상위 키·RFC 3339 `pub_date`·버전·빈 `notes`·플랫폼 4개·URL·`.sig` 와 바이트 단위로 같은
  서명), 태그의 `tauri.conf.json`(git blob·SHA-256 기록)과 이전 릴리스 태그의 공개키 동일, minisign 키 ID·payload·global 서명과
  앞·중간·끝 1바이트 변조·trusted comment 변조 거부, updater 앱·DMG 앱의 디렉터리 여부·버전·번들 ID·arm64·codesign strict·
  Team ID·Gatekeeper 공증·stapler·같은 CDHash, DMG 컨테이너의 Developer ID 서명과 볼륨 구성, setup.exe 의 VS_FIXEDFILEINFO·
  StringFileInfo 버전과 Authenticode 없음, 아카이브 항목이 `HanPage.app/` 밖으로 나가면 풀지 않음.
- `--release <ID>` 를 주면 GitHub 릴리스의 태그·자산 목록·digest 가 감사한 파일과 같은지 대조한다(공개본 = 감사본).
- updater 서명은 앱의 updater(tauri-plugin-updater 2.10.1·minisign-verify 0.2.5)처럼 엄격하게 디코딩한다(정규 base64,
  잘못된 UTF-8 거부, Rust `str::lines` 와 같은 줄 나누기, 주석 줄 접두어, 74·64바이트, `Ed`/`ED`). 앱은 `latest.json` 의
  서명 문자열을 다듬지 않고 쓰기 때문이다.
- DMG 는 임시 폴더의 복제본을 마운트해 원본이 이미 마운트돼 있어도 충돌하지 않게 한다. 마운트 실패 이유를 결과에 남긴다.
- 결과는 JSON 한 개(배포 기록의 증적), 실패가 있으면 종료 코드 1. 예외가 나거나 Ctrl-C 를 받아도 남은 검사와 정리를 마치고
  JSON 을 남긴다. 자산은 실행하지 않는다. 키 ID 는 기존 기록과 같은 바이트 순서와 minisign 표기를 함께 남긴다.

**문서**: 운영 가이드 §3.1(검증 단계·러너·인증서 만료 2027-02-01·DMG 컨테이너 미공증), §4(절차 8단계 — 태그 전 시험은 PR 브랜치에서
병합 전에 하고 CLI 로 실제 검증 출력 확인, 병합 tree 확인과 빌드 입력이 다를 때 devel 재시험, `--release` 감사와 대조군, 공개 직전
재감사, 공개 뒤 updater 매니페스트 해시 확인, 공개 뒤 재실행 금지), §5(`notes` 빈 값), §7(macOS 배포 문구), §8(트러블슈팅),
Desktop README 한 줄, 문서 manifest 날짜.

**비범위**: DMG 컨테이너 공증·스테이플 — 안의 `.app` 은 공증·스테이플되며 0.8.x 내내 같다. 업로드 뒤 다시 올려야 해 실패
지점이 늘어난다. tauri-action 을 빌드와 업로드로 나누는 구조 변경.

## 3. 영향 분석

| 질문 | 답 |
|---|---|
| 실행 event | `hanpage-desktop-v*` 태그 push 와 workflow_dispatch 의 macOS job. `desktop-check.yml` 러너 변경은 데스크톱 크레이트를 바꾼 PR 의 macOS 검사 |
| 새 실패 조건 | 서명 Team ID 불일치, `.app` 이 디렉터리가 아님, 세 `.app` CDHash 불일치, 아카이브에 `HanPage.app` 밖 항목, DMG 컨테이너 서명·볼륨 구성 위반, 초안 자산 없음·digest 불일치, 릴리스가 이미 공개됨(공개 뒤 재실행). 모두 공개하면 안 되는 상태이며 초안은 공개되지 않은 채 남는다 |
| 오탐 위험 | API 는 세 번까지 시도하고, 그래도 실패하면 실패 job 재실행으로 복구. 이름 규칙(`HanPage_aarch64.app.tar.gz`, DMG 파일명)과 DMG 볼륨 구성은 0.8.x 자산·tauri-bundler 와 같다. `macos-15` 이미지에서의 빌드·서명·공증·검증은 dispatch 실제 실행으로 확인한다 |
| 신뢰 경계 | 새 secret 없음. 태그 빌드만 `github.token` 으로 tauri-action 이 알려 준 릴리스 하나를 읽고, digest 가 없으면 그 자산을 받는다(읽기 전용). fork 는 이 workflow 를 쓰지 않는다 |
| required check | `devel` 보호 규칙 없음 |
| rollback | 변경 PR revert(러너만 되돌리면 macOS 14 은퇴로 다시 실패할 수 있다). 감사 스크립트는 수동 도구라 workflow 와 독립적이다 |

## 4. 검증 계획

- YAML 파싱, 검증 단계 스크립트 `bash -n`, 저장소 워크플로 정책 테스트(`scripts/tests`).
- 검증 단계 스크립트를 YAML 에서 그대로 꺼내 공개된 0.8.12 자산을 러너와 같은 경로에 놓고 실행: 정상, 다른 Team ID, 다른 빌드의
  아카이브, 다시 압축한 아카이브(업로드 파일과 다름), 아카이브 여분 항목, 서명 없는 DMG, DMG 볼륨 여분 항목·앱 링크, DMG 없음,
  다른 릴리스의 자산, 공개된 릴리스(초안 아님), 자산 없는 초안, 릴리스 ID 없음·태그 불일치, API 실패(세 번 시도), digest 없음
  (내려받아 계산), dispatch, 미서명 dispatch·태그. 초안 응답이 필요한 경우는 실제 릴리스 응답을 초안으로 바꿔 돌려주는 `gh` 대역을 쓴다.
- 감사 스크립트: 공개된 0.8.11·0.8.12 자산(양성, `--release` 포함), 변조·버전·공개키·`.sig` 교환·서명 형식·다른 버전 DMG/EXE·
  여분/누락 자산·경로 이탈·읽기 전용 아카이브·서명 없는/구성이 다른 DMG·`latest.json` 키·`pub_date`·다른 릴리스(음성)에서 해당
  검사만 실패하는지, 원본 DMG 가 마운트된 상태와 Ctrl-C 에서도 정리되는지.
- 독립 반박 검토(워크플로·스크립트·문서) 두 차례.
- PR CI(`Desktop check` 의 `macos-15` 포함), 그리고 PR 브랜치의 workflow_dispatch 실제 실행으로 `macos-15` 에서 빌드·서명·공증과
  새 검증 단계가 통과하는지. 업로드 대조는 dispatch 에서 건너뛰므로 로컬 재현으로 확인하고, 첫 태그 빌드에서 다시 본다.
