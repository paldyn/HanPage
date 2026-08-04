# Task #23 Stage 3 완료 보고서 — 리브랜딩 + CI/Pages + studio 데스크톱 글루

- **이슈**: [paldyn/HanPage#23](https://github.com/paldyn/HanPage/issues/23) (M100)
- **단계**: Stage 3 / 5
- **계획서**: `mydocs/plans/task_m100_hp23_impl.md` §2-2·2-3·§3 Stage 3
- **일자**: 2026-06-02
- **product 커밋**: `50074b26`(Stage 3a 데스크톱 글루) · `5e921ade`(Stage 3b 리브랜딩+CI/Pages+보안)

## 1. 단계 목표 (구현 계획서 §3)

> **Stage 3** — 리브랜딩 + CI/Pages 재적용(§2-2·2-3) / 44파일 해결·HanPage 브랜딩 잔존·엔진 식별자 보존·Pages 격리

## 2. 실측 스코프 (계획 "44파일 기계적" → 실측 보정)

upstream(`f6ffe9d6`)↔main(`0156d8ef`)의 비-엔진 차이는 **약 123파일**이나, 대부분은 **upstream의 rhwp-studio 진화분**(33 PR이 추가한 신규 다이얼로그·핸들러 리팩터: `page-border-dialog.ts`·`endnote-shape-dialog.ts`·`input-handler.ts` 등)으로 **브랜딩이 아님 → upstream 유지**(되돌리면 33-PR 동기화 무효화).

→ Stage 3는 "파일 복사"가 아닌 **3-버킷 외과 분리**로 수행. 실제 편집/채택 = **46파일**, 나머지 ~77파일은 upstream 그대로.

## 3. 3-버킷 처리 결과

| 버킷 | 처리 | 파일 |
|------|------|------|
| **A. upstream 유지(무작업)** | 그대로 | rhwp-studio/src/**.ts 진화분 · `.mailmap`·`dependabot.yml`·`codeql.yml` · `CHANGELOG{,_EN}.md`·`THIRD_PARTY_LICENSES.md`(엔진 추적) · `rhwp-shared/.../document-url-resolver.test.js`(엔진 fixture) · `rhwp-{chrome,firefox}/README.md`(paldyn 미리브랜드) · `scripts/prepare-npm.sh`(paldyn 미변경) · `main.ts` issue#888 링크 · `.gitattributes`(pdf-large LFS) · workflows `ci/npm-publish/release-binary/render-diff.yml`(브랜드 토큰 0) · `font-loader.ts`(브랜드 토큰 0) |
| **B. main 그대로 채택** | 복사 | 로고 H-마크 셋(logo-16/32 삭제 포함 main 정합)·favicon·PWA 아이콘·`public/{CNAME,LICENSE}`·`web/fonts/{LICENSES.md,OFL.txt}`·`deploy-pages.yml`·`desktop-release.yml`·CLEAN 2(`rhwp-chrome/PRIVACY.md`·`npm/README.md`)·데스크톱 글루(`desktop-bridge.ts`·`file.ts`) |
| **C. 외과 적용** | upstream+브랜드 | (take-main 안전) 문서 9: `README{,_EN}.md`·`CONTRIBUTING.md`·`CLAUDE.md`·`.github/{SECURITY,CODE_OF_CONDUCT}.md`·`rhwp-firefox/PRIVACY.md`·`rhwp-vscode/README.md`·`npm/editor/README.md` / 코드·설정 7: `Cargo.toml`·`npm/editor/{package.json,index.d.ts,index.js}`·`rhwp-vscode/package.json`·`about-dialog.ts`·`vite.config.ts` / (surgical) `index.html`(title만) · `main.ts`(데스크톱 배선 2훅) |

### take-main vs surgical 판정 방법론
- **판별식**: `git diff f6ffe9d6 origin/main -- <file>`가 **브랜드 라인만**이면 take-main 안전(= upstream 신버전 + 브랜드), upstream 기능 라인이 섞이면 surgical.
- 라인수 동일(`==`) 파일(CONTRIBUTING/CLAUDE/SECURITY/CODE_OF_CONDUCT/PRIVACY/vscode README/npm editor README)은 upstream이 리플레이-정지점 이후 미변경 → 본문 보존 확인.
- `README{,_EN}`는 paldyn이 의도적으로 축약한 fork README(652→508) → take-main이 곧 paldyn 의도.
- **`index.html`은 MIXED**(main이 grid-view·endnote-shape·page-border·note-toolbar를 제거 = upstream 기능 손실 위험) → take-main 불가, **title만 surgical**.

## 4. Stage 2 스코프 보정 — studio 데스크톱 글루 (Stage 3a)

Stage 2가 `HanPage-Desktop/`(Tauri 셸)만 복사해 빠졌던 **studio측 데스크톱 통합**을 보정 재적용. 없으면 이식한 셸이 네이티브 브리지 없는 studio를 로드해 데스크톱 기능 死.
- `core/desktop-bridge.ts` — paldyn 신규(자기완결, studio 내부 import 0) → 추가.
- `command/commands/file.ts` — 데스크톱 저장/열기 훅 **+51/−0**(upstream 대비 순수 가산) → 채택.
- `main.ts` — `initDesktopBridge` 배선 2훅(import + init 블록) 가산. upstream 각주 리팩터 보존.
- **심볼 검증**: `dispatcher`(main.ts:88)·`open-document-bytes` 이벤트(upstream 존재)·bridge exports 모두 upstream 신버전에 존재 → 배선 호환.

## 5. 브랜드 변환 규칙 (확정)

| 변환(서비스 표면) | 보존(엔진 식별자) |
|------------------|------------------|
| `github.com/edwardkim/rhwp` → `github.com/paldyn/HanPage` | crate `rhwp` · `[lib] name=rhwp` |
| `edwardkim.github.io/rhwp`/데모 → `hanpage.paldyn.com` | `@rhwp/*` npm 스코프 |
| 제품명 `rhwp`(UI title·about·README 헤더) → `HanPage` | `edwardkim.rhwp-vscode`(publisher edwardkim) |
| 로고/파비콘 → H-마크 · CNAME=hanpage.paldyn.com | Edward Kim 저작권 · 엔진 크레딧 링크(CHANGELOG·issue#888) |

(`hwpio` 중간 브랜드 흔적 0건 — 최종 HanPage 완전 전환 확인.)

## 6. 보안 위생 (no-committed-secrets 부합)

paldyn이 추가한 보안 조치를 채택 — 작업지시자 "시크릿 커밋 금지" 제약 직접 부합:
- `.gitignore`에 cert/key 차단 섹션 추가(`web/certs/`·`*.pem`·`*.key`·`*.p12`·`*.pfx`). main = upstream + 삽입(안전).
- upstream에 커밋돼 있던 **자체서명 개인키** `web/certs/localhost-{cert,key}.pem` 제거(main 정합). web/certs 외 추적 `.pem/.key` 없음 확인 → 광범위 패턴 부작용 없음.

## 7. 검증 결과

| 검증 | 결과 |
|------|------|
| 엔진 `src/` 변경 vs upstream | **0** ✅ (Stage 3 엔진 0줄) |
| crate `rhwp`(package+lib) | 잔존 ✅ |
| `@rhwp/` npm 스코프 | 5파일 잔존 ✅ |
| `edwardkim.rhwp-vscode`/publisher edwardkim | 6파일 잔존 ✅ |
| Edward Kim 저작권(about-dialog) | 잔존 ✅ |
| HanPage 서비스 브랜드 | 22파일 ✅ |
| hanpage.paldyn.com / CNAME | 9파일 / CNAME=hanpage.paldyn.com ✅ |
| Pages 격리 | `deploy-pages.yml` paths-ignore에 `HanPage-Desktop/**` 포함 ✅ |
| 잔여 `edwardkim.github.io/rhwp` | CHANGELOG×2·prepare-npm.sh(의도적 engine-credit/paldyn 미변경) ✅ |

**산출 규모**: 46파일 변경(+737/−474), HanPage-Desktop 제외.

## 8. 잔여 사항 → Stage 4

- **studio TS 타입체크/빌드**: 데스크톱 글루(main.ts 배선·file.ts·desktop-bridge.ts)와 take-main 코드(about-dialog·vite.config)의 컴파일 검증은 `cargo`로 안 잡히므로 Stage 4에서 `npm run build:frontend`(또는 tsc) 수행 대상.
- 엔진 `cargo build`/`cargo test`·엔진 src diff=0 재확인·누락 33 PR 흡수·무손실 검증.

## 9. 보존 불변식 점검

| 불변식 | 상태 |
|--------|------|
| rhwp 엔진 식별자(crate/`@rhwp`/edwardkim) | 보존 ✅ |
| paldyn 브랜딩(HanPage·hanpage.paldyn.com·H-마크) | 적용 ✅ |
| GitHub Pages 무영향 | `deploy-pages.yml`는 `push:[main]` 트리거 — devel force-push가 Pages 미트리거. 데스크톱 paths-ignore 격리 유지 ✅ |
| 시크릿 금지 | 신규 시크릿 0 + 기존 커밋 개인키 제거(강화) ✅ |
| 엔진 `src/` 무변경 | diff=0 ✅ |

## 10. 다음 단계

- **Stage 4** — 검증: `cargo build`+`cargo test`(네이티브)·엔진 src diff=0·studio 빌드(데스크톱 글루)·누락 33 PR 흡수·브랜딩/데스크톱/무손실.
- **승인 대기** — 본 보고서 승인 후 Stage 4 착수.
