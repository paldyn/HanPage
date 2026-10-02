# PR #113 리뷰 — HanPage Desktop 0.8.6 새 로고·릴리스 준비

## 최종 판정

**승인 — 로컬 검증 범위의 Desktop 패키징 후보를 수용한다.** 실제 최신 PR head CI 성공과 merge 직전 exact head·MERGEABLE/CLEAN 재확인이 병합 전 조건이다. 앱 릴리스는 양 플랫폼 draft 자산·서명·공증 검증 뒤에만 공개한다.

## 접수 정보와 라우팅

- [PR #113](https://github.com/paldyn/HanPage/pull/113), 작성자/실행자 enxec, base devel.
- 기준 `c5b6022532371fd07a2f5e978fb21e4c3cc37758`; source `0ac1f1bd784a59eacd6bafab30d4d522211678a5`; 번호 발급 후보 `0e8ad16c83fe2986bbc43aa6d0fc981362ae3d51`.
- 관련 Issue [#112](https://github.com/paldyn/HanPage/issues/112), 공개 완료 전 close하지 않는다.
- ADMIN 실행자의 author self-review, reviewer assign 없음, 관리자 우회 없음.
- base route: collaborator_self_merge.md; modifiers: intake_and_review.md, review_template.md, local_validation.md, post_merge.md.
- 2026-10-02 사용자 “응 하자” 승인: 새 로고·통합 기능을 포함한 앱 릴리스에 필요한 준비 PR·merge·Desktop tag·공개. 댓글·무관한 배포·secret/권한 변경은 수행하지 않는다.
- 번호 발급 시 Open/MERGEABLE, CI 진행 중. 최신 CI는 trailing 기록 head에서 확인한다.

## 변경과 검증

[실행 보고](../../report/task_m100_hp112_report.md)와 [로컬 검증 원장](../assets/task_m100_hp112_release/local-validation.json)을 확인했다. 로고 입력/원본팩/백업은 commit에 보존했고 새 .app의 Info.plist 0.8.6, 동일 ICNS, SW 미포함·검증 WASM 동일을 확인했다. 실제 locked Desktop release 컴파일 및 unsigned 로컬 .app bundle 성공. workflow 기존 계약9/9·YAML/diff 검사 통과.

Desktop version/lock6필드, 새 로고·NSIS 설정, 재현 빌드·draft rollout만 변경했다. 엔진/Studio/Rust production source·test·baseline·sample은 기준과 byte 동일하다. #111의 검증과 알려진 한계를 유지하며 이번 변경에 관계없는 엔진 전체 로컬 회귀를 반복하지 않았다. 배포 CI는 fresh WASM과 실제 서명된 두 플랫폼 artifact를 만든다.

공통 조판 원칙: **비해당** — renderer/layout/paint 변경 및 문서 출력 개선 주장이 없다. 새 HWP/HWPX/PDF 입력을 쓰지 않은 패키징 검증이므로 검증 입력 커밋 항목도 문서 fixture에는 비해당이다. 실제 로고/구성 입력의 경로·SHA는 원장에 있고 commit blob과 실행 파일을 대조했다.

## 한계와 배포 조건

로컬 앱은 unsigned 테스트 번들이며 Gatekeeper·실제 updater 설치·Windows GUI를 검증했다고 주장하지 않는다. 양 플랫폼 draft 릴리스에서 latest.json version/4플랫폼/동일tag URL과 실제 updater payload·sig·기존 공개키 검증, macOS codesign/Gatekeeper/stapler를 확인한 뒤 공개한다. 기존 공개0.8.3은 초안 동안 유지한다. Desktop tag는 검증된 devel commit에서 만들며 main/Pages/CLI/npm/확장 배포는 포함하지 않는다.

## 후속 처리

본 PR이 review/impl·오늘할일을 포함한다. 병합 후 실제 merge SHA와 CI·릴리스 run·공개 검증은 운영 근거/최종 보고에 추가한다. contributor comment는 요청되지 않아 게시하지 않는다. 전용 작업트리는 앱 pin/workspace 보호가 있으면 사유를 남기고 유지한다. 사용자 primary의 원본은 hash 보존한다.

## 실제 병합과 첫 배포 결과

최종 head1afd7a501c4b5485e5e6fe96a327419d283be282/basec5b6022532371fd07a2f5e978fb21e4c3cc37758의32checks가28SUCCESS·4정상SKIPPED, 실패/대기0임을 root가 CLEAN/MERGEABLE과 함께 재확인했다. 관리자 우회 없이 d523c04493ddf8296bc5d56e2c90c38f4da4fe96으로 병합했다. merge parents 및 tree가 검증 후보와 동일하다. Rust 회귀10083PASS·50skip·0FAIL, Studio1830PASS·2skip, responsive2666PASS·0FAIL. [최종 CI 근거](../assets/task_m100_hp112_release/pr113-final-ci-summary.json).

Desktop0.8.6 [첫 tag run](https://github.com/paldyn/HanPage/actions/runs/36948944525)은 양 플랫폼 모두 WASM 옵션 파싱에서 실패했다. action의 latest 조회 fallback이0.9.1을 설치했고 --no-opt를 지원하지 않았다. 컴파일/서명/자산 업로드 이전의 실패이며 초안 자산0·공개0.8.3 유지 상태를 확인했다. [실패 근거](../assets/task_m100_hp112_release/desktop-v086-failed-release.json). 원 태그는 옮기지 않고 후속 준비 PR에서 tool pin/Windows native wrapper 및 Desktop0.8.7을 검증한다.

Duration metadata run36948892747는 success이나 original measured worker unavailable:b로 no-verified-pr-duration-measurements 갱신 보류를 확인했다. 병합 후 검증 CI를 다시 실행하지 않았다. 원 작업공간의39파일은 그대로 보존하며 stash/ff/drop 동기화는 자동 승인 검토의 원본 유실 위험 판단으로 실행되지 않았다. 댓글 게시 없음. Issue #112는 실제 앱 공개 완료까지 OPEN이다.
