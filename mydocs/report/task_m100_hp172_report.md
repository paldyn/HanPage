# Task #172 (M100) — Desktop 릴리스 태그 경로 리허설과 후속 정리 최종 보고서

Issue: #172 · 계획: [task_m100_hp172.md](../plans/task_m100_hp172.md) · 워크플로 `.github/workflows/desktop-release.yml` · 스크립트 `HanPage-Desktop/scripts/release-audit.mjs`

## 1. 결과 요약

#170 후속으로 남긴 네 가지를 처리했다.

- **태그 경로와 재시도**: 복제 재시도 단계를 tauri-action 의 `retryAttempts: 1` 로 바꿨다. `tauri build` 와 자산별 POST 만 다시 시도해 범위는 전보다 좁고, 그 밖의
  GitHub API 일시 오류는 실패 job 재실행으로 복구한다. 태그 전용 경로를 실제 태그 전에 시험하는 리허설을 운영 가이드 §4.1 에 넣고, 감사 스크립트에 `--rehearsal`
  을 더했다. 검증 단계는 초안 자산 대조가 어느 분기로 통과했는지 로그에 남긴다. 실제 실행 결과는 4절.
- **PR #137(tauri-action 1.0)**: 병합하지 않는다. `latest.json` 의 다운로드 주소가 시간당 60회 제한의 API 주소로 바뀌어 공개된 앱의 업데이트 경로가 그 한도에
  묶이고, 태그에서만 도는 digest 대조가 깨진다. 1.0 의 이득은 가드와 절차로 막고 있거나 작다. 판단은 운영 가이드 §7 에 있다.
