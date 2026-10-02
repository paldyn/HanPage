# HanPage Desktop 0.8.9 배포 검증

- 사용자 승인: 2026-10-02 “응 배포하자”. 수정 UI의 PR·정상 병합·Desktop 배포를 진행한다.
- 관련 [Issue #59](https://github.com/paldyn/HanPage/issues/59) 참조만. 기존 Issue #112와 별도이며 새 공개 이슈·댓글을 만들지 않는다.
- 제품·버전 후보 `fd1034984020684e5238c25599fc647f051b1f50`, 실제 fork base `origin/devel`=`52aeb4be36728109758c56b749c7946122f04d29`.
- route: collaborator self-merge; intake/review template/local validation/review-only fast-pass/post-merge/rework(대형 증적 diff), GitHub 운영·배포·Desktop guide를 읽었다. source PR→CI→review trailing→정상 merge→exact desktop tag→draft artifact gate→공개 순서다. 관리자 우회·main/CLI/npm/Pages 배포 없음.

## 변경과 검토

[UI 구현·107개 검사](desktop_update_quiet_brand_20261002.md)에 연결한다. 백그라운드는 하단 상태만 표시하고 버튼·파일 메뉴·macOS 메뉴에서 상세를 연다. 가운데 카드·테마 표면 혼합·최신 `닫기`/`다시 확인`·준비 `나중에`를 구현했다. 제품 정보와 Desktop authors/publisher는 PALDYN이며 MIT/Edward Kim·기술 URL과 식별자는 유지했다.

독립 agent가 상태 snapshot/event 경합, 수동 command canExecute, 저장의 saved 결과 후 applying/inert/input guard/IPC, 오류 복구, 테마 fallback과 dvh 높이를 실제 호출 경로로 검토했고 배포 차단 결함을 발견하지 않았다. Native Rust·engine·WASM source·키·아이콘·의존성은 base와 동일하며 Desktop 5파일의 6버전 필드만0.8.9로 맞췄다.

## 실제 검증과 한계

| 대상 | 결과와 범위 |
| --- | --- |
| 기존 npm test·TypeScript/Vite | 1,834 PASS /2 SKIP /0 FAIL의 최종 source 동일 근거 재사용. 0.8.9 Desktop bundle beforeBuildCommand의 TypeScript/Vite도 PASS |
| 기존 실제 브라우저 | 정상 원본 runner107 PASS,6테마·4크기·색상 대비·최신 닫기·저장·입력 보호; 현 제품8파일+E2E1파일 hash동일 |
| 0.8.9 추가 브라우저 | 동일 CDN WOFF bytes 로컬 전달 wrapper107 PASS /0 FAIL /예외0, PNG25. 실제 Studio/WASM/편집/저장, Tauri IPC·상태 이벤트만 대역. [결과](../pr/assets/desktop_v089_release/browser-results.json) |
| 초기 재실행2회 | CDN FontFace5000ms timeout 이후 canvas 준비 대기로 setup실패. 조건 완화·제품 수정 없이 원본 폰트 전달만 분리했다. [진단·font hash·wrapper](../pr/assets/desktop_v089_release/font-delivery-diagnostic.json). CDN 지연/fallback 자체는 미검증 |
| Desktop fmt/Clippy all-targets | 0.8.9 후보 PASS. Root/Rust test source 변경 없어 root lint 묶음 재실행 비해당 |
| 실제 unsigned 로컬 app | 두plist0.8.9/arm64/ID/sourceICNS/같은 WASM/JS/SW제외 PASS; CLI-only updater artifacts off. [bundle](../pr/assets/desktop_v089_release/local-bundle.json) |
| 사용자 작업 | primary HEAD·원래39파일 hash동일; checkout전환/reset/stash·앱 설치/재시작 없음 |
| 조판·문서 Visual Sweep·fixturecommit | 엔진/페이지/표/기준값 미변경, 실제 빈 문서 입력으로 별도 HWP/HWPX/PDF fixture 미사용: 비해당 |

새 wrapper 화면의0.8.7/0.8.8은 대역 상태 입력이다. 실제 설치 앱의 첫 업데이트 안내는 그 앱의 기존 UI이며 새 UI는0.8.9 설치 후 적용된다. Native GUI/About·Windows 설치·구형 WebView fallback은 미실행이다. fmt·clippy·bundle·browser 원문 로그는 ignored `output/pr-review/desktop089/logs`에 보관한다.

![최신 안내](../pr/assets/desktop_v089_release/latest-light-card.png)
![준비 안내](../pr/assets/desktop_v089_release/ready-card.png)

## 공개 gate와 rollback

최신 source/review trailing PR CI·mergeability 확인 후 exact mergeSHA에 `hanpage-desktop-v0.8.9`를 생성한다. Mac/Windows build 성공·실제6asset/API digest·4platform manifest·기존 키 payload/global서명/변조거부·Mac codesign/Gatekeeper/stapler/DMG·source아이콘을 확인한 뒤 초안을 공개한다. 공개 전0.8.8 latest를 유지한다. 실패 시 기존 tag를 이동하거나 secret/권한을 바꾸지 않고 동일 source 실패 job만 복구한다. 공개 후 회귀 시0.8.8 latest 복구나 후속patch를 검토한다.
