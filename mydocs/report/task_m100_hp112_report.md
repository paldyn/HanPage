# Task #112 — HanPage Desktop 0.8.6 릴리스 준비 결과

- Issue: [#112](https://github.com/paldyn/HanPage/issues/112)
- source SHA: `0ac1f1bd784a59eacd6bafab30d4d522211678a5`; 기준 origin/devel `c5b6022532371fd07a2f5e978fb21e4c3cc37758`.
- 사용자 승인: 2026-10-02 “응 하자”. 코드·검증·PR·merge·Desktop 태그·실제 배포는 목표의 필요한 단계로 수행한다.
- 상태: 로컬 후보 검증 완료. 최신 PR CI·병합·양 플랫폼 signed build·초안 자산 검증·공개는 다음 단계이다.

## 변경과 범위

Desktop와 독립 npm/Cargo lock의 6개 version 필드를 0.8.6으로 통일했다. 현재 package-lock의 0.8.2 불일치도 해소했다. 의존성 버전, 기존 updater 공개키·endpoint·identifier는 유지했다.

현재 승인된 새 로고 39파일을 hash 고정해 전용 검토 작업트리에서 복사했다. PNG 31개(원본 포함)는 정상 RGBA·명명 해상도, ICO 각 7프레임·ICNS 각 8프레임을 확인했다. 활성 ICO는 공급 원본과 동일 픽셀이고 Tauri가 읽는 첫 항목만 256px로 정렬했다. root도 실제 icon.png를 열어 파란 배경의 흰 한글 심볼을 직접 판독했다. macOS app/Dock과 Windows NSIS 설치·삭제 icon을 연결했다.

이전 로고 ZIP과 공급 원본팩은 저장소의 복구 자료로 보존한다. `bundle.resources` 설정이 없으므로 이 자료가 앱 자원으로 자동 첨부되지 않는다. 사용자 원 작업공간의 39파일은 그대로 두었고 hash 동일성을 확인했다.

Desktop workflow는 Node22.22.1·양쪽 npm ci·locked WASM wrapper(`--no-opt`)·Tauri locked cargo로 고정했다. 새 릴리스는 draft로 빌드하며 두 플랫폼 manifest 병합이 원자적이지 않은 점을 고려해 공개 전에 플랫폼과 서명을 직접 검증한다.

## 실행한 검증

- `npm run build -- --no-bundle --ci -- --locked`: Desktop frontend tsc/Vite와 실제 release 네이티브 compile 종료 0. 공유 target 재사용, Cargo jobs6.
- `tauri bundle --bundles app --no-sign --ci --config '{"bundle":{"createUpdaterArtifacts":false}}'`: 로컬 테스트 .app 종료 0. 이 번들은 unsigned이며 실제 signed 릴리스와 구분한다.
- 실제 Info.plist 버전0.8.6/identifier 확인. 번들 ICNS는 입력과 byte 동일(SHA `8260b6100180a8c589c5b63943d77587fc575bbc74d591d3249557a6df8d70c4`).
- Desktop dist에는 sw.js가 없고 WASM은 #111에서 검증한 source와 동일한 package(SHA `5c7a9a16c22cc6e0086d14d38df0386c5e63a1c75fe194a49848111c95011d12`)다. 실제 릴리스 CI는 WASM을 다시 locked build한다.
- 기존 release-channel policy/workflow wiring 테스트 9/9 통과. Ruby YAML parse·draft/locked/Node 계약 확인. diff check 통과.
- 초기 로컬 기본 Node20.11.1은 Vite의 styleText import에 실패했고, 지원되는22.22.1로 환경을 맞춰 성공했다. Python3.9의 tomllib 및 bundled Python의 yaml import 실패는 제품 실패로 세지 않았으며 각각3.13 unittest/Ruby YAML로 다시 검증했다. 원 로그를 보존했다.

근거: [로컬 검증 원장](../pr/assets/task_m100_hp112_release/local-validation.json).

## 공통 조판/입력 검토

원칙 준수 검토: 비해당. 엔진·renderer/layout/Studio source 및 golden/sample 변경이 없다. #111의 동일 엔진/Studio 회귀·시각 검증과 남은 한계를 연결한다. 이번 변경으로 새 문서 렌더링 개선을 주장하지 않는다. HWP/HWPX/PDF 입력은 이 로고·패키징 검증에서 새로 사용하지 않았다.

## 다음 단계와 한계

최신 PR CI 통과 및 exact head 병합 후 해당 commit에 `hanpage-desktop-v0.8.6` 태그를 push한다. macOS aarch64/Windows x64 설치파일·updater payload·sig·latest.json 및 macOS 서명/공증을 검증한 뒤 초안을 공개한다. main 승격/Pages/CLI/npm/확장 배포와 secret 변경은 범위 밖이다.

실제 OS updater 설치, Windows GUI 실행은 아직 수행하지 않았다. 알려진 Linux CLI SVG 호환 PDF 문구 누락은 [#111 검토](../pr/archives/pr_111_review.md)의 별도 한계이고 이 작업의 Desktop 인쇄와 같은 경로라고 주장하지 않는다. 공개 이전에는 기존0.8.3 latest를 유지한다.
