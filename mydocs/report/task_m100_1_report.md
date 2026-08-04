# Task #1 최종 결과 보고서 — HanPage 데스크톱 앱 (Tauri 1단계)

- **이슈**: [#1](https://github.com/edwardkim/rhwp/issues/1) (M100, v1.0.0)
- **브랜치**: `local/task1` (base `main`)
- **계획서**: `mydocs/plans/task_m100_1.md`(수행), `task_m100_1_impl.md`(구현),
  `task_m100_1_impl_v2.md`(Stage 4 개정 — Windows + CI)
- **단계 보고서**: `mydocs/working/task_m100_1_stage{1,2,3,4}.md`
- **일자**: 2026-05-30
- **결과**: 4 Stage 전체 완료. macOS `.dmg` 로컬 산출 + Windows NSIS CI 자동 빌드 파이프라인 구축.

---

## 1. 목표 및 불변 제약

기존 **rhwp-studio**(웹) 빌드 산출물 + WASM 엔진을 **Tauri v2** 셸로 감싸 데스크톱 앱
(표시명 **HanPage**)을 만든다. 엔진(WASM)은 **무수정**.

**최우선 불변 제약**: "깃헙페이지에 영향가지않게 그냥 데스크톱 앱으로만." → rhwp-studio 에
추가하는 모든 코드는 브라우저에서 **완전한 no-op(web-inert)** 이며, GitHub Pages 배포
산출물·트리거에 일절 영향을 주지 않는다.

---

## 2. 아키텍처

```
┌──────────────────────────── HanPage.app (Tauri v2) ────────────────────────────┐
│  Rust 백엔드 (rhwp-desktop/src-tauri)        OS 웹뷰 (wry)                        │
│  ─────────────────────────────────────       ───────────────────────────────    │
│  • cmd_open_document  (네이티브 dialog)  ◀── invoke ── rhwp-studio (WASM 엔진)    │
│  • cmd_save_document  (네이티브 dialog)                 + canvaskit 렌더          │
│  • cmd_take_pending_documents                          desktop-bridge.ts(web-inert)│
│  • 파일 연결: macOS Opened / Win·Linux argv ── emit ──▶ 펜딩 드레인·메뉴 dispatch  │
│  • 네이티브 메뉴(파일/편집/보기) ─── emit(EVT_MENU) ──▶ dispatcher.dispatch(id)    │
│  • 최근 문서(store) · 창 상태(window-state)                                        │
└────────────────────────────────────────────────────────────────────────────────┘
        frontendDist = ../../rhwp-studio/dist  (데스크톱 빌드: PWA off, base ./)
```

**핵심 설계**:
- **web-inert 브리지**(`rhwp-studio/src/core/desktop-bridge.ts`): `@tauri-apps/*` npm 의존을
  **일절 import 하지 않고** `window.__TAURI__`(전역, `withGlobalTauri: true`)만 사용. 브라우저에선
  `__TAURI_INTERNALS__` 가드로 즉시 return → no-op. 웹 번들에 Tauri 코드 정적/동적 모두 미포함.
- **파일 IO 전부 Rust**: dialog(`tauri-plugin-dialog`) + `std::fs`. JS ACL 권한 불필요
  (앱 자체 command 는 Tauri v2 ACL 대상이 아님) → `capabilities` = `core:default` 유지.
- **파일 핸드오프 2경로**: (a) 메뉴/dialog 열기 = 바이트 직접 반환, (b) 파일 연결/최근 문서/argv
  = Rust 펜딩 큐(`Mutex<Vec>`) 적재 + 도착 신호(emit) → 프런트 드레인(콜드/웜 스타트 모두 안전).
- **빌드 격리**: 루트 `Cargo.toml` 에 `[workspace]` 없음 → `src-tauri` 는 독립 크레이트.
  루트/WASM `cargo build` 그래프에 tauri 계열 의존이 **일절 새지 않음**.

---

## 3. 단계별 요약

| Stage | 내용 | 핵심 산출 |
|-------|------|----------|
| **1** | Tauri 스캐폴드 + rhwp-studio dist 로드 | `rhwp-desktop/` 골격, `vite.config` PWA 분기(`VITE_TARGET=desktop`), 워크스페이스 격리 |
| **2** | 네이티브 열기/저장 dialog + web-inert 브리지 | `cmd_open/save_document`, `desktop-bridge.ts`, `file.ts` C2/C3 분기 |
| **3** | 파일 연결 + 메뉴바 + 최근 문서 + 창 상태 | `fileAssociations`, 네이티브 메뉴(커맨드 위임), store/window-state/single-instance, 펜딩 큐 |
| **4** | 멀티플랫폼 번들 + Pages 격리 | macOS `.dmg`(로컬), Windows NSIS CI(`desktop-release.yml`), `paths-ignore` 가드 |

> Stage 2~4 는 구현계획서 §2 대비 **의존성 정련**(deviation, 승인): `@tauri-apps` npm·
> `tauri-plugin-fs` 미도입, `window.__TAURI__` 전역 + Rust `std::fs` 채택 → rhwp-studio
> `package.json` **무변경** = 최대 web-inert.

---

## 4. 생성 / 수정 파일 (전체)

**신규**
- `rhwp-desktop/**` (디렉터리 전체: `package.json`, `src-tauri/{Cargo.toml, tauri.conf.json, src/lib.rs, capabilities/, icons/}`)
- `rhwp-studio/src/core/desktop-bridge.ts` (web-inert 브리지)
- `.github/workflows/desktop-release.yml` (멀티플랫폼 번들 CI)

**수정 (rhwp-studio — 웹 동작 불변)**
- `rhwp-studio/vite.config.ts` (PWA `VITE_TARGET` 분기)
- `rhwp-studio/package.json` (`build:desktop` 스크립트 추가)
- `rhwp-studio/src/main.ts` (`initDesktopBridge(deps)` 1지점)
- `rhwp-studio/src/command/commands/file.ts` (open/save/save-as web-inert 가드 분기)

**수정 (인프라)**
- `.github/workflows/deploy-pages.yml` (`paths-ignore: rhwp-desktop/**`)

**커밋 제외**(의도): `rhwp-studio/public/rhwp.js` (copy-wasm 재동기화 — 매 Stage revert).

---

## 5. Pages 무영향 종합 증빙

| 항목 | 웹(브라우저) | 데스크톱(Tauri) |
|------|-------------|----------------|
| `initDesktopBridge()` | `__TAURI_INTERNALS__` 가드 → **no-op** | 핸들러·드레인·리스너 등록 |
| `@tauri-apps` npm 의존 | **없음**(전역만) | 〃 |
| 웹 번들 Tauri 코드 | **dist 내 0건** | 〃 |
| rhwp-studio `package.json` 런타임 의존 | **무변경** | 〃 |
| 웹 빌드(`npm run build`) | `tsc` 클린·109 모듈·**PWA on** | 데스크톱: PWA off |
| 루트/WASM cargo 그래프 | tauri 계열 **전부 부재** | src-tauri 독립 크레이트 |
| `deploy-pages.yml` | `rhwp-desktop/**` paths-ignore | 〃 |
| 워크플로 트리거 | Pages=`main` push | 데스크톱=`desktop-v*` 태그(분리) |
| 엔진 식별자 | crate `rhwp`/`@rhwp/*`/Edward Kim 저작권 **불변** | 〃 |

---

## 6. 검증 종합

- **웹 빌드**: `tsc` 클린, 109 모듈, PWA 산출(sw.js/workbox/manifest), dist 내 `@tauri-apps` **0건**.
- **데스크톱 빌드**: `tsc` 클린, PWA off, base `./`.
- **macOS 번들**: `tauri build` exit 0, `HanPage_0.7.13_aarch64.dmg`(35M) + `HanPage.app` 생성.
- **Rust**: `cargo build`/`cargo clippy` 클린(경고 0), 플러그인 4종 정상 컴파일.
- **격리**: `cargo metadata` 워크스페이스 멤버 = `rhwp` 1개, tauri 계열 루트 그래프 부재.
- **워크플로**: `desktop-release.yml`/`deploy-pages.yml` YAML 유효, 트리거 분리.

---

## 7. 빌드 / 실행 방법

**macOS 로컬(.dmg)**
```bash
cd rhwp-desktop
npm install                 # 최초 1회 (studio·desktop 각각)
npm run build               # → src-tauri/target/release/bundle/dmg/HanPage_*.dmg
npm run dev                 # 개발 실행(HanPage 창)
```

**Windows `.exe` (CI — macOS 로컬 빌드 불가)**
```bash
git tag desktop-v1.0.0 && git push origin desktop-v1.0.0   # → GitHub Release 자동 첨부
# 또는 Actions > Desktop Release > Run workflow (dispatch, 테스트 빌드 = 아티팩트 업로드)
```
CI(`windows-latest`)가 NSIS `.exe`, `macos-14`가 `.dmg` 를 산출한다.

---

## 8. 잔존 / 후속 (Out of Scope — Phase 2 이후)

- **코드 서명·공증**: macOS notarization / Windows Authenticode (미서명 → Gatekeeper/SmartScreen 경고).
- **Linux 인스톨러**(`.deb`/AppImage), **universal/Intel macOS**, **자동 업데이트**.
- 네이티브 `rlib` 코어 직접 호출(현재 WASM 유지), 딥링크/다중 윈도우/탭.
- **GUI 시각 확인**(작업지시자 영역): `.dmg` 설치·실행, 파일 연결 더블클릭, 메뉴/창 복원/최근 문서.

---

## 9. 승인 요청

Task #1(HanPage 데스크톱 앱 Tauri 1단계) 4 Stage 전체 완료 및 무영향 검증을 보고합니다.
**최종 승인 및 이슈 #1 처리(클로즈) 방침 지시를 요청합니다.**
(이슈 클로즈는 작업지시자 승인 후에만 수행합니다.)
