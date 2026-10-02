# PR #114 리뷰 — HanPage Desktop0.8.7 배포 빌드 보정

## 최종 판정

**승인 — 로컬 검증을 통과한 실제 배포 실패 보정을 수용한다.** 병합은 최신 trailing head의 모든 GitHub checks 성공/정상skip, exact head 및 MERGEABLE/CLEAN을 root가 재확인한 뒤 정상 merge로 수행한다. 실제 앱 공개는 양 플랫폼 초안 자산·서명·macOS 공증 확인 뒤 별도 실행한다.

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
