# PR #118 리뷰 — 공개 웹 로고 전용 릴리스

## 최종 판정

**승인.** 로고13파일의독립source검토·exact main 로컬검증·실제FULL GitHub Actions를통과했고선행117은정상병합했다. source변경없는최신문서trailing head의원래main CI/CodeQL/Render 재사용·MERGEABLE/CLEAN·exacthead를확인한뒤 관리자우회없이 정상병합한다. 사용자2026-10-03 웹favicon PR·배포 승인에 따른 동일13파일 공개다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#118](https://github.com/paldyn/HanPage/pull/118) / enxec / paldyn/HanPage main |
| 고정 base | `f590c24de8f75c90a2a143e3c1fd32dcb73c0d8a` |
| 구현 / remote code candidate | `fd7dd1d22113930e371fe71aeef75664691693e7` / `ddffefaf584aa4f9733f33bc09d2c3aa84a22985`(동일source,검증·경로docs) |
| 선행 병합 | 일반devel수정 [#117](https://github.com/paldyn/HanPage/pull/117), 독립검증병렬·main병합은117뒤 |
| 관련 이슈 | [#112](https://github.com/paldyn/HanPage/issues/112) 참조만, CLOSED 유지 |
| 절차 | self-review / reviewer 지정 없음 / 관리자우회 없음 / 별도self-comment 없음 |

## 변경과 검토 범위

main과 실제공개cc8d0481 source는 제품파일이동일하고 docs2파일만다르다. 승인된신규 DesktopICO/PNG를13파일로적용했다. main HTML의favicon link1항목과 기존Vite manifesticons5경로만 문맥수정했다. root/Public/sourceICO 동일hash, versionedPNG4/legacyalias4 동일새PNG이며192는256비례축소다. index/config전체를devel로덮지않았고기존locale/메뉴/기능·engine/Studio0.8.2·Desktop0.8.3source·CIworkflow·locks·test/fixtures/goldens는base그대로다. 실제설치Desktop0.8.9는별도공개상태를유지한다.

[정확한13제품파일](../../working/assets/web_favicon_main_20261003/candidate-scope.json), [원본/크기/hash](../../working/assets/web_favicon_main_20261003/prepared-assets.json), [배포범위와승인경로](../../working/web_favicon_main_release_20261003.md)를대조했다. 일반source는devel117으로통합하고동일승인scope의로고만mainrelease로공개한다. 누적전체devel→main promotion은 ancestry/forkpolicy/inventory16위반차단상태를그대로보고하며이PR에서그workflow를승격하거나gate/권한을바꾸지않는다.

## 실제 로컬 검증

[검증](../../working/assets/web_favicon_main_20261003/validation.json), [fresh WASM](../../working/assets/web_favicon_main_20261003/fresh-wasm.json), [Chrome builtapp](../../working/assets/web_favicon_main_20261003/browser-results.json)이다. main의 실제Pagespin0.15.0 wasm-pack/Cargo1.93.1로fresh releaseWASM을sharedtarget/pr-review에서빌드하고publicJS/WASM복사hash를확인했다. Node20.20.2/npm10.9.4의npm install과TypeScript/Vite/PWA build는PASS, locks/원래trackedpublicJS는byte동일하다.765기존testsPASS/0SKIP/0FAIL이다.

최종0.15WASM의builtChrome은hashedfaviconHTTP200/newICO동일hash, PWA5항목 decode/크기·Apple256·실제SW제어후6brandingURL offline200/동일hash·page exception0이었다. initial0.13보다최종optimizedWASM12B차이라최종0.15에서build/browser만다시확인했고동일source/test의765test는재실행하지않았다. 기존큰chunk/개발Puppeteer의Node22요구warning을남겼으며probe/test는Node22에서실행했다.

조판측정/배치/분할/LineSeg/기준값은비해당: Rust/renderer/page-visiblegeometry를수정하지않았다. HWP/HWPX/한컴PDFfixture를사용하지않으며아이콘검사를문서출력일치증거로세지않는다. App문서편집·확장설치·OS에설치된PWA아이콘재수집·사용자browsertoolbar시각은미검증이다.

## GitHub Actions와 시각 증적

codecandidate ddffefaf의 [codeCI감사](../assets/web_favicon_main_20261003/ci-code-candidate.json)는 exacthead/base·실제testedmerge `9b9590c230b41012ec88f2bef342332b7fe6284e`와CI37099603828/CodeQL37099603622/Render37099603625 전체SUCCESS를확인했다. trustedmainclassifier의 `fail-closed:unclassified-path`/`no-green-current-base-build-candidate`로FULL을실행했다. Build&Test111138897726은3archive/4shard의실행수가예상배정과일치하고Lint/NativeSkia/Frontend필수worker가성공한것을실제확인했다. Rust5221PASS/27SKIP/0FAIL(regular3687+693+840,slow1), NativeSkia64PASS, Studio765PASS, freshWASM/Studio/Chrome·Firefox/VSCode/확장dist/fontcontracts PASS이다. CodeQL3언어최종SUCCESS; RenderCanvas3페이지/PDFdirect3페이지/0fail·CanvasKit위반0은통과했다. 별도PDFreport4warnings를남겼고한컴전체시각일치로표현하지않는다. 독립검토는13경로외source/locks/workflows/tests동일과CC8공개runtime동일·Desktop0.8.9태그ICO7frame/128·256·512PNG byte동일·192직접판독·alias/PWA크기/hash·Pagescopy까지대조했다. source변경없는single-parent review/오늘할일/증적후행head의최신checks와실제reuse는별도확인한다. 새develController를기다리거나원래main CI를우회하지않는다.

![새 로고](../../working/assets/web_favicon_main_20261003/new-logo-preview.png)

source새ICO는이미승인된DesktopICO와byte동일하며192PNG를직접열어모양/색을확인했다. 이PNG는branding시각증거이고한컴문서renderVisualSweep은비해당이다. PR본문Markdownimage는최신headrepository/SHA고정URL로맞춘다.

## 배포·후속 처리·복구

main기존Pages workflow는단일Build&Push이며수동dispatch도실제게시한다. 사전dispatch는하지않고정상mainmergepush의자동빌드를기다린다. Pages실제source/워크플로성공/gh-pages commit과공개HTMLhashfavicon/newmanifest/ICO/PNG를확인하고, [기존공개SW 캐시](../assets/web_favicon_main_20261003/public-browser-before.json)를담은본인전용profile에서업데이트/offline7URL을검증한다. 사용자browser/cache/PWA/설치앱은접근하지않는다. 공개검증전에는배포완료로기록하지않는다.

별도selfPR/참조만인CLOSED이슈댓글·close는지시가없어추가하지않는다. 공개후mergeSHA·코드전후동일scope·Issue112CLOSED·실제후속run을확인하고archive/assets/오늘할일운영기록만devel에보존한다. main구버전의push검증CI자동run은실제currentworkflow경계라조사/결과기록하며source/trigger정책을임의변경하거나추가dispatch하지않는다. 실패면원인을확인해정상PR검증을다시받으며directgh-pages게시/gate우회를하지않는다. 사용자39dirty파일/HEAD/sharedtarget/userdependencies/설치앱/forkremotebranch를유지하고ownedgenerated/privateprofiles만증거보존후정리한다.

선행 [PR117](https://github.com/paldyn/HanPage/pull/117)은2026-10-03 05:33:14Z에 `343fde08e1aee07009f353eae8086432997401cc`로정상squash병합했다. finalhead0214122와merge의전체tree는 `789deb3aafb42306afa33f0edb2d77aa3ac702b3`로동일하다. devel후속2운영run은SUCCESS, durationrefresh는no-verified-pr-duration-measurements로갱신보류였고추가검증dispatch없다.
