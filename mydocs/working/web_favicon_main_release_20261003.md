# 공개 웹 로고 전용 배포 후보

## 반영 범위

사용자가 웹 버전의 이전 파비콘을 지적하고 수정·PR·웹 배포에 “응”으로 승인했다. 일반 devel 수정은 [PR117](https://github.com/paldyn/HanPage/pull/117)로 진행한다. 별도로 공개 웹의 정확한 main `f590c24de8f75c90a2a143e3c1fd32dcb73c0d8a`에서 로고 관련13파일만 바꾼 후보 `fd7dd1d22113930e371fe71aeef75664691693e7`를 준비했다.

main과 실제 공개 source `cc8d04812e44ce4746f6855b88db201b6a7baba3`의 차이는 문서2파일뿐이다. main 후보는 공개 engine/Studio0.8.2를 유지한다. devel의0.8.6 engine·Studio의 신규 기능·Desktop0.8.9 UI·CI workflow는 이번 공개 웹에 포함하지 않는다. [후보13파일](assets/web_favicon_main_20261003/candidate-scope.json).

기존 main HTML의 locale·메뉴·초기화나 Vite config를 통째로 덮지 않았다. favicon link와 PWA5경로만 문맥에 맞춰 변경하고, 신규 hashed source ICO·versioned PNG와 기존 alias를 동일한 승인 Desktop 로고로 복사했다. 원본과 크기·SHA256는 [자산 근거](assets/web_favicon_main_20261003/prepared-assets.json)에 있다. PWA192는256의 비례 축소이며 새 디자인이 아니다.

![승인된 새 로고](assets/web_favicon_main_20261003/new-logo-preview.png)

## 릴리스 경로의 결정 근거

[배포 가이드](../manual/publish_guide.md)의 기본은 검증된 devel 전체를 main으로 promotion하는 경로다. 현재 fork는 main이 devel의 ancestor가 아니고 workflow24개 inventory에16개 위반이 있으며, promotion policy가 upstream repository/actor에 고정돼 있다. [읽기 전용 감사](assets/web_favicon_main_20261003/release-scope.json). 전체 promotion에는 브랜치 관계와 fork workflow 정책의 별도 정비가 필요하며, favicon13파일에28,383개 차이와 운영 변경을 섞지 않는다.

이 후보는 승인된 로고13파일 공개를 위한 main 기반 release PR의 구체적 준비 결과다. 일반 source 수정은 최신 devel기반 PR117에서 통합한다. 같은13파일을 기존main문맥에 적용한 독립release후보는 별도PR에서 병렬검증하되 main병합은 PR117의 정상통합 뒤에 진행한다. 사용자의 웹 파비콘 PR·배포 승인은 이 동일한제품범위의 정상releasePR과 자동Pages게시를 포함하므로 같은요청의 승인을 반복하지 않는다. 이는 전체devel→main workflow승격이 아니며 기존promotion차단을 통과로 보고하거나 policy·검증·권한·requiredcheck를 바꾸지 않는다. main PR의 기존 Full CI·CodeQL·Render와 MERGEABLE/CLEAN·exact latest head를 확인하고 관리자 우회 없이 정상 병합한다.

## 검증과 공개 확인

이 exact main 구현의 [독립 검증](assets/web_favicon_main_20261003/validation.json)은 PASS다. 실제 Pages pin과 같은 wasm-pack0.15.0·Cargo1.93.1로 fresh release web WASM을 빌드하고 public복사 hash가 같은지 확인했다. [fresh WASM](assets/web_favicon_main_20261003/fresh-wasm.json). Node20.20.2/npm10.9.4의 npm install은 lock을 바꾸지 않았고 npm run build(TypeScript+Vite/PWA)가 통과했다. 기존 Studio검사는765PASS/0SKIP/0FAIL이다. 최종0.15패키지에서 [실제 Chrome](assets/web_favicon_main_20261003/browser-results.json)이 hashed favicon200/동일ICOhash·PWA5항목decode/크기·Apple256·offline6URL200/동일hash·page exception0을 확인했다. 처음0.13빌드와0.15최적화WASM은12B달라 최종0.15에서 웹빌드/SW브라우저 검증을 다시 실행했다. 제품source/JS/test가 동일하므로765단위검사는 재실행하지 않았다. 기존 큰chunk warning과 Puppeteer개발의존성의Node22요구warning은 기록했고 빌드는성공했다. browser/test는Node22를 사용했다. Rust/renderer source를 수정하지 않았으므로 조판 원칙·한컴 fixture·Visual Sweep은 비해당이며 아이콘 검증을 문서 조판 일치 근거로 바꾸지 않는다. Linux Pages의 실제 빌드·게시와 공개 endpoint 확인은 남았다.

main의 현재 Pages는 단일 Build & Push job이며 수동 workflow_dispatch도 공개 배포한다. 사전 검증용 dispatch는 하지 않고 정상 main PR merge push의 자동 Pages 빌드를 기다린다. 공개 페이지의 HTML 새 hashed URL·실제ICO/PNG hash·PWA manifest·SW 상태가 확인되기 전에는 배포 완료로 기록하지 않는다. 실패하면 exact source의 원인을 수정하고 검증을 다시 받으며 direct gh-pages push·배포 gate 우회를 하지 않는다.

사용자 primary39개 dirty파일·설치앱·브라우저 cache·공유 target/pr-review를 보존한다. 이미 설치된 PWA의 OS 홈 화면 아이콘 재수집과 사용자 browser toolbar의 실제 시각 표시는 미검증으로 구분한다.
