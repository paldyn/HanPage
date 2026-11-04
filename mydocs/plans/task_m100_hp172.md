# Task #172 (M100) — Desktop 릴리스 태그 경로 리허설과 후속 정리 수행계획서

- **이슈**: [paldyn/HanPage#172](https://github.com/paldyn/HanPage/issues/172)
- **브랜치**: `claude/desktop-release-rehearsal` (`origin/devel` `39ca2a7fc8` 기준)
- **작업지시자 지시(2026-11-04)**: #170 후속으로 남긴 네 가지(태그 경로 첫 실행, Dependabot PR #137, Developer ID 인증서 만료,
  v0.8.6 초안)에 "이거다하자"
- **등급**: [GitHub 운영 매뉴얼](../manual/github_operations.md) O4(tag push·Release) + O3(job 명령). 원격 변경은 4절의 명령을 제시해 명시
  승인을 받은 뒤 실행한다. 저장소 설정·secret·branch protection 은 바꾸지 않는다

## 1. 현재 상태 (2026-11-04)

| 항목 | 값 |
|---|---|
| 태그 전용 경로 | 공개 릴리스 확인, 초안 여부·digest 대조는 workflow_dispatch 에서 돌지 않아 로컬 재현으로만 확인했다. 태그는 옮기지 않으므로 여기서 결함이 나면 그 버전을 버려야 한다 |
| 재시도 | `Build & bundle (retry)` 가 tauri-action 단계를 통째로 복제했고 실제로 돈 적이 없다. 두 단계를 같이 고쳐야 한다. tauri-action 0.6.2 에는 `tauri build` 와 자산 업로드를 다시 시도하는 `retryAttempts` 입력이 이미 있다 |
| DMG 일시 실패 | 세 번 있었다(run 40627099903, 41574281039 첫 시도, 41600452028). 모두 `--verbose` 전이라 어느 단계인지 모른다. 복구는 모두 새 러너에서였다 |
| PR #137 | Dependabot 의 tauri-action 0.6.2 → 1.0.0. 2026-10-10 부터 열려 있다 |
| Developer ID 인증서 | 2027-02-01 22:12:15 GMT 만료. 2026-06-14 에 발급됐지만 구 `Developer ID Certification Authority` 의 만료일에 묶여 있다. 알려 주는 장치가 없다 |
| v0.8.6 초안 | 릴리스 ID 401464978, 자산 0개, 2026-10-02 생성. git 태그 `hanpage-desktop-v0.8.6` 은 있고 그 버전은 공개되지 않았다 |

## 2. 설계

**재시도**(`desktop-release.yml`): 복제 단계와 `continue-on-error` 를 지우고 tauri-action 에 `retryAttempts: 1` 을 준다. 검증 단계의 `RELEASE_ID` 는
`steps.tauri.outputs.releaseId` 하나만 읽는다. 복제 단계보다 범위가 좁다. `tauri build`(최대 2회)와 자산별 POST(최대 2회)만 다시 시도하고, 릴리스
조회·생성, 자산 목록·삭제, `latest.json` 읽기·삭제의 일시 오류는 job 실패가 된다. 실패 job 재실행으로 복구되고 버전을 버리지는 않는다(`latest.json` 이
지워진 채 끝났으면 다른 플랫폼 job 도 다시 실행한다). 로그로 확인한 tauri-action 단계 실패는 모두 `tauri build` 실패였고, 복제 단계를 유지보수하는 비용이
더 크므로 받아들인다. 재시도가 성공하면 job 이 성공으로 끝나므로 돌았는지는
로그로 본다(운영 가이드 §4).

**태그 경로 리허설**(운영 가이드 §4.1): 셋으로 나눈다. 대역을 태그에 넣으면 하나뿐인 재시도를 대역이 써 버려, 실제 DMG 실패가 한 번만 끼어도 macOS
job 이 실패하고 리허설의 주목적인 검증 단계가 돌지 않는다.

1. 태그 `hanpage-desktop-v0.8.12-rehearsal.1` 을 이 PR head 에 바로 붙인다. 일회용 commit 없이 병합할 워크플로가 그대로 돌고, 릴리스가 없을 때의 공개
   릴리스 확인, 초안 생성, 검증 단계의 초안·digest 대조를 지난다. 공개 릴리스와 `releases/latest` 는 바뀌지 않는다.