- **Developer ID 인증서**: 갱신은 계정 소유자만 할 수 있어 잊지 않게 만들었다. 검증 단계와 감사 스크립트가 실제 인증서를 읽어 만료 90일 전부터 경고한다.
  운영 가이드 §3.1.1 에 Apple 안내에 맞춘 절차(G2 Sub-CA, 1년 유효)를 적었다. 실제 갱신은 작업지시자 결정으로 뒤로 미뤘고 [#173](https://github.com/paldyn/HanPage/issues/173) 으로 추적한다.
- **v0.8.6 초안**: 릴리스 노트 원문을 [배포 기록](../pr/assets/desktop_v086_release/draft-release-notes.md)으로 옮겼다. 삭제는 되돌릴 수 없어 작업지시자가 실행한다.

리허설이 실제 결함을 찾았다. 2026-11-04 러너에서 Apple 서버 연결이 자주 끊겨(`stapler validate` 270회 중 39회), 티켓이 있어도 검증 단계가 job 을 실패시켰다. 빌드 안의
`stapler staple` 도 끊기는데(120회 중 14회) tauri 가 그 실패를 무시해 티켓 없는 앱이 만들어진다. 검증 단계·감사 스크립트의 `validate` 와 `xcrun` 대역을 통한 `staple` 을 다섯 번까지
시도하게 고쳤다. 자세한 내용은 4절.

DMG 일시 실패의 원인은 확정하지 못했다. #170 때 Finder AppleScript 를 원인으로 적었지만 근거가 없었고, 실패 시각은 3건 중 2건이 AppleScript 시작 전임을 가리킨다.
워크플로 주석과 운영 가이드 §8 을 원인 미확정으로 고쳤다.

## 2. GitHub 운영 변경 기록

- 등급: O4(tag push·Release) + O3(job 명령)
- 근거: #172, 작업지시자 지시(2026-11-04) "이거다하자"
- 기준 시각 / SHA: 2026-11-04, `origin/devel` `39ca2a7fc8`(PR base)
- 대상 workflow·설정: `.github/workflows/desktop-release.yml` 의 tauri-action 단계(`retryAttempts`, 복제 단계 제거)와 검증 단계(인증서 만료, 대조 로그).
  저장소 설정·branch protection·secret 은 바꾸지 않는다
- 적용 전(base `39ca2a7fc8`): tauri-action 단계 `continue-on-error` + 복제 단계 `Build & bundle (retry)`, 인증서 만료 확인 없음
- 적용 후(이 PR): 1절과 같다
- 영향 event·branch·required check: `hanpage-desktop-v*` 태그 push, workflow_dispatch. required check 없음
- 변경 파일: `.github/workflows/desktop-release.yml`, `HanPage-Desktop/scripts/release-audit.mjs`, `mydocs/manual/desktop_auto_update.md`, `mydocs/README.md`,
  `mydocs/pr/assets/desktop_v086_release/draft-release-notes.md`, 계획서·이 보고서
- 로컬 검증: 3절
- 적용 승인·결과·run URL: 4절. 승인은 작업지시자 "6가지 전부 진행"(2026-11-04, 이슈·PR·리허설 태그·일회용 브랜치·재실행·#137), "4가지 전부 진행"(2026-11-04, 2회차), "응 알아서해줘"(2026-11-10, 병합·기록·브랜치 정리)
- 관찰 창과 완료 조건: 계획서 4·5절. 리허설의 판정 항목이 모두 통과하고, 초안 삭제를 재조회로 확인하면 끝난다
- rollback 조건과 명령: 재시도나 인증서 확인이 정상 빌드를 막으면 병합 commit 을 `git revert` 한다. 복제 재시도 단계로 돌아간다. 리허설 초안·태그는 삭제로 끝난다

## 3. 로컬 검증

검증 단계는 YAML 에서 `run` 블록을 그대로 꺼내, 공개된 0.8.12 자산을 러너와 같은 `bundle/` 경로에 놓고 macOS 에서 실행했다(`bash --noprofile --norc -eo pipefail`).
방식은 [#170 보고서](task_m100_hp170_report.md) 3절과 같다.

| 경우 | 결과 |
|---|---|
| 정상(태그·초안) | 통과 — 인증서 경고(`Feb  1 22:12:15 2027 GMT 에 만료됩니다`)와 요약 줄 `Developer ID 인증서 만료`, 대조 줄 2개 |
| `stapler` 한 번 끊김 / 계속 끊김 / 티켓 없음 | 재시도로 통과(`validate action worked` 3회) / 다섯 번 뒤 `macOS 공증` 으로 실패 / 호출 1번 만에 `공증 티켓이 없습니다` 로 실패 |
| digest 없음 / 수동 실행 / 미서명 수동 실행 | 통과(`내려받아 계산` 대조 줄 / 대조 생략 / 경고 후 생략) |
| 인증서 추출 실패 / openssl 실패 | 통과 — `서명 인증서 만료일을 읽지 못했습니다` 경고, 요약은 `확인 못 함` |
| #170 의 음성 16가지 | 전과 같은 `::error` 로 실패 |

감사 스크립트:

| 검사 | 결과 |
|---|---|
| 공개된 0.8.12·0.8.11 자산(`--release` 432269742·408924819) | 각 62/62 통과. 대상 commit 이 로컬 태그와 같음. `signingCertNotAfter` 와 89일 경고가 JSON·stderr 에 남음 |
| 음성 30가지 | 해당 검사만 실패 — #170 의 26가지에 `--rehearsal` 을 실제 릴리스에 준 경우, 리허설 번호가 다른 경우, `stapler` 계속 끊김(앱마다 5회), 티켓 없음(앱마다 1회) |
| 양성 대조 4가지 | 원본 DMG 가 이미 마운트된 상태, `--release` 정상, 리허설 태그 형태(`latest.json` URL 을 바꾸고 로컬 임시 태그로 감사, 태그는 지웠다), `stapler` 한 번 끊김 |
| 인자 10가지 | `--rehearsal` 값 누락·빈 값·0·문자·소수, 없어진 `--tag`, `--release` 오류, 오타, 버전 누락은 사용법 오류(종료 2) |

| 기타 | 결과 |
|---|---|
| YAML 파싱(ruby), 검증·공개 릴리스 확인 단계 `bash -n` | 통과 |
| 저장소 워크플로 정책 테스트 29개(`scripts/tests`) | 모두 통과 |
| 문서 링크(`scripts/check_markdown_links.py`) | 이상 없음 |
| 일회용 브랜치의 `dmg-probe.sh` | 로컬에서 AppleScript 를 끄고 시험. 강제 실패, 시간 초과(종료 124), 같은 러너 재시도, 합계 출력, 정리 확인 |
| `xcrun` 대역 | 단계를 YAML 에서 꺼내 실행. 가짜 `xcrun` 으로 통과(1회), 두 번 실패 뒤 통과(3회), 계속 실패(5회, 종료 1), `validate`·`notarytool` 은 그대로 넘김. 실제 `stapler staple` 로 티켓을 지운 사본에 같은 티켓이 다시 붙음 |

## 4. 실제 실행

### 리허설 1회차 (`0f93f796ab`, 태그 `hanpage-desktop-v0.8.12-rehearsal.1`)

| 실행 | 결과 |
|---|---|
| 태그 run [41766786305](https://github.com/paldyn/HanPage/actions/runs/41766786305) 첫 실행 | 두 job 의 공개 릴리스 확인 통과(릴리스 없음), 초안 433493223 하나에 자산 6개, 대상 commit `0f93f796ab`. Windows 성공. macOS 는 번들·아카이브의 `stapler validate` 뒤 DMG 안 `.app` 에서 `Error 68` 로 실패 |
| 같은 run 의 macOS job 재실행 | 공개 릴리스 확인 통과(초안 있음), `Found draft release`, `Deleting existing` 3회, 같은 초안에 올림. 자산 id 는 macOS 3개와 `latest.json` 만 바뀌고 Windows 2개는 그대로. 검증 단계는 첫 `stapler validate` 에서 `Error 68` 로 실패 |
| 일회용 브랜치 수동 실행 [41766893337](https://github.com/paldyn/HanPage/actions/runs/41766893337) | 재시도 동작 확인: 첫 시도가 `Copying volume icon file`, `rehearsal: forced SetFile failure` 뒤 실패, `Attempt 1 failed, retrying...`, 두 번째 시도가 공증 `Accepted`·AppleScript·`Disk image done` 까지 끝나 단계 성공. 이미지가 붙은 채 남은 상태는 두 번째 시도를 막지 않았다. job 토큰 조회는 `v0.8.12` 200, `v0.8.6` 404, 없는 태그 404. 검증 단계는 같은 `Error 68` 로 실패해 macOS artifact 는 올라가지 않았다 |
| `dmg-probe`(같은 run, 러너 4대) | 20회 모두 성공, 1회 18~24초. 빈 러너에서는 재현되지 않았다 |
| `stapler validate` 측정 [41773521203](https://github.com/paldyn/HanPage/actions/runs/41773521203)(04:29 UTC, 러너 3대, 공개된 0.8.12 앱) | 150회 중 20회 실패(러너별 0·12·8회/50회). 모두 `Code=-1005`, `Error 68`, 실패 한 번에 약 7초, 연속 실패는 2회까지. 측정 시작 직전부터 약 50초 겹쳐 돈 로컬 Mac 은 24회 모두 성공 |
| `stapler staple`·`validate` 측정 [41778733001](https://github.com/paldyn/HanPage/actions/runs/41778733001)(05:21 UTC, 러너 3대) | `staple` 120회 중 14회 실패(러너별 2·3·9회/40회), 종료 68, 티켓이 생기지 않음. `validate` 120회 중 19회 실패(7·4·8회/40회). 티켓 없는 앱의 `validate` 는 세 러너 모두 종료 65 |

`stapler validate` 는 Apple 서버의 최신 티켓과 비교한다(`man stapler`). 이 오류는 리허설 전 로컬 감사에서 한 번 나와 일시 오류로 적어 둔 것이다. job 을 실패시킨다는 것과 빈도는
리허설로 처음 알았다. #124 이후 11-03 까지 검증 단계 11번(호출 24번, `macos-14` 18번·`macos-15` 6번)은 모두 통과했고 실패는 모두 11-04 에 나왔다. 언제부터 왜 그런지는 모른다.
호출 독립을 가정하면 재시도 없는 검증 단계는 3번에 1번꼴로 실패할 빈도이고, 같은 날 실제 검증 단계는 3번 모두 실패했다. 측정한 앱은 풀어 놓은 앱이고 DMG 안 `.app` 은 재지 않았다.

빌드 안의 `stapler staple` 은 tauri 가 종료 코드를 보지 않아(`tauri-macos-sign` `staple_app`) 끊기면 티켓 없는 앱으로 빌드가 성공한다. 실제 빌드 22회는 이 흔적이 없었지만 측정에서 끊겼다.
검증 단계·감사 스크립트의 `validate` 는 티켓이 없다는 답이 아니면 다섯 번까지 시도하고, `staple` 은 `xcrun` 대역으로 다섯 번까지 시도하도록 고쳤다. 초안 digest 대조는 그 앞의 실패 때문에
1회차에서는 돌지 않았다.

### 리허설 2회차 (`28d9db73ce`, 태그 `hanpage-desktop-v0.8.12-rehearsal.2`)

| 실행 | 결과 |
|---|---|
| 태그 run [41781157824](https://github.com/paldyn/HanPage/actions/runs/41781157824) 첫 실행 | 두 job 성공. 공개 릴리스 확인 통과(릴리스 없음), 초안 433572957 하나에 자산 6개, 대상 commit `28d9db73ce`. 검증 단계 머리 `REQUIRE_SIGNED: true`·`RELEASE_ID: 433572957`, `stapler staple try 1: exit 0`(대역이 쓰였다), `The validate action worked!` 3회, `초안 자산 대조 통과` 2줄(GitHub digest), annotation `Developer ID 인증서` 경고, `macOS 미서명` 없음 |
| 같은 run 의 macOS job 재실행 | 성공. 공개 릴리스 확인 통과(초안 있음), `Found draft release`, `Deleting existing` 3회, 검증 단계의 대조 통과. 자산 id 는 macOS 3개와 `latest.json` 만 바뀌고 Windows 2개는 그대로 |
| 초안 감사 `--release 433572957 --rehearsal 2` | 62/62 통과([`rehearsal-audit.json`](../pr/assets/desktop_rehearsal_20261104/rehearsal-audit.json)). 같은 태그의 릴리스 1개, `latest.json` 플랫폼 4개와 리허설 태그 URL, 대상 commit 이 로컬 태그와 같음. 89일 인증서 경고 |
| 일회용 브랜치 수동 실행 [41781166794](https://github.com/paldyn/HanPage/actions/runs/41781166794) | 성공. 첫 시도가 `Copying volume icon file`, `rehearsal: forced SetFile failure` 뒤 실패, `Attempt 1 failed, retrying...`, 공증 `Accepted` 2회, 두 번째 시도의 AppleScript·`Disk image done`, `stapler staple try 1: exit 0` 2회(빌드마다). job 토큰 조회 `v0.8.12` 200, `v0.8.6` 404, 없는 태그 404 |
| 재시도로 만든 DMG(위 run 의 artifact) | 볼륨에 `HanPage.app`·`/Applications` 링크·`.VolumeIcon.icns`·`.DS_Store`. `.DS_Store` 는 #170 의 `macos-15` 수동 실행 DMG 와 같은 `706ae760…`, 아이콘 좌표 180,170·480,170, 볼륨 속성 C, Developer ID 서명, 안의 `.app` 은 공증 티켓 확인 |
| 바깥 영향 | 회차마다 태그 push 전과 재실행 뒤의 스냅샷([before-1](../pr/assets/desktop_rehearsal_20261104/snapshot-before-1.txt), [before-2](../pr/assets/desktop_rehearsal_20261104/snapshot-before-2.txt), [after-2](../pr/assets/desktop_rehearsal_20261104/snapshot-after-2.txt)): 공개 릴리스 432269742 의 자산 id·`updated_at`·digest, `latest.json` 해시, `releases/latest` 그대로. 태그는 리허설 태그만 늘었다 |

2회차 실행(06:11~06:37 UTC)에서는 Apple 서버 연결이 끊기지 않아 `stapler` 재시도가 실제 끊김에서 복구하는 것은 보지 못했다. 로그 발췌는
[`log-excerpts.txt`](../pr/assets/desktop_rehearsal_20261104/log-excerpts.txt)에 있다.

### PR CI

head `28d9db73ce` 에서 31 성공·2 실패·5 건너뜀. 실패는 `test-archive-b-shard-1` 의 `issue_6612_sample14_wmf_pictures_reach_pdf`(기대 21, 실제 101)와 그 집계 `Build & Test` 다. 이 PR 은
엔진을 바꾸지 않았고 같은 테스트가 직전 head `0f93f796ab` 에서 통과했다. 통과한 job 은 `ubuntu-26.04` 20261025.179.1, 실패한 job 은 20261101.186.1 이미지였고 실패한 쪽에만
`svg2pdf SubsetError` 와 `embed_text=false` 재시도 경고가 나온다. 같은 이미지에서 11-07 의 다른 PR 7개도 같은 이유로 실패한다. 작업지시자 결정("응 알아서해줘", 2026-11-10)으로 이
사유를 남기고 병합했다(`80465772f2`). 테스트 실패는 별도로 고친다.

## 5. 독립 검토

읽기 전용 반박 검토를 세 차례 했다(영역마다 검토자 1명, 중간 이상 발견마다 검증자 2명).

- 1차(에이전트 24): 태그 경로에서 버전을 버리게 만드는 결함과 공개 릴리스를 건드리는 경로는 없었다. 검증자 판정은 모두 low 이하였다(high 1건은 이 변경의 결함이 아니라고
  판정, medium 8건은 low 로 조정). 마지막 완결성 검토의 medium 3건은 검증자 없이 받아들였다. 다음을 고쳤다.
  - 리허설: `SetFile` 대역을 태그에 넣으면 하나뿐인 재시도를 대역이 써 버려, 실제 DMG 실패가 끼면 검증 단계까지 못 간다. 태그는 PR head 에 바로 붙이고 재시도는 일회용
    브랜치로 나눴다. 초안이 있는 경로는 macOS job 재실행으로 지나게 했다. 바깥 영향은 스냅샷 비교로 바꿨다.
  - 사실과 달랐던 문장: DMG 실패 원인, `retryAttempts` 가 복제 단계와 같은 일을 한다는 설명, tauri-action 1.0 의 이득이 없다는 설명, 공개 릴리스를 만났을 때의 1.0 동작.
  - 빠져 있던 것: G2 인증서 1년 유효, 같은 이름 인증서 구분, Dependabot 이 다음 1.x 를 다시 여는 문제(`@dependabot ignore this major version`), 원격 명령표, 이슈 완료
    조건에 초안 삭제 포함, 리허설 기록 경로.
  - 감사 스크립트: `--tag` 가 임의 문자열을 받아 태그와 자산 버전이 다른 릴리스가 통과할 수 있어 `--rehearsal <N>` 으로 바꿨다. 공개키를 `refs/tags/` 에서만 읽고 대상 commit
    대조를 더했다. 인증서를 못 읽는 경우와 경고가 터미널에 안 나오는 문제를 고쳤다.
- 2차(에이전트 12): 1차 지적 38건 중 33건 해결, 1건 보류(이슈 번호), 4건 미해결을 짚었고 모두 반영했다. 태그 경로 결함은 없었다. 검증자 판정은 모두 low 이하다.
  - 계획서 명령표의 `gh` 명령에 `--repo` 가 없어 이 clone 에서는 upstream 으로 간다. 표의 모든 명령을 실행할 수 있는 형태로 고쳤다.
  - §4.1 이 재실행을 기다리지 않고 감사로 넘어가 재실행 전 자산을 감사할 수 있었다. 재실행 뒤 증거가 섞이는 문제와 함께 고쳤다.
  - 판정 문구가 정상 실행에서 맞지 않았다. 대조 구간이 로그를 남기지 않아 대조 줄을 더했다.
  - 초안이 둘이 되는 경합, `latest.json` 이 지워진 채 끝난 경우, stapler `Error 68` 일시 실패를 §8 에 적었다.
  - `dmg-probe` 는 멈춤에서 증거를 잃고 결과를 과하게 읽을 수 있어 회당 제한, 재시도 대기, 러너 4개를 넣고 참고용으로 낮췄다.
- 3차(에이전트 15, 리허설 1회차 뒤 `stapler` 수정 대상): 재시도 로직은 맞게 동작했다. 빌드 안의 `stapler staple` 이 같은 서버를 쓰는데 재시도가 없고 tauri 가 실패를 무시한다는 지적(high, 검증자
  판정 medium)을 받아 러너에서 재 보고 `xcrun` 대역을 넣었다. 티켓 없음과 연결 실패를 나눴고, 보고서의 표본 표기(전날 호출 수, 이전 통과 기록), 지웠던 로컬 실패 기록, 판정표의 회차 표기를 고쳤다.
  감사 쪽 재시도 시험을 더하고 대기 중 중단을 처리했다.

## 6. 남은 확인

- 공개 릴리스를 막는 분기(`exit 1`)의 실제 실행. job 토큰 조회로 전제(공개 200, 초안 404)만 확인했다.
- 검증 단계의 digest 없음 다운로드 분기와 `초안 아님` 실패 분기(로컬 재현). 두 회차 모두 GitHub digest 로 대조했다.
- 자산 업로드 재시도, Windows 의 `tauri build` 재시도.
- `stapler` 재시도가 실제 끊김에서 복구하는 것(2회차에는 끊김이 없었다). Apple 서버 연결이 끊기는 원인과 앞으로의 빈도.
- 실제 DMG 일시 실패의 단계와 같은 러너 복구. `dmg-probe` 는 빈 러너에서 재현하지 못했다. 다음 실제 실패의 `--verbose` 출력이 첫 증거다.
- 설치된 앱의 업데이트 경로. 리허설은 공개하지 않으므로 지나지 않는다.
- G2 인증서로 서명한 자산의 태그 경로와 감사. 인증서를 바꾼 뒤의 리허설에서 본다(#173).
- 리허설 초안 2개·태그와 v0.8.6 초안 삭제. 되돌릴 수 없어 작업지시자가 실행하고, 재조회로 확인한 뒤 #172 를 닫는다.
