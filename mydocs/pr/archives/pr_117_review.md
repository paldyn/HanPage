# PR #117 리뷰 — 새 웹 파비콘과 PWA 로고

## 최종 판정

**정상 병합 및 웹 공개 완료.** 구현·독립호출경로 검토·로컬 실제검증과 exact code candidate의 전체CI가 통과했다. source변경없는최신문서trailing head의 CI·MERGEABLE/CLEAN·exacthead를확인한뒤 정상병합한다. 사용자가2026-10-03 “응”으로 웹 파비콘 PR·웹 배포를 승인했다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#117](https://github.com/paldyn/HanPage/pull/117) / enxec / paldyn/HanPage devel |
| 고정 base | `133f5d31b88a87b46c483af883efbe1ca0298d70` |
| 제품 구현 / remote code candidate | `a4ddf332745998970b1d28f690e610a1e92c340a` / `f2616ba9eaa48f62923d94b3edea005fafff059f`(source동일, 증적·승인 문서 추가) |
| 관련 이슈 | [#112](https://github.com/paldyn/HanPage/issues/112) 참조만, CLOSED 유지 |
| 경로 | collaborator self-merge, reviewer 지정·관리자 우회·별도 self-comment 없음 |

## 변경과 검토 범위

Pages `Copy favicon`은 rootICO를 publicICO에 복사한다. 두 원본 및 신규 source ICO가 동일 승인 Desktop 로고를 제공한다. HTML의 source asset import는 Vite가 내용 hash URL을 생성하므로 기존 `/favicon.ico` cache와 요청 URL이 구분된다. PWA5개 manifest 경로는 versioned128/192/256/512 PNG를 사용하고, legacy4PNG alias는 같은 새 파일로 맞춰 Apple touch·확장 packaging의 기존256 복사 경로를 유지한다. 기존 Workbox PNG/ICO precache와autoUpdate는 변하지 않는다.

13개 제품파일 외 변경은 증적·보고서·review·오늘할일이다. Rust/engine·Desktop버전·워크플로·확장 manifest·test/golden/baseline은 base와 동일하다. 대형 증적 추가에 대해 별도 agent가 실제 extension config 빌드와 호출·copy 경로를 검토했고, release-scope agent가 공개 source와 브랜치/배포 계약을 독립 감사했다. renderer 공통 원칙·한컴 fixture·Visual Sweep은 비해당이다.

## 검증 입력과 결과

[원인·로컬 검증](../../working/web_favicon_hanpage_20261003.md), [실제 browser](../../working/assets/web_favicon_20261003/browser-results.json), [확장 config](../../working/assets/web_favicon_20261003/extension-favicon-check.json), [자산 원본과 hash](../../working/assets/web_favicon_20261003/prepared-assets.json)를 확인했다.

- Studio `npm run build`: TypeScript/Vite/PWA 생성 PASS. 기존 큰 chunk warning은 남았다.
- `npm test`: 1,834 PASS /2 SKIP /0 FAIL. 이 source에서 새 실행했다.
- 실제 built app Chrome: hashed favicon HTTP200 및 DesktopICO SHA256동일; manifest5항목 decode·명시크기·새URL; Apple touch256 동일 hash; 실제 SW제어 후6URL offline200 및builtasset동일 hash; page exception0.
- Chrome/Firefox 실제 Vite config 별도 빌드 PASS. publicDir:false에서도 source hashedICO가 출력되고 existingApple touch 명시copy는 정상이다. 전체 확장 설치/package는 미실행이다.
- 기존 compiled WASM을 재사용했으며 fresh-engine 증거로 세지 않는다. 이미 설치된 PWA의 OS 아이콘 재수집·사용자 browser toolbar 시각·문서편집 runtime은 이 favicon 검증 범위 밖이다.

## 실제 CI

exact `f2616ba9`의 [codeCI감사](../assets/web_favicon_20261003/ci-117.json)에서5workflow전부SUCCESS·32checks완료/0실패/0대기를확인했다. CI37098971448/CodeQL37098971468/Render37098971296/Adapter37098971456/Proptest37098971509이다. trustedPRbase classifier의 `fail-closed:unclassified-path`와 `no-green-build-candidate`는FULL을선택했고실제testedmerge는 `c46882b9fad454df2a620273d67e3131997e0c77`(headf261+base133)이다. Build&Test가archive/shard총10090일치·A3873/B2053/C2005/D2159 PASS/0FAIL과Lint/NativeSkia/Frontend완료를검사했다. 실제Studio1834PASS/2SKIP·responsive2666PASS·freshWASM/packagebuild·확장distribution·Adapter7PASS도확인했다. CodeQL3언어최종SUCCESS이며RenderCanvas3case/PDFdirect3pages/0fail은통과했다. report4pages/4warnings/0errors는기존남은차이로보존하고한컴전체시각일치로세지않는다. trailing은source/test/workflow/fixture없이single-parent문서만추가하며최신head CI를별도확인한다.

## 웹 공개 범위와 후속 처리

이 devel PR merge만으로 웹은 배포되지 않는다. [배포 범위 감사](../assets/web_favicon_20261003/release-scope.json)에서 전체 devel→main promotion은 main ancestry·fork repository/actor·workflow inventory16위반 때문에 차단됨을 확인했다. 아직 실행하지 않은 promotion을 통과로 기록하지 않는다. 현재 웹 파비콘 PR·배포에 대한 사용자승인이 로고13파일을 공개하는 동일범위를 포함한다. main기반 로고전용 release후보를 독립로컬검증하고 devel정상통합후 동일제품변경을 별도 main PR/CI/정상merge/자동Pages로 공개한다. 전체promotion gate차단은 그대로보존하고 workflow를승격하거나 검증/권한/정책을바꾸지않는다. 기존mainPages 수동dispatch도 실제게시하므로 사전검증용으로 쓰지 않는다.

공개 웹 actualICO/HTMLhashURL/PWA/SW 확인 전에는 완료로 보고하지 않는다. selfPR·참조만인CLOSED이슈에 별도댓글/close는 추가하지 않는다. merge 뒤 정확한SHA·Issue112상태·duration refresh 성공/증거부족 보류를 확인하고 운영기록만 보완한다. primary39dirty파일/HEAD·설치앱·브라우저cache·sharedtarget/pr-review·forkremotebranch를 보존한다. 소유ignoredoutput은 증적을 영구보존한 뒤 정리하고 실제archive가 보호를 반환하면 원인을 기록하고 보호를우회하지않는다.

본인전용headless profile에서 [기존공개SW](../assets/web_favicon_20261003/public-browser-before.json)의oldfavicon/cache를실제설치해5검사를PASS했고page/request/consoleerror0이었다. [PR118](https://github.com/paldyn/HanPage/pull/118)은main로고전용독립후보이며fresh0.15WASM/Node20build/765tests/최종browser6offline검증을완료하고정상CI중이다. 사용자cache는건드리지않았고이기존본인profile은공개후upgrade확인에쓴다.

## 2026-10-03 병합·실제 공개 최종 확인

사용자 승인 범위의 로고 13파일을 정상 PR로 통합했다. PR117은 devel `343fde08e1aee07009f353eae8086432997401cc`, PR118은 main `6dc1a2d55cfb0752b4688d8f303a6f802cf49b5d`로 squash 병합했다. 각각 최종 검토 head와 merge의 전체 tree가 동일하다. [117 후행 검증](../assets/web_favicon_20261003/ci-117-tail.json), [117 후속 운영 실행](../assets/web_favicon_20261003/postmerge-117.json), [118 후행 검증](../assets/web_favicon_main_20261003/ci-118-tail.json), [118 실제 게시 연결](../assets/web_favicon_main_20261003/postmerge-118.json)에 정확한 SHA와 실행 결과를 보존했다.

자동 [Pages 배포 37100934978](https://github.com/paldyn/HanPage/actions/runs/37100934978)는 main merge 6dc1a2d에서 SUCCESS였다. 게시 commit `bcc34ff6626ea2261e9450cbae9adb4892a24308`의 Pages build1256683413 및 deployment6823952278은 **2026-10-03 14:52:36 KST에 성공**했다. [공개 HTTP 검사](../assets/web_favicon_20261003/public-http-after.json)는 실제 HTML의 `/assets/hanpage-favicon-DGxGu2KG.ico`, fallbackICO, manifest5항목, versioned/legacyPNG8개, Apple touch256, SW의 새 precache를 13응답의 hash·크기로 확인했다.

[기존 SW가 있던 동일 전용 프로필](../assets/web_favicon_20261003/public-browser-after.json)의 9검사가 PASS였다. 첫 방문에는 기존 SW가 이전 HTML을 제공했고 45초 안에 화면이 자동 갱신되지는 않았다. **일반 새로고침 1회 후** 새 hashed favicon·PWA5항목·Apple touch·fallbackICO가 반영됐으며, 갱신된 SW의 오프라인 branding7URL도 모두 HTTP200/새 hash였다. page/request/console 오류는 각각0이었다. 이는 사용자의 실제 browser toolbar 또는 OS에 설치된 PWA 아이콘을 직접 확인한 증거로 확대하지 않는다.

기존 main push workflow가 시작한 추가 CI/CodeQL은 중복 자동 실행으로 상태만 보존했다. 병합 전 exact product tree의 FULL CI·CodeQL·Render와 최신 문서 후행 검증이 모두 성공했고 merge tree도 동등하다. 추가 자동 실행의 진행 중 항목을 성공으로 세지 않았으며 수동 dispatch/rerun·검증 정책 변경은 없다. 참조 Issue112는 CLOSED/2026-10-02T02:44:40Z를 재확인했으며 댓글이나 close를 추가하지 않았다. Desktop0.8.9는 기존 공개 상태를 유지한다.

후속 문서 경로는 maintainer 직접 운영 기록(Option M)이다. paldyn/HanPage admin/maintain/write 권한과 이 웹 수정·PR·배포에 대한 사용자 승인을 근거로 **archive review·mydocs/pr/assets·오늘할일만** devel에 반영한다. 구현·테스트·workflow·baseline·기존 sample은 포함하지 않는다. [사용자39파일 보존](../assets/web_favicon_20261003/primary-preservation.json), [전용 browser 정리](../assets/web_favicon_20261003/public-browser-cleanup.json), [소유 산출물 정리와 보존](../assets/web_favicon_main_20261003/cleanup.json)에 경로·실행 결과를 남겼다. 전체 devel→main promotion의 별도 ancestry/fork/inventory 차단은 이번 로고 공개와 분리된 미해결 운영 범위로 그대로 기록한다.

최종 정리: 소유한 빌드/임시 output9경로와 전용 Chrome profile/helper를 제거했다. 실제 managed archive 호출은 `/Users/lwm/.codex/worktrees/upstream-sync-final-20261001/HanPage`, `/Users/lwm/.codex/worktrees/web-favicon-release-20261003/HanPage` 모두 “This worktree is protected by a pinned task or workspace.”를 반환했다. 보호를 우회하지 않고 clean 작업공간과 복구용 local task branch를 유지한다. 사용자 primary HEAD와39파일은 최종 bytes/status/SHA256 대조에서도 모두 동일했다.
