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
- 적용 승인·결과·run URL: 4절
- 관찰 창과 완료 조건: 계획서 4·5절. 리허설의 판정 항목이 모두 통과하고, 초안 삭제를 재조회로 확인하면 끝난다
- rollback 조건과 명령: 재시도나 인증서 확인이 정상 빌드를 막으면 병합 commit 을 `git revert` 한다. 복제 재시도 단계로 돌아간다. 리허설 초안·태그는 삭제로 끝난다

## 3. 로컬 검증

검증 단계는 YAML 에서 `run` 블록을 그대로 꺼내, 공개된 0.8.12 자산을 러너와 같은 `bundle/` 경로에 놓고 macOS 에서 실행했다(`bash --noprofile --norc -eo pipefail`).
방식은 [#170 보고서](task_m100_hp170_report.md) 3절과 같다.

| 경우 | 결과 |
|---|---|
| 정상(태그·초안) | 통과 — 인증서 경고(`Feb  1 22:12:15 2027 GMT 에 만료됩니다`)와 요약 줄 `Developer ID 인증서 만료`, 대조 줄 2개 |
| digest 없음 / 수동 실행 / 미서명 수동 실행 | 통과(`내려받아 계산` 대조 줄 / 대조 생략 / 경고 후 생략) |
| 인증서 추출 실패 / openssl 실패 | 통과 — `서명 인증서 만료일을 읽지 못했습니다` 경고, 요약은 `확인 못 함` |
| #170 의 음성 16가지 | 전과 같은 `::error` 로 실패 |

감사 스크립트:

| 검사 | 결과 |
|---|---|
| 공개된 0.8.12·0.8.11 자산(`--release` 432269742·408924819) | 각 62/62 통과. 대상 commit 이 로컬 태그와 같음. `signingCertNotAfter` 와 89일 경고가 JSON·stderr 에 남음 |
| 음성 28가지 | 해당 검사만 실패 — #170 의 26가지에 `--rehearsal` 을 실제 릴리스에 준 경우, 리허설 번호가 다른 경우 |
| 양성 대조 3가지 | 원본 DMG 가 이미 마운트된 상태, `--release` 정상, 리허설 태그 형태(`latest.json` URL 을 바꾸고 로컬 임시 태그로 감사, 태그는 지웠다) |
| 인자 10가지 | `--rehearsal` 값 누락·빈 값·0·문자·소수, 없어진 `--tag`, `--release` 오류, 오타, 버전 누락은 사용법 오류(종료 2) |

| 기타 | 결과 |
|---|---|
| YAML 파싱(ruby), 검증·공개 릴리스 확인 단계 `bash -n` | 통과 |
| 저장소 워크플로 정책 테스트 29개(`scripts/tests`) | 모두 통과 |
| 문서 링크(`scripts/check_markdown_links.py`) | 이상 없음 |
| 일회용 브랜치의 `dmg-probe.sh` | 로컬에서 AppleScript 를 끄고 시험. 강제 실패, 시간 초과(종료 124), 같은 러너 재시도, 합계 출력, 정리 확인 |

## 4. 실제 실행

병합 뒤 기록 commit 에서 채운다(PR head 를 움직이면 CI 가 다시 돌고 리허설한 commit 과 달라진다). 항목은 계획서 5절의 표다.

## 5. 독립 검토

읽기 전용 반박 검토를 두 차례 했다(영역마다 검토자 1명, 중간 이상 발견마다 검증자 2명).

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

## 6. 남은 확인

계획서 6절과 같다. 리허설 뒤에 실제 결과를 반영해 다시 적는다.