2. 그 run 이 끝나면 첫 실행의 결과를 저장하고 macOS job 만 다시 실행한다. 최근 공개 태그 6개 중 2개가 쓴 복구 경로를 포함해, 초안이 있을 때의 공개
   릴리스 확인과 `latest.json` 합치기를 지난다. 성공한 job 을 다시 돌리므로 그 복구에서는 일어나지 않았던 같은 이름의 자산 교체까지 지난다. 감사는 재실행이
   끝난 뒤에 돌린다.
3. 재시도는 태그가 필요 없다. 일회용 브랜치 `claude/desktop-release-retry-shim` 을 수동 실행한다. 이 브랜치에는 다음만 더한다.
   - `SetFile` 대역. 첫 호출에서 자신을 지우고 실패해 첫 `tauri build` 를 `bundle_dmg.sh` 가 볼륨을 마운트한 뒤에 멈추게 한다. 실패 3건 모두 job 끝에
     `diskimages-help` 고아가 남아 이미지가 붙은 채 끝난 것으로 보이므로 그 상태를 만든다. 입증하는 것은 재시도 배선과 그 상태에 대한 내성이다.
   - job 토큰으로 `releases/tags/hanpage-desktop-v0.8.12`(공개)와 `releases/tags/hanpage-desktop-v0.8.6`(초안)을 읽는 단계. 공개 릴리스 확인의 전제를
     실제 `GITHUB_TOKEN` 으로 확인한다. 그래서 초안 삭제는 이 뒤에 한다.
   - `dmg-probe` job(러너 4개, 각 5회). 시크릿 없이 공개된 앱으로 `bundle_dmg` 만 되풀이해, 실패하면 단계를 남기고 180초 뒤 같은 러너에서 다시 돌린다. 복구가
     모두 새 러너에서였고 실패 3건은 모두 러너의 첫 DMG 였으므로 그 조건을 몇 개 모은다. 빌드 뒤 실행이 아니므로 재현돼도 재현되지 않아도 참고로만 쓴다.

리허설 뒤 PR head 의 워크플로나 감사 스크립트가 바뀌면 `rehearsal.2` 로 다시 돌린다. 문서만 바뀌면 다시 돌리지 않는다.

**감사 스크립트**: `--rehearsal <N>` 으로 태그 `hanpage-desktop-v<버전>-rehearsal.<N>` 의 초안을 감사한다. 태그 문자열을 직접 받지 않는 것은 URL 검사와
릴리스 태그 대조가 태그와 자산 버전이 같다는 것까지 입증하기 때문이다. 공개키는 `refs/tags/<태그>` 에서만 읽고, `--release` 는 릴리스 대상 commit 이
로컬 태그 commit 과 같은지도 본다. 이전 릴리스를 고를 때는 숫자 버전 태그만 보므로 리허설 태그가 남아 있어도 다음 감사의 기준이 되지 않는다.

