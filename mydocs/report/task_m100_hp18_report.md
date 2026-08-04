# Task #18 — Windows 실행 파일명 `rhwp-desktop.exe` → `HanPage.exe` (최종 결과 보고서)

- 이슈: [paldyn/HanPage#18](https://github.com/paldyn/HanPage/issues/18) · 마일스톤 M100(v1.0.0) · #13 후속
- 브랜치: `local/task18` (origin/main `a7228f52` 분기) · 머지 타깃: **`main` PR** (#12·#13·#16·#17 선례)
- PR: [#19](https://github.com/paldyn/HanPage/pull/19)(mainBinaryName+버전 범프) · [#21](https://github.com/paldyn/HanPage/pull/21)(0.7.14→0.7.13 환원, 후속 결정)
- 상태: **완료** — `mainBinaryName="HanPage"` 수정을 **`hanpage-desktop-v0.7.13` 단일 릴리스로 재적용** · 양 플랫폼 본체명 CI 로그 직접 확정 · 이슈 #18 클로즈 ✅ (버전 전략은 §11 참조)

## 1. 배경 / 목표

데스크톱 앱의 사용자 노출 명칭은 이미 **HanPage**(productName·창 title·identifier `com.paldyn.hanpage`·설치 마법사 파일명 `HanPage_*-setup.exe`)이나, **Windows 설치 후 본체 실행 파일이 `rhwp-desktop.exe`** 로 노출되었다. 시작메뉴 바로가기 대상·작업관리자 프로세스명·NSIS "실행 중 앱 종료" 프롬프트가 모두 `rhwp-desktop.exe`. → 본체 실행 파일명을 **`HanPage.exe`** 로 통일한다. **Rust 크레이트 식별자(`rhwp-desktop`/`rhwp_desktop_lib`)는 유지**(#13 결정 — 변경 시 빌드 영향만 늘고 사용자 가치 0).

## 2. 원인 (확정 — Tauri v2 공식 스키마)

`@tauri-apps/cli/config.schema.json` `mainBinaryName` 정의(설치된 CLI 기준):

> Overrides app's main binary filename. **By default, Tauri uses the output binary from `cargo`**, by setting this, we will rename that binary in `tauri-cli`'s `tauri build` command... this config should not include the binary extension (e.g. `.exe`), we'll add that for you.

- 기본값 = cargo 산출 바이너리 = `[package] name` = `rhwp-desktop` → `rhwp-desktop.exe`. (Tauri v1 은 productName 으로 자동 리네임했으나 **v2 는 크레이트명을 그대로** 사용 — v1→v2 마이그레이션의 알려진 변경점.)
- 현재 `tauri.conf.json` 에 `mainBinaryName` **없음** → 번들러가 `rhwp-desktop.exe` 를 그대로 설치.
- **#13 정정**: #13 에서 `[package] name` 을 "비노출이라 유지" 로 분류했으나, **Windows 에서는 `[package] name` 이 실행 파일명으로 표면화**된다. `mainBinaryName` 으로 "노출 exe명"·"크레이트명" 을 분리하면 #13 원의도(노출=HanPage, 내부=유지)가 완성된다.

## 3. 변경 요약 (Stage 1, `a4b8ea26`)

### 3-1. 핵심 수정 — `tauri.conf.json`

```jsonc
"productName": "HanPage",
"mainBinaryName": "HanPage",   // ← 신규 (번들 단계에서 본체 바이너리 리네임)
"version": "0.7.14",
```

### 3-2. 버전 범프 `0.7.13` → `0.7.14` (릴리스 태그 `hanpage-desktop-v0.7.14` 정합)

| 파일 | 변경 |
|------|------|
| `HanPage-Desktop/src-tauri/tauri.conf.json` | `mainBinaryName` 신규 + `version` |
| `HanPage-Desktop/src-tauri/Cargo.toml` | `version` 만 (`[package] name`·`[lib] name` 무변경) |
| `HanPage-Desktop/package.json` | `version` |
| `HanPage-Desktop/package-lock.json` | root·`packages[""]` version (`npm install`, 0 vulns·dep churn 없음) |

> `src-tauri/Cargo.lock` 은 **미추적**(`git ls-files` 0건) → 커밋 대상 아님. CI 빌드가 `Cargo.toml`(0.7.14)에서 재생성.

### 3-3. 대안 검토 (채택 안 함)

스키마는 "가능하면 package name 또는 `[[bin]] name` 변경" 을 우선 권고하나, 본 프로젝트는 **#13 결정으로 크레이트 식별자를 유지**한다. `mainBinaryName` 은 cargo 산출물명(크레이트 내부)은 그대로 두고 **번들 산출물명만** 바꾸는 가장 외과적 수단이라, 크레이트 manifest 에 사용자 노출명을 섞지 않는다. → `mainBinaryName` 채택.

## 4. 유지(무변경) 항목

- **Rust 크레이트 식별자**: `Cargo.toml` `[package] name="rhwp-desktop"`·`[lib] name="rhwp_desktop_lib"`, `main.rs` `rhwp_desktop_lib::run()` — 무변경(HEAD 대비 IDENTICAL).
- productName(`HanPage`)·identifier(`com.paldyn.hanpage`)·창 title·fileAssociations(hwp/hwpx 2건)·아이콘 — 무변경.
- 워크플로(`desktop-release.yml` 등)·프런트(`rhwp-studio`)·엔진(`rhwp`)·GitHub Pages — 무관, 무변경.

## 5. 검증 결과 (정적, 7항목 전부 ✓)

| 케이스 | 방법 | 결과 |
|--------|------|------|
| `mainBinaryName` 스키마 유효 | Tauri v2 `config.schema.json` properties 확인 | **공식 스키마에 존재**(원인·수정 모두 공식 문서로 확정) ✓ |
| JSON 유효성 | `node require(tauri.conf.json)` | 파싱 OK, `mainBinaryName="HanPage"` ✓ |
| 크레이트명 무변경 | 작업트리 vs `HEAD` `^name =` 비교 | `rhwp-desktop`·`rhwp_desktop_lib` **IDENTICAL** ✓ |
| 노출 메타 무회귀 | node 로 키 추출 | productName·identifier·fileAssoc 2건 **불변** ✓ |
| diff 최소성 | `git diff` | mainBinaryName 1줄 + version 필드만 ✓ |
| 버전 일괄 정합 | 4파일(+lock 2필드) | 전부 `0.7.14` ✓ |
| lock churn | `npm install` 출력 | 0 vulnerabilities, dep 변동 없음 ✓ |

## 6. 영향 범위

- **데스크톱 앱 한정 설정 수정.** 소스 로직(엔진·스튜디오·셸) 무변경.
- **GitHub Pages 무영향**: `deploy-pages.yml` `paths-ignore`(`HanPage-Desktop/**`, #13)에 의해 데스크톱 전용 변경은 `main` push 시에도 웹 배포를 트리거하지 않음.
- **데스크톱 반영**: 본 PR 머지 후 **`hanpage-desktop-v0.7.14`** 태그 push → CI 가 `.dmg`/`.exe` 산출 + "HanPage Desktop-v0.7.14" 릴리스 생성, 설치 본체 = `HanPage.exe`.

## 7. 기능 검증(Windows 본체명) — CI 위임 사유

- **Windows `.exe` 본체명은 macOS 로컬에서 산출 불가**(NSIS 번들러는 Windows 호스트 필요) → 최종 권위 검증은 **CI 릴리스 산출물(Stage 3)**.
- macOS 로컬 `.app` 내부 바이너리명으로 proxy 검증 가능하나, (a) 동작이 **공식 스키마로 문서화**, (b) 실제 관심 대상(Windows)은 CI 한정, (c) 풀 빌드(WASM+studio+cargo)는 proxy 대비 비용 과다 → **계획서 §5 허용대로 CI 검증으로 갈음**.
- Stage 3 확인 항목: `HanPage_0.7.14_x64-setup.exe` 설치 본체 = `HanPage.exe`(+ 시작메뉴/프로세스명), `.dmg` 정상, 릴리스 표시명 "HanPage Desktop-v0.7.14".

## 8. 리스크 / 주의

- **업그레이드 잔존**: v0.7.13(`rhwp-desktop.exe`) 설치본 위에 v0.7.14(`HanPage.exe`) 설치 시 NSIS 가 이전 언인스톨러를 실행하지 않으면 동일 설치폴더에 구 exe 가 남을 수 있음. 신규 설치 위주(사전 1.0)라 영향 경미.
- **태그 스킴**: 차기 데스크톱 릴리스는 `desktop-v*` 가 아니라 **`hanpage-desktop-v*`** 로 태그해야 트리거됨(#13 정의).
- 크레이트명은 절대 변경하지 않음(#13 결정 유지).

## 9. Stage 3 (완료) — 머지·릴리스·검증

- PR #19 머지(main, `448fd709`) → **`hanpage-desktop-v0.7.14`** 태그 push → `desktop-release.yml` 트리거.
- CI 산출 확인: `HanPage_0.7.14_x64-setup.exe` + `HanPage_0.7.14_aarch64.dmg`, 표시명 "HanPage Desktop-v0.7.14".
- **CI 빌드 로그에서 본체명 직접 확정**: Windows `Built application at: …\release\HanPage.exe` → `makensis to produce …\HanPage_0.7.14_x64-setup.exe`, macOS `Built application at: …/release/HanPage` → `HanPage.app`. `rhwp-desktop` 본체 참조 **0건**(양 플랫폼).
- 이슈 #18 클로즈(완료 코멘트) + 오늘할일 갱신(PR #20).

## 10. 커밋

| 커밋 | 내용 |
|------|------|
| `b460f507` | 수행+구현 계획서 + 오늘할일 |
| `a4b8ea26` | Stage 1 — mainBinaryName=HanPage 추가 + 버전 0.7.14 범프(4파일) + Stage 1 보고서 |
| `2b9a829e` | Stage 2 — 최종 결과 보고서 |
| `448fd709` | PR #19 머지 (mainBinaryName + 0.7.14) |
| `abb56e3d` | PR #21 머지 — 버전 0.7.14 → 0.7.13 환원 (§11) |

## 11. 버전 전략 변경 (후속 결정)

Stage 1~3 은 새 패치 버전 **0.7.14** 로 범프·발행·검증을 마쳤다. 이후 작업지시자 결정으로 **버전 인플레이션을 피하고 `mainBinaryName` 수정을 `0.7.13` 단일 릴리스로 재적용**하기로 변경했다.

- **사전 1.0·신규 설치 위주·자동 업데이터 미설정**(`tauri.conf.json` 에 updater 없음 → 버전 단조 증가 제약 없음) 맥락에서, 0.7.13 을 "아직 외부 배포 전" 으로 보고 재태그 허용.
- **수행(PR [#21](https://github.com/paldyn/HanPage/pull/21), `abb56e3d`)**: version 4파일 `0.7.14`→`0.7.13` 환원(`mainBinaryName` 유지) → `hanpage-desktop-v0.7.14` 릴리스/태그 삭제 + 기존 `hanpage-desktop-v0.7.13` 릴리스/태그 삭제 → `hanpage-desktop-v0.7.13` 태그를 `abb56e3d` 로 재생성·push → CI 재빌드.
- **결과**: 단일 릴리스 **"HanPage Desktop-v0.7.13"**(`HanPage_0.7.13_x64-setup.exe`·`HanPage_0.7.13_aarch64.dmg`). 재빌드 Windows 로그 `Built application at: …\release\HanPage.exe` → `makensis to produce …\HanPage_0.7.13_x64-setup.exe`, `rhwp-desktop.exe` **0건** 재확인.
- **순(net) 효과**: 타스크 분기점(`a7228f52`) 대비 영속 소스 변경 = **`mainBinaryName: "HanPage"` 1줄**(버전 번호 불변). 트레이드오프: 같은 0.7.13 태그가 시점에 따라 내용물(`rhwp-desktop.exe`→`HanPage.exe`)이 달라지는 릴리스 불변성 위반을 감수.
- **유지**: #13 이전 옛 태그 스킴 릴리스 `desktop-v0.7.13`(2026-05-30)은 본 변경 범위 밖이라 그대로 둠.
