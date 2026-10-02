# PR #114 리뷰 — HanPage Desktop0.8.7 배포 빌드 보정

## 최종 판정

**승인·병합·실제 Desktop0.8.7 공개 확인 완료.** 최신 trailing head의32checks(28성공·4정상skip), exact head 및 MERGEABLE/CLEAN을 root가 재확인해 정상 merge했고, 실제 양 플랫폼 자산·업데이트 서명·macOS 공증·새 로고를 확인한 뒤 공개했다. 설치된 사용자 앱의 업데이트 설치/재시작과 Windows GUI 실행은 수행하지 않았다.

## 접수와 라우팅

- [PR #114](https://github.com/paldyn/HanPage/pull/114), 작성자/실행자 enxec, ADMIN, base devel. author self-review이며 reviewer 지정·관리자 우회 없음.
- base d523c04493ddf8296bc5d56e2c90c38f4da4fe96, source549411762fecb5a833de3e30d3e82d3aab8319f2, 번호 발급 후보b0ff41721577f182717b4e906698d454563e6671.
- base route collaborator_self_merge.md; modifiers intake_and_review.md, review_template.md, local_validation.md, post_merge.md. 이전 #113과 같은 릴리스 작업의 실제 실패 보정으로 권위 문서를 적용했다.
- 사용자2026-10-02 “응 하자”의 실제 앱 릴리스 승인 범위. [Issue #112](https://github.com/paldyn/HanPage/issues/112)는 공개 완료 전 OPEN. 댓글·main/Pages/CLI/npm/확장·secret/권한 변경 없음.
- PR 발급 시 Open/non-draft. 현재 source와 validation을 포함하고 최신 검증은 이 self-review/오늘할일 trailing head에서 확인한다.

## 문제와 변경 범위

#113의0.8.6 초안은 양 플랫폼 모두 action의 latest 조회 fallback으로0.9.1을 설치해 --no-opt 옵션 파싱에서 실패했다. 고정 v0.13.1 공식 두 플랫폼 배포 파일과 실제 CLI 옵션을 확인했다. 정확한 tool version을 검사하고 Windows에는 기존 native PowerShell cargo.exe proxy wrapper를 선택한다. 동일 태그를 이동하지 않고 Desktop6버전 필드를0.8.7로 올린다.

Root엔진/Studio/앱 런타임·의존성·로고 바이너리는 base와 동일하다. 변경은 workflow 도구 고정·OS wrapper 분기, Desktop 버전/릴리스 설명 및 검증 기록이다. Linux native SVG 호환 PDF의 기존 문구 누락 등 #111 한계는 해결 완료로 세지 않는다. 공통 조판 원칙·Hancom 시각 비교는 renderer/layout/문서 출력 변경이 없어 비해당이다. 신규 HWP/HWPX/PDF 검증 입력 없음. 실제 로고 입력은 #113의 committed asset 및 SHA로 고정돼 있다.

## 실제 검증

[보정 실행 보고](../../report/task_m100_hp112_release_retry.md), [로컬 원장](../assets/task_m100_hp112_release/desktop-v087-local-validation.json), [Windows 계약](../assets/task_m100_hp112_release/desktop-v087-windows-wasm-contract.json)에서 command/environment/source/artifact를 확인했다. wasm-pack0.13.1 fresh locked/no-opt WASM compile 성공, 해당 fresh WASM으로 Node22.22.1 frontend/native Desktop0.8.7 compile 성공, 실제 unsigned .app bundle 성공. Info.plist0.8.7·com.paldyn.hanpage, source ICNS byte 동일, dist WASM fresh output 동일·sw.js 없음. 기존 workflow 계약9/9·YAML/diff·6버전 필드·의존성 불변 검사 통과. 같은 shared target Cargo 실행은 순차였다.

원본39파일은 primary에 hash 그대로 유지했다. 원본을 stash/ff/drop하는 동기화는 자동 승인 검토에서 유실 위험으로 거절됐고 명령은 실행되지 않았다. 우회하지 않고 격리 작업트리에서 계속했다.

## 공개·후속 조건

로컬 unsigned 번들은 macOS 코드서명/공증, Windows 실제 빌드/GUI, 설치된 앱 updater 설치 실측의 근거가 아니다. latest.json0.8.7/4플랫폼/동일Desktop tag URLs와 실제 updater payload·sig·기존 공개키, macOS codesign/Gatekeeper/stapler와 실제 아이콘을 확인한 뒤 초안을 공개한다. 기존 공개0.8.3은 그때까지 유지한다.

이 PR은 archive review/impl·오늘할일을 포함한다. postmerge duration success/갱신보류를 확인하며 검증 CI를 재실행하지 않는다. GitHub 댓글 없음. 소유 symlink/output만 정리하고 shared target·원본39파일은 보존한다. 보호된 managed worktree는 앱 archive 결과의 정확한 유지 사유를 기록한다. contributor fork remote branch는 자동 삭제 대상이 아니다.


## 실제 병합 및 배포 1차 — 외부 계약 조건 보류

2026-10-02 최종 head440d57b0616adc8a10ce4415b0383a9372064aa5의32 checks(28성공·4정상skip), Build & Test·3언어CodeQL/GHAS·전체 검증과 MERGEABLE/CLEAN을 root가 확인하고 정상 merge했다. merge는b762cbdbbe0d57b379bb8cf29393599e078c9eaa이며 synthetic/head와 tree가 동일하다. [최종 CI 원장](../assets/task_m100_hp112_release/pr114-final-ci-summary.json)에 실제10,083회귀PASS·50skip·0fail 및 나머지 lane/정체성을 고정했다. postmerge duration은 B worker의 원 측정 증거를 확인할 수 없어 자료 갱신 보류였고 검증 CI를 재실행하지 않았다.

위 merge의 hanpage-desktop-v0.8.7 tag로 [실제 Desktop 배포 실행](https://github.com/paldyn/HanPage/actions/runs/36952025097)을 진행했다. 양 플랫폼 wasm-pack0.13.1 guard와 각 OS locked wrapper WASM은 성공했다. Windows 전체 job은 성공했고 실제 설치 파일39,860,953bytes와 sig·partial latest.json을 다운로드해 API 크기·SHA와 대조했다. [업데이트 서명 검증](../assets/task_m100_hp112_release/desktop-v087-windows-signature.json)은 기존 공개키835d6b3831e133aa로 실제 payload/global signature를 검증하고 변조 대조군을 거부했다. [외부 설치 파일 아이콘](../assets/task_m100_hp112_release/desktop-v087-windows-icon.json)의7개 frame payload와 RGBA가 승인 입력과 모두 같다. Windows GUI·설치·압축 내부 앱/제거 프로그램은 실행/검사하지 않았다.

macOS native compile은3분41초로 성공하고 코드 서명 명령 뒤 공증 요청에 진입했으나 Apple서버가 HTTP403 `required agreement is missing or has expired`로 거절했다. [필요 로그 원문](../assets/task_m100_hp112_release/desktop-v087-macos-contract-excerpt.txt)과 [1차 배포 원장](../assets/task_m100_hp112_release/desktop-v087-attempt1-contract-hold.json)에 경계를 남겼다. 완성 macOS signed/notarized archive가 업로드되지 않아 codesign/Gatekeeper/stapler·ICNS 실제 번들 검증은 미검증이다. [Apple 역할 정본](https://developer.apple.com/help/account/access/roles/)에 따라 Account Holder의 계약 상태 확인/동의가 필요하며 사용자에게 요청했다. 계약 동의는 대행하거나 elapsed time으로 완료 추정하지 않는다.

0.8.7 릴리스401478979는 draft이며 Windows의3개 자산·2플랫폼 키만 존재한다. macOS 공증, 총6개 자산과4플랫폼 키의 signature/manifest 검증 전에는 공개하지 않는다. 기존 앱의 공개 latest endpoint는 [실제 익명 조회](../assets/task_m100_hp112_release/public-latest-after-v087-hold.json)에서0.8.3/4키·이전 manifest SHA 그대로다. Issue112는OPEN으로 유지했다. 계약 상태가 확인되면 동일 소스·태그의 실패한 macOS release job만 재시도한다. 이번 계정 오류를 이유로 소스·secret·권한을 바꾸거나0.8.8을 만들지 않았다.

공개 후 최종 docs/assets 운영 commit·local devel CAS·본인 issue 종료·소유 임시 산출물 정리는 아직 남아 있다. 원본39파일·기본 HEAD/index는 그대로이며 managed worktree와 소유 output은 미완료 배포 재개용으로 유지한다. 이 중간 증적은 현재 작업트리에 준비됐고 아직 최종 운영 push를 했다고 보고하지 않는다.


## Desktop0.8.7 공개 확정 — 2026-10-02

사용자가 Apple 계약 동의를 완료했다고 알려온 뒤 기존 source/tag를 유지해 실패한 macOS job만 재시도했다. [2차 실행 원장](../assets/task_m100_hp112_release/desktop-v087-attempt2-ci-audit.json)의 실제 native compile4분13초, Apple공증 Accepted2026-10-02T02:36:24Z·notary id ad78ce80-c5f2-47cd-b084-88a8855b5f55 및 [필요 로그](../assets/task_m100_hp112_release/desktop-v087-attempt2-macos-log-excerpt.txt)를 확인했다. Windows표시job110676137763은 첫 성공110666824771의 실행 시간·단계와 산출물을 재사용한다. 양 플랫폼 최종 배포 job이SUCCESS이고 head는b762cbdbbe0d57b379bb8cf29393599e078c9eaa이다. 검증 CI를 병합 뒤 다시 실행한 것은 아니다.

[HanPage Desktop0.8.7](https://github.com/paldyn/HanPage/releases/tag/hanpage-desktop-v0.8.7)을 **2026-10-02 11:39:24 KST(02:39:24 UTC)**에 공개하고 latest로 지정했다. 앱이 사용하는 익명 latest endpoint도0.8.7/4플랫폼이며 검증한 초안 manifest와 바이트가 같다. [공개 원장](../assets/task_m100_hp112_release/desktop-v087-public-verification.json), [공개 응답 원문](../assets/task_m100_hp112_release/public-latest-v087.json), [Release API](../assets/task_m100_hp112_release/desktop-v087-public-release.json)에 version/tag/source/draft=false·실제 자산 digest·공개 시각을 고정했다. manifest SHA-256은e222e1bc9252621fa941fd45e19fd7a0f97b6fab9e2667cd31569bdf367d7c06이다.

| 실제 배포 검증 | 판정과 permanent evidence |
| --- | --- |
|6개 자산의 다운로드 크기·SHA-256/API digest|[초안 inventory](../assets/task_m100_hp112_release/desktop-v087-draft-inventory.json)와 공개 API 모두 동일·PASS|
|macOS/Windows updater signature 및 변조 대조군|[기존 공개키 검증](../assets/task_m100_hp112_release/desktop-v087-updater-signatures.json), 양 payload/global signature PASS·한 바이트 변조 REJECTED, key id835d6b3831e133aa|
|실제 Mac app 버전/아키텍처/새 ICNS|[실제 번들](../assets/task_m100_hp112_release/desktop-v087-macos-bundle.json),0.8.7·com.paldyn.hanpage·arm64, source ICNS와 바이트 동일·SHA8260b6100180a8c589c5b63943d77587fc575bbc74d591d3249557a6df8d70c4|
|Mac codesign·Gatekeeper·stapler|동일 원장에4검사 exit0·Notarized Developer ID·stapled ticket·기존 공개 앱과 같은 TeamIdentifier8L78W6D8XF. [기존 공개 패키지 대조군](../assets/task_m100_hp112_release/macos-v083-validation-control.json)도 별도로PASS|
|Mac DMG 무결성|[hdiutil 원문](../assets/task_m100_hp112_release/desktop-v087-dmg-verify.txt), checksum VALID|
|실제 Windows 외부 NSIS icon|[PE resource](../assets/task_m100_hp112_release/desktop-v087-windows-icon.json),7/7 encoded payload 및 RGBA가 새 source ICO와 일치|
|공개 후 실행 경계|[실제 run 목록](../assets/task_m100_hp112_release/desktop-v087-postpublication-boundary.json),Desktop/정상 postmerge 운영만 존재, npm·CLI·Pages·검증 CI release event 실행 없음|

source.ICNS의1024px preview를 직접 열어 파란 바탕·흰 한글 심볼을 확인했다. 실제 Mac ICNS가 해당 source와 바이트 동일하므로 동일 로고다. 이 검증은 사용자 설치 앱의 Dock cache·업데이트 UI·실행 실측을 대신하지 않는다. Windows설치/GUI·압축 내부 executable/uninstaller 아이콘은 미검증으로 구분한다. source/runtime·engine0.8.6·Studio·의존성과 기존 Linux native SVG 호환 PDF 한계는 이번 환경 재시도에서 바뀌지 않았다.0.8.6 실패 tag/draft는 보존했다.

### 이 운영 commit 뒤의 후속 순서

archive review/impl·기존 오늘할일·필요한 assets만 한 운영 commit으로 origin(paldyn/HanPage)/devel에 반영한다. 현재 Issue112는2026-10-02T02:41Z 조회에서OPEN이며, push와 local devel CAS 완료 뒤 댓글 없이 종료할 예정이다. local devel은 어느 worktree에서도 checkout되지 않은 ref만 조상/예상 SHA를 확인해 CAS한다. 현재 primary branch/HEAD/index/39개 파일은 switch/reset/stash로 바꾸지 않는다. [공개 후 원본 보존](../assets/task_m100_hp112_release/primary-preservation-after-v087-publication.json)은39개 byte/hash 모두 동일하다. 이전 stash/ff/drop 거절을 우회하지 않았다.

실제 issue 종료와 CAS·cleanup 결과는 이 commit 이후 최종 응답에서 보고한다. permanent evidence의 원격 반영 뒤 소유 pkg/node_modules symlink·hp112 임시 output·사용하지 않는 소유 local branch만 정리한다. managed worktree는 앱 archive를 시도하고 pinned/workspace 보호가 계속되면 정확한 경로와 이유를 보고한다. 공유 target/pr-review·primary 원본·fork remote head는 보존한다. source/report/plan/test/workflow/golden 변경, GitHub 댓글, secret/권한 변경은 이 운영 commit에 포함하지 않는다.