**tauri-action 1.0(PR #137)**: 병합하지 않고 이유를 남겨 닫는다. 1.0.0 소스(`1deb371b0c`)와 실측으로 확인한 내용이다.

- 잃는 것: `latest.json` 의 URL 이 `github.com/.../releases/download/...` 에서 `api.github.com/.../releases/assets/<id>` 로 바뀌고 이를 고르는 입력이 없다
  (`upload-version-json.ts:97`). 앱의 updater 2.10.1 은 `Accept: application/octet-stream` 을 보내 받을 수는 있다. 다만 로그인 없는 API 는 IP 당 시간당
  60회이고 302 응답만으로 1회를 쓴다(실측 `x-ratelimit-limit: 60`, used 8→9). 같은 IP 의 다른 API 사용과 한도를 나눈다. 받은 파일은 메모리에만 있어 적용을
  미룬 PC 는 실행할 때마다 다시 쓴다. 넘기면 자동 재시도 없이 업데이트 실패로 표시되고, 앱 사용은 막히지 않는다. 지금 규모(0.8.11 업데이터 파일 약 24일에
  12회)로 HanPage 만으로 닿기는 어렵지만 학교·회사처럼 IP 를 나눠 쓰는 곳에서는 다른 트래픽으로 소진될 수 있고, 공개된 앱의 업데이트 경로를 바꾸는 일이다.
  `api.github.com` 을 막은 기관망에서 되는지도 모른다.
- 얻는 것: 공개 릴리스 자산을 지우지 않음, `latest.json` 처리 전체 재시도와 재시도 사이 대기, `untagged-` URL 함정 해소. 앞의 것과 뒤의 것은 가드와
  절차로 막고 있고 나머지는 작다. tauri-action 자체 advisory 는 없고, 0.6.2 도 이미 `node24` 라 올려야만 하는 시한은 찾지 못했다. 주소가 자산 id 에 묶여
  공개 뒤 자산을 교체하면 그 주소가 무효가 되는 새 제약도 생긴다.
- 그대로 병합하면: `.app.tar.gz` 이름이 바뀌어 태그에서만 도는 digest 대조가 `초안 자산` 오류로 실패한다. PR CI 와 수동 실행에는 보이지 않는다.
- 공개 릴리스를 만났을 때 1.0.0 은 소스상 경고 뒤 `createRelease` 로 넘어간다. GitHub 가 받아들이는지는 미확인이다. 어느 쪽이든 공개 자산은 지우지 않는다.
- 다시 볼 조건: 0.x 로 고칠 수 없는 결함이 나오거나 upstream 이 주소를 고르게 해 줄 때. 그때는 `uploadUpdaterJson: false` 와 별도 job 으로 지금 주소를
  유지할 수 있다. API 주소 전환 경위는 tauri-action #1297(private repo 용 opt-in 요청)과 #1315 에 있다.

닫을 때는 이유를 남기고 `@dependabot ignore this major version` 을 쓴다. 닫기만 하면 다음 1.x 가 나올 때 다시 열린다. `.github/dependabot.yml` 의 ignore 는
Dependabot 이 읽는 `main` 에 가야 효과가 있어 쓰지 않는다. 운영 가이드 §7 과 워크플로 주석에 판단을 남긴다.

**Developer ID 인증서**: 갱신은 계정 소유자만 할 수 있어 대신할 수 없다. 대신 잊지 않게 만든다. Apple 은 G2 인증서를 1년 유효로 안내하므로 해마다
돌아오는 일로 다룬다.

- 검증 단계가 서명 인증서 만료일을 job 요약에 남기고, 90일 안이면 `::warning` 을 낸다. 만료일을 읽지 못하면 그 사실을 경고한다. 감사 스크립트는
  `signingCertNotAfter`·`warnings` 와 stderr 에 남긴다. 알림일 뿐이라 둘 다 실패로 치지 않는다. 고정 날짜가 아니라 실제 인증서를 읽으므로 갱신 뒤에도 그대로 쓴다.
- 운영 가이드 §3.1.1 에 Apple 안내에 맞춘 갱신 절차를 적는다. **G2 Sub-CA** 를 고르지 않으면 같은 만료일에 다시 묶일 수 있다.
- 이번 갱신은 [#173](https://github.com/paldyn/HanPage/issues/173) 으로 추적한다(작업지시자 결정: 나중에 한다). 교체 뒤 §3.1.1 의 수동 실행과 §4.1 리허설을 한 번 더 돌린다(이번 리허설은 구 인증서로 돈다).

**v0.8.6 초안**: 릴리스 노트 원문을 [배포 기록](../pr/assets/desktop_v086_release/draft-release-notes.md)으로 옮긴다. 삭제는 되돌릴 수 없어 작업지시자가
직접 실행한다. `--cleanup-tag` 없이 지운다. git 태그는 "이 번호를 이미 썼다"는 기록이라 남긴다. 노트가 병합된 뒤, 위의 조회 단계가 끝난 뒤에 지운다.

**비범위**: tauri-action 업그레이드, DMG 창 배치 단계 끄기(아이콘 배치가 사라지고 원인도 미확정이다), 다른 Dependabot PR.

## 3. 영향 분석

| 질문 | 답 |
|---|---|
| 실행 event | `hanpage-desktop-v*` 태그 push 와 workflow_dispatch 의 두 job |
| 새 실패 조건 | 빌드 뒤 GitHub API 일시 오류가 같은 job 안에서 자동 복구되지 않는다(2절). 인증서 확인은 경고만 낸다 |
| 바뀌는 동작 | 재시도가 성공하면 전처럼 실패 단계가 남지 않고 로그의 `Attempt 1 failed, retrying...` 한 줄로만 보인다. 업로드 재시도는 대기 없이 바로 다시 보낸다 |
| 리허설의 바깥 영향 | 태그 하나, 초안 하나, 일회용 브랜치 하나가 시험 동안 보인다. 공개 릴리스·`latest.json`·기존 앱에는 영향이 없다. tauri-action 은 `tag_name` 정확 일치로만 릴리스를 찾고, 이 태그 패턴은 `desktop-release.yml` 만 실행한다 |
| 신뢰 경계 | 새 secret 없음. 일회용 브랜치의 commit 과 run 기록은 지워도 공개 저장소에 남으므로 비밀을 넣지 않는다. `dmg-probe` job 은 시크릿을 쓰지 않는다 |
| required check | `devel` 보호 규칙 없음 |
| rollback | 이 PR 을 되돌리면 복제 재시도 단계로 돌아간다. 리허설 초안·태그는 삭제로 끝난다 |

## 4. 원격 변경과 승인

순서대로 실행한다. 태그 push 전과 5번 뒤에 스냅샷을 저장해 비교한다: `gh api repos/paldyn/HanPage/releases/latest --jq .id`,
`curl -sL https://github.com/paldyn/HanPage/releases/latest/download/latest.json | shasum -a 256`, 릴리스 432269742·401464978 의 자산 id·`updated_at`, 태그 목록.
tauri-action 은 같은 이름 자산을 지우고 다시 올리므로 공개 릴리스를 건드렸다면 자산 id 가 달라진다.

| # | 명령 | 대상 | 실행 | 복구 |
|---|---|---|---|---|
| 1 | `gh issue create --repo paldyn/HanPage --title <제목> --body-file <파일>` 두 건 | 이 작업, 인증서 갱신 | Claude | `gh issue close <번호> --repo paldyn/HanPage` |
| 2 | `git push -u origin claude/desktop-release-rehearsal`, `gh pr create --repo paldyn/HanPage --base devel --head claude/desktop-release-rehearsal --title <제목> --body-file <파일>` | PR(`Refs #172`) | Claude | `gh pr close <PR> --repo paldyn/HanPage` |
| 3 | `git tag -a hanpage-desktop-v0.8.12-rehearsal.1 <PR head> -m "rehearsal"`, `git push origin hanpage-desktop-v0.8.12-rehearsal.1` | 태그, 초안 | Claude | 9번 |
| 4 | `git push origin claude/desktop-release-retry-shim`, `gh workflow run desktop-release.yml --repo paldyn/HanPage --ref claude/desktop-release-retry-shim` | 일회용 브랜치, run | Claude | 8번 |
| 5 | `gh run rerun <태그 run ID> --job <macOS job ID> --repo paldyn/HanPage`. 실제 DMG 오류로 실패한 실행은 `gh run rerun <run ID> --failed --repo paldyn/HanPage` | 리허설 초안의 macOS 자산 | Claude | 9번 |
| 6 | `gh pr comment 137 --repo paldyn/HanPage --body-file <파일>`, `gh pr comment 137 --repo paldyn/HanPage --body "@dependabot ignore this major version"` | PR #137 | Claude | `gh pr reopen 137 --repo paldyn/HanPage` |
| 7 | CI·리허설 통과 뒤 `gh pr merge <PR> --repo paldyn/HanPage --squash`, 기록 commit 을 `origin/devel` 에 push | `devel` | Claude | `git revert` |
| 8 | `git push origin --delete claude/desktop-release-rehearsal claude/desktop-release-retry-shim` | 브랜치 두 개 | Claude | commit 에서 다시 만든다 |
| 9 | `gh release delete hanpage-desktop-v0.8.12-rehearsal.1 --repo paldyn/HanPage --cleanup-tag`. 초안이 없으면 `git push origin --delete hanpage-desktop-v0.8.12-rehearsal.1` | 리허설 초안·태그 | 작업지시자 | 없음. 기록을 먼저 남긴다 |
| 10 | `gh release delete hanpage-desktop-v0.8.6 --repo paldyn/HanPage` | v0.8.6 초안(태그는 남김) | 작업지시자 | 없음. 노트를 먼저 옮긴다 |
| 11 | `git tag -d hanpage-desktop-v0.8.12-rehearsal.1`, 재조회 뒤 `gh issue close 172 --repo paldyn/HanPage` | 로컬 태그, 이 작업 | Claude | `gh issue reopen 172 --repo paldyn/HanPage` |

PR 은 `Refs #172` 로 둔다. 병합으로 이슈가 닫히면 9·10번이 추적에서 사라지기 때문이다. `close-issues-on-devel-push.yml` 은 PR 제목·본문과 commit 메시지를
읽으므로 그곳에 closing 키워드를 쓰지 않는다. 11번은 10번 뒤에 한다.

## 5. 검증 계획

- YAML 파싱, 검증 단계 `bash -n`, 저장소 워크플로 정책 테스트 29개, 문서 링크.
- 검증 단계 스크립트를 YAML 에서 그대로 꺼내 공개된 0.8.12 자산으로 재현한 22가지(#170 의 20가지와 만료일을 못 읽는 2가지). 인증서 경고·요약 줄·대조 줄 확인.
- 감사 스크립트: 공개된 0.8.12·0.8.11 양성, 음성 28가지(#170 의 26가지와 `--rehearsal` 2가지), 양성 대조 3가지, 인자 오류 10가지.
- 독립 반박 검토.
- PR CI.
- 실제 실행. 증적은 `mydocs/pr/assets/desktop_rehearsal_20261104/` 에 모은다(태그 commit, run URL, 스냅샷, 감사 JSON, `latest.json`, 로그 발췌). PR head 를
  움직이지 않도록 병합 뒤 기록 commit 에 넣고, 초안을 지우기 전에 작업 트리에 저장을 끝낸다.

  | 대상 | 실행 | 판정 수단 |
  |---|---|---|
  | 공개 릴리스 확인: 릴리스 없음 | 태그 run 의 두 job | `.../runs/<run ID>/attempts/1/jobs` 의 단계 결론 success |
  | 공개 릴리스 확인: 초안 있음 | macOS job 재실행 | 같은 단계 success, 로그에 `Found draft release`, 릴리스 ID 가 첫 실행과 같음 |
  | 공개 릴리스 확인의 전제 | 일회용 브랜치의 조회 단계 | job 토큰으로 `v0.8.12` 200, `v0.8.6` 404 |
  | 재시도 | 일회용 브랜치 수동 실행 | tauri-action 단계 출력에서 `Attempt 1 failed, retrying...` 1회, 그 앞에 `Copying volume icon file` 과 `rehearsal: forced SetFile failure`(대역을 만드는 단계의 원문은 제외), `Notarizing Finished with status Accepted` 2회, job 성공 |
  | 검증 단계 | 태그 run 과 재실행 | 단계 success, 단계 머리의 `REQUIRE_SIGNED: true` 와 숫자 `RELEASE_ID`, 로그에 `초안 자산 대조 통과: HanPage_` 2줄, `The validate action worked!` 3회, annotation 제목에 `Developer ID 인증서` 있음·`macOS 미서명` 없음. 요약 줄은 로그인한 브라우저에서만 보여 판정에 쓰지 않는다 |
  | 자산 교체 | 재실행 전후의 자산 id | macOS 자산 3개와 `latest.json` 은 바뀌고 Windows 자산 2개는 그대로 |
  | 초안 | 재실행이 끝난 뒤 | 같은 태그의 릴리스 1개, 자산 6개, `latest.json` 플랫폼 4개, 감사 `--release --rehearsal 1` 통과 |
  | 바깥 영향 | 4절 스냅샷 전후 | 릴리스 id·`latest.json` 해시·자산 id·`updated_at` 은 같고, 태그 목록은 리허설 태그 하나만 늘어난다 |
  | 재시도로 만든 DMG | 일회용 브랜치의 artifact | `.DS_Store`, 아이콘 좌표 180,170·480,170, `.VolumeIcon.icns`(0.8.12 와 같음) |
  | DMG 실패 단계(참고) | `dmg-probe` job | 시도·실패·같은 러너 재시도 결과 기록 |

  판독 규칙: 실제 DMG 오류로 job 이 실패하면 재시도 결함이 아니다. 그 `--verbose` 출력은 실패 단계의 첫 증거이므로 저장하고 다시 실행한다.

## 6. 리허설 뒤에도 남는 미검증

- 공개 릴리스를 막는 분기(`exit 1`)의 실제 실행. 로컬 재현과 job 토큰 조회로만 본다.
- 검증 단계의 digest 두 분기 중 돌지 않은 쪽(대조 줄로 어느 쪽인지 남는다)과 `초안 아님` 실패 분기(로컬 재현).
- 자산 업로드 재시도, Windows 의 `tauri build` 재시도.
- 실제 일시 실패가 같은 러너에서 복구되는지. `dmg-probe` 는 빌드 뒤 실행이 아니어서 재현돼도 실제 조건과 다르다. 다음 실제 실패의 `--verbose` 출력이 첫 증거다.
- 설치된 앱의 경로. 리허설은 공개하지 않으므로 지나지 않는다.
- G2 인증서로 서명한 자산의 태그 경로와 감사. 교체 뒤의 리허설에서 본다.
