# HanPage 웹 파비콘 새 로고 반영 준비

## 원인과 공개 상태

사용자가 웹 버전의 이전 파비콘을 지적했다. [공개 웹](https://hanpage.paldyn.com/)의 실제 `/favicon.ico`는 HTTP200이며 SHA256 `552ba2454b39bcbddd2d17243622ddcb3f16d05f110555e158ba6e500ee24c9f`로 현재 웹 source의 구 로고와 byte 동일하다. PWA128/192/256/512도 같은 구 로고다. 사용자 브라우저 cache는 읽거나 초기화하지 않았고 서버 자체가 이전 파일을 제공함을 확인했다. [공개 전 감사](assets/web_favicon_20261003/public-before.json).

새 Desktop 로고는 [Issue112](https://github.com/paldyn/HanPage/issues/112)의 별도 앱 배포에 포함됐다(현재 CLOSED, 참조만). Pages는 `main` push만 배포하며 Desktop 태그·devel 병합·Pages 수동 Build는 공개 웹을 갱신하지 않는다. 최근 [실제 Pages 배포](https://github.com/paldyn/HanPage/actions/runs/30916201318)는 2026-08-04 source `cc8d04812e44ce4746f6855b88db201b6a7baba3`, public gh-pages는 `d43389077593abe93f8200db2dd22c7f86afb868`이다. 현재 웹 빌드는 `assets/logo/favicon.ico`를 `rhwp-studio/public/favicon.ico`에 덮어쓰므로 두 원본을 함께 보정해야 한다.

## 로컬 후보

- HanPage fork의 이미 승인·통합한 제품 source를 유지해 base `133f5d31b88a87b46c483af883efbe1ca0298d70`에서 `codex/web-favicon-hanpage-20261003`을 준비했다. 추가 upstream 동기화·primary checkout 전환은 수행하지 않았다.
- 구현 SHA `a4ddf332745998970b1d28f690e610a1e92c340a`: 두 favicon 원본과 새 source asset을 승인한 Desktop ICO와 byte 동일하게 맞췄다. HTML이 source asset을 참조해 Vite가 내용 hash URL `/assets/hanpage-favicon-DGxGu2KG.ico`를 생성한다. 브라우저의 기존 `/favicon.ico` 장기 cache와 새 HTML의 요청 URL을 분리한다.
- PWA128/192/256/512는 `hanpage-04-크기.png`를 사용한다. 128/256/512는 승인된 Desktop PNG byte 복사,192는 기존256 PNG를 sips로 비례 축소했다. 기존 `icon-크기.png` alias도 동일 새 PNG로 맞춰 Apple touch256와 Chrome/Firefox의 명시적 복사 규칙을 유지했다. Workbox의 기존 PNG/ICO precache 규칙·autoUpdate 설정을 바꾸지 않았다.
- Rust/engine/WASM source·Desktop 버전·workflow·확장 manifest·문서 조판·test/golden/기준값은 변경하지 않았다. 기존 WASM을 재사용했으며 fresh-engine 검증으로 보고하지 않는다.

![새 웹 로고 미리보기](assets/web_favicon_20261003/new-logo-preview.png)

## 실제 검증과 한계

[검증 요약](assets/web_favicon_20261003/validation.json), [browser 결과](assets/web_favicon_20261003/browser-results.json), [Chrome/Firefox config 빌드](assets/web_favicon_20261003/extension-favicon-check.json), [자산 원본·hash](assets/web_favicon_20261003/prepared-assets.json)를 보존했다.

- `npm run build`: TypeScript·Vite·실제 PWA/Service Worker 생성 PASS. 기존 큰 JS chunk 안내 warning은 남았고 실패가 아니다.
- `npm test`: 1,834 PASS /2 SKIP /0 FAIL. 웹 source 변경 뒤 새로 실행했다.
- 실제 built app을 headless Chrome에서 열어 HTML의 hashed favicon HTTP200·Desktop ICO hash 동일, PWA5 manifest 항목의 image decode·명시된 크기·새 URL, Apple touch256의 새 Desktop PNG hash 일치를 확인했다. 실제 service worker가 제어하는 상태에서 네트워크를 끄고 6개 branding URL의 HTTP200/hash 동일을 확인했다. 미처리 page exception0이다.
- Chrome/Firefox의 실제 Vite config를 격리 output으로 각각 빌드해 새 hashed ICO 파일/참조를 확인했다. 기존 packaging의 Apple touch copy 규칙을 대조하고 동일 copy만 실행했다. 전체 확장 package/install은 미실행이다.
- primary 원래39파일은 status·bytes·SHA256와 HEAD가 동일하며 사용자 설치앱·브라우저 cache·shared target/pr-review를 바꾸지 않았다. 설치된 PWA의 icon 재수집과 사용자 browser toolbar의 시각 표시는 미검증이다. 원문 로그는 ignored `output/pr-review/web-favicon-20261003/logs`에 보관한다.

## 게시·배포 대기

2026-10-03 작업지시자가 웹 파비콘 수정의 PR·웹 배포 요청에 “응”으로 승인했다. 이 승인을 받은 뒤 일반 devel PR을 게시하고 정확한 최신 head CI와 정상 merge를 진행한다. source 구현과 로컬 검증은 완료했으며, 공개 웹 갱신은 아직 미실행이다. [내부 task 승인 규칙](../manual/codex/docs_and_git_workflow.md#internal-task-pr-approval)의 push/PR 경계를 따른다.

현재 main과 devel은 파일28,383개 차이이며 최근 engine/Studio 통합·Desktop 작업 등 미공개 변경을 포함한다. 이 favicon 후보의13파일과 별도로 전체 devel→main 웹 promotion 범위를 검토해야 한다. 승인 시 일반 devel PR의 최신 CI와 정상 병합을 먼저 처리하고, main release 후보의 정확한 차이·promotion gate를 확인한 뒤 웹 공개 파일/hashed URL·manifest·SW를 다시 검증한다. 공개 웹에서 새 로고 확인 전에는 배포 완료로 보고하지 않는다.

## PR 본문 초안

웹 서버와 Pages 빌드의 파비콘 원본이 이전 로고를 제공하던 문제를 수정합니다. 새 Desktop 로고와 같은 favicon·PWA·Apple touch 자산을 사용하고, 브라우저가 새 파비콘을 요청하도록 Vite의 내용 hash URL을 연결합니다. 기존 확장 viewer의 Apple touch 복사 경로는 유지합니다.

TypeScript/PWA 빌드, Studio 1,834 PASS /2 SKIP, 실제 browser의 새 asset decode 및6URL offline SW, Chrome/Firefox Vite config 빌드를 확인했습니다. Rust/엔진/기준값·workflow는 변경하지 않습니다. 전체 확장 설치·이미 설치된 PWA의 icon 갱신은 미검증이며 웹 공개는 별도 배포 후 확인합니다. 관련 Issue112는 참조만 합니다.
