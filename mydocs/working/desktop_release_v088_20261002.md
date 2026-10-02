# HanPage Desktop 0.8.8 배포 후보 로컬 검증

- 사용자 승인: 2026-10-02 “응 배포하자”. 수정 UI의 PR·검증·병합·0.8.8 배포를 진행한다.
- 관련 [Issue #59](https://github.com/paldyn/HanPage/issues/59)의 후속 개선이다. 신규 공개 Issue·담당자 지정은 자동 승인 검토에서 배포와 별도 게시 승인 부족으로 거절되어 수행하지 않았다.
- source 후보 `9d8d93cdcc8baba3c9ad47d25050df819b9feeca`, fork의 실제 PR base `origin/devel`=`71f327f2ffdbc20bc3b8926d136ead049caeeb68`. 최신 원본 upstream 조회는 수행했으나 이번 UI 배포에 새 엔진 변경을 섞지 않았다.
- route: `collaborator_self_merge.md`; modifiers `intake_and_review.md`, `review_template.md`, `local_validation.md`, `review_only_fast_pass.md`, `post_merge.md`, `rework_and_exceptions.md`(대형 증적 diff). 기본·보조 문서를 읽고 self-review·독립 agent 검토를 수행했다. reviewer 지정·관리자 우회 없음.

## 변경과 검토

업데이트 카드·진행 상태·재진입은 [구현 검증](desktop_update_ui_20261002.md), 문구·문장 끝 개행은 [후속 검증](desktop_update_ui_copy_20261002.md)에 연결한다. 최종 Native 확인·청크 누산·서명 검증 전후 상태·원자적 적용·worker 오류 복구, 저장 확인·입력 잠금·늦은 이벤트 방어의 실제 호출 경로를 독립 검토했고 배포 차단 결함을 발견하지 않았다. 기존 macOS 메뉴·문서 저장 확인은 같은 경로를 사용하며 기능 중복·충돌을 발견하지 않았다.

이번 버전 준비는 Desktop 5파일의 6필드를 0.8.8로 맞추고 Desktop README/CHANGELOG를 갱신했다. root 엔진·Studio·npm 버전 및 의존성 해결은 base와 바이트 동일하다. 우리 소유 원문·압축 로그36개는 ignored output에 hash 동일 사본을 보존하고 PR tree에서 제외했다. PNG·의미 결과·source hash는 유지했다.

## 검증 결과

| 대상 | 실제 결과와 한계 |
| --- | --- |
| 기존 Studio `npm test` | 1,834 PASS / 2 SKIP / 0 FAIL. 최종 Studio source와 동일한 copy50f9에서 완료한 근거 재사용 |
| 최종 후보 실제 브라우저 | 55 PASS / 0 FAIL, PNG12장 재생성. Tauri IPC·이벤트만 대역이고 실제 Studio/WASM·입력·저장 경로 사용 |
| Desktop fmt·Clippy all-targets | 0.8.8 후보에서 PASS |
| 실제 로컬 앱 bundle | `tauri build --bundles app --no-sign` 성공. CLI config로 updater artifact만 비활성화한 unsigned 로컬 패키지. 두 plist0.8.8·identifier·source ICNS 일치·fresh WASM 일치·SW없음 확인 |
| Native 상태 회귀7개·Rust lint 묶음 | 이전1399 근거와 Native/test/policy Rust source hash가 같아 재사용. Root native/WASM32/workspace Clippy 및 build PASS. 호스트의 nextest 부재로 이전 상태7개는 focused cargo test이며 전체 nextest 결과로 세지 않음 |
| 조판 원칙·문서 Visual Sweep | 비해당. 엔진/페이지/표/기준값 미변경. 앱 카드 밝은/다크390px·Windows 안내·적용 막대 직접 판독 |
| 검증 입력 commit | 파일 없는 빈 문서를 실제 입력·저장한 브라우저 검사. 신규 HWP/HWPX/PDF fixture 미사용으로 별도 입력 commit 비해당 |
| 원본 사용자 변경 | primary HEAD와 원래39개 파일 byte/hash 동일. switch/reset/stash 없음 |

- [후보 source·결과·hash](../pr/assets/desktop_v088_release/local-validation.json)
- [55개 실제 브라우저 결과](../pr/assets/desktop_v088_release/browser-results.json)
- [실제 unsigned 로컬 bundle](../pr/assets/desktop_v088_release/local-bundle.json)
- [primary 보존](../pr/assets/desktop_v088_release/primary-preservation.json)

![최종 후보 업데이트 카드](../pr/assets/desktop_v088_release/ready-card.png)

![최종 후보 적용 중 카드](../pr/assets/desktop_v088_release/applying-card.png)

카드의0.8.7→0.8.8은 상태 이벤트 대역의 검사 입력이다. 실제 설치된0.8.7에서 이번0.8.8을 받는 첫 알림은 기존 화면이며 새 UI는0.8.8 설치 이후부터 적용된다. 사용자 설치 앱을 실행·교체·재시작하거나 Windows GUI를 실행하지 않았다.

## 배포 gate와 rollback

완료된 후보를 devel PR로 게시한 뒤 최신 code/문서 trailing head의 CI와 mergeability를 확인한다. 정상 병합한 exact SHA에 `hanpage-desktop-v0.8.8`을 생성하고 양 플랫폼 초안 빌드를 진행한다. 총6asset·4platform manifest·기존 공개키 updater signature와 변조 거부·Mac codesign/Gatekeeper/stapler·실제 로고·DMG를 검증한 뒤 공개한다. 공개 endpoint는 완료 전 기존0.8.7을 유지한다.

부분 실패는 동일 source/tag의 실패 job만 복구하며 기존 공개 버전을 유지한다. 태그를 이동하거나 secret/권한·main·CLI/npm/Pages를 변경하지 않는다. 공개 후 회귀가 확인되면 0.8.8의 latest 노출을 거두고 이전0.8.7 latest를 복구하거나 후속 patch를 검토한다. 실제 원격 조치는 사용자 배포 승인 범위에서 필요 시 수행한다.


## 공개 완료

PR [#115](https://github.com/paldyn/HanPage/pull/115)는 최신 trailing 검사 통과 뒤 `27c0632e59241828f8b274998320b5aadc4ef743`로 정상 병합했다. source tree는 검증한 head와 동일하다. Desktop Release36966794272의 Mac·Windows job 모두 성공했고, 태그와 Release source도 같은 SHA다.

Mac app archive/DMG의 버전·arm64·source 로고·Developer ID 팀8L78W6D8XF·codesign·Gatekeeper·stapler, Windows 설치 EXE 외부 아이콘 전체 7프레임, 실제 6개 자산의 크기·API digest를 확인했다. 기존 공개키로 두 updater 서명과 변조 거부를 확인한 뒤 2026-10-02 14:19:19 KST에 0.8.8을 공개했다.

[공개 Release](https://github.com/paldyn/HanPage/releases/tag/hanpage-desktop-v0.8.8)와 [공개 endpoint 검증](../pr/assets/desktop_v088_release/desktop-v088-public-verification.json)을 연결한다. 자세한 gate·한계·운영 후속은 [PR115 최종 검토](../pr/archives/pr_115_review.md)에 통합했다. 사용자 앱을 설치·재시작하지 않았다.
