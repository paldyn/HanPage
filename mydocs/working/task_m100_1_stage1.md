# Task #1 · Stage 1 완료 보고서 — HanPage 데스크톱 앱 (Tauri 1단계)

- **이슈**: #1 (M100, v1.0.0)
- **브랜치**: `local/task1` (base `main`)
- **Stage**: 1 / 4 — Tauri 스캐폴드 + rhwp-studio dist 로드 (PWA off · base=`./`)
- **계획서**: `mydocs/plans/task_m100_1.md`(수행), `mydocs/plans/task_m100_1_impl.md`(구현)
- **일자**: 2026-05-30

---

## 1. Stage 1 목표

기존 `rhwp-studio` 웹 프런트엔드 + WASM 산출물을 **수정 없이** OS 웹뷰(Tauri v2)에
그대로 로드하는 최소 데스크톱 셸을 세운다. 네이티브 기능(열기/저장 dialog, 파일 연결,
메뉴, 최근문서, 윈도우 상태)은 후속 Stage(2~4)로 미룬다.

**불변 제약**: GitHub Pages(웹 빌드)에 일절 영향을 주지 않는다. 데스크톱 전용 분기는
`VITE_TARGET=desktop` 환경변수로만 활성화되며, 기본 웹 빌드 경로는 바이트 단위로 그대로다.

---

## 2. 수행 내용

### 2.1 `rhwp-desktop/` 스캐폴드 (신규, 28개 파일)

`rhwp-ios` / `rhwp-vscode` 형제 패키지와 동일한 모노레포 배치 규약을 따른다.

| 파일 | 역할 |
|------|------|
| `package.json` | npm 스크립트(tauri/dev/build, copy-wasm, build:frontend, dev:frontend), devDep `@tauri-apps/cli ^2` |
| `package-lock.json` | 잠금 파일 (형제 패키지 컨벤션 준수 — chrome/firefox/studio/vscode 모두 tracked) |
| `.gitignore` | `node_modules`, `dist` |
| `README.md` | HanPage 데스크톱 셸 개요 · Pages 무영향 원칙 · 빌드 명령 · 로드맵 |
| `src-tauri/Cargo.toml` | 독립 크레이트 `rhwp-desktop` v0.7.13. 루트 `[workspace]` 신설 금지 주석 명시 |
| `src-tauri/build.rs` | `tauri_build::build()` |
| `src-tauri/src/lib.rs` | `run()` — `tauri::Builder::default().run(generate_context!())` |
| `src-tauri/src/main.rs` | Windows 콘솔 억제 + `rhwp_desktop_lib::run()` |
| `src-tauri/tauri.conf.json` | productName `HanPage`, id `com.paldyn.hanpage`, frontendDist=`../../rhwp-studio/dist`, devUrl=7700, before(Build/Dev)Command, withGlobalTauri, 윈도우(main, HanPage, 1200×840), bundle targets `[dmg, app]` |
| `src-tauri/capabilities/default.json` | main 윈도우 `core:default` 권한만 (1단계) |
| `src-tauri/.gitignore` | `/target`, `/gen/schemas` |
| `src-tauri/icons/*` | 데스크톱(32/64/128/128@2x/icns/ico/png) + Windows 타일 로고. **모바일(android/ios) 제외** |

> 아이콘 원본은 `assets/logo/logo-1024.png`(1009×1024)를 1024×1024로 정규화 후
> `tauri icon`으로 생성. 모바일(android/ios)은 본 데스크톱 프로젝트 범위 밖이라 삭제.

### 2.2 `rhwp-studio` 데스크톱 분기 (기존 파일 최소 수정 2건)

- **`vite.config.ts`**: `const isDesktop = process.env.VITE_TARGET === 'desktop'` 추가 후
  `VitePWA(...)` 를 `...(isDesktop ? [] : [VitePWA({...})])` 로 감쌌다.
  → 웹 빌드(`isDesktop=false`)는 `[VitePWA({...})]` 로 **동작 무변경**, 데스크톱만 PWA 제외.
- **`package.json`**: `"build:desktop": "tsc && VITE_TARGET=desktop vite build --base=./"` 스크립트 추가.
  기존 `"build"` 스크립트는 손대지 않음.

### 2.3 조사지점 C1 결론 — WASM 로드 경로

`src/core/wasm-bridge.ts` 는 `import init from '@wasm/rhwp.js'` 후 **인자 없는** `await init()`
호출이다. wasm-bindgen `--target web` 산출물의 인자 없는 init은
`new URL('rhwp_bg.wasm', import.meta.url)` 로 **모듈 상대 경로** 로딩을 수행한다.
→ `base=./` (상대 base)와 완전 호환. **데스크톱용 경로 수정 불필요**(C1 해소).

### 2.4 WASM → studio 동기화 메커니즘 확인

`copy-wasm` (`cp ../pkg/{rhwp_bg.wasm,rhwp.js} ../rhwp-studio/public/`) 은 신규 발명이 아니라
**기존 CI(`deploy-pages.yml` L85-88)와 동일한 표준 동기화**다. CI도 매 배포마다 동일하게
tracked `public/rhwp.js` 를 빌드 산출물로 덮어쓴다. 따라서 데스크톱 빌드 후 `public/rhwp.js`
가 dirty로 보이는 것은 저장소의 기존 관행이며, **본 커밋에는 포함하지 않는다**(기능과 무관한
WASM 글루 재동기화 · 웹 번들 소스라 Pages 영향 차단 목적).

---

## 3. 검증 결과

### 3.1 데스크톱 프런트 빌드 (`npm run build:frontend`)

`✓ built in 345ms`. 산출 `rhwp-studio/dist`:

- **PWA 산출물 없음** ✓ — `sw.js` / `workbox-*.js` / `manifest.webmanifest` / `registerSW.js` 부재.
- **상대 경로** ✓ — `index.html` 이 `src="./assets/index-*.js"`, `href="./assets/index-*.css"`.
- **WASM 번들** ✓ — `dist/assets/rhwp_bg-DsdHiDs5.wasm` (5,062,007 B) 정상 emit.

### 3.2 웹 빌드 무변경 = Pages 무영향 (`npm run build`)

기본 웹 빌드 산출 `dist`:

- **PWA 산출물 존재** ✓ — `sw.js`, `workbox-dcde9eb3.js`, `manifest.webmanifest`, `registerSW.js`.
- **절대 경로** ✓ — `src="/assets/index-*.js"`, `href="/assets/index-*.css"`.

→ 두 빌드의 유일한 차이는 `isDesktop` 분기 하나이며, 이는 `VITE_TARGET=desktop`(= `build:desktop`
스크립트에서만 주입)에서만 참이 된다. 기본 `build` 경로는 그대로 PWA on + 절대 base.

### 3.3 빌드 격리 (`cargo metadata --no-deps`)

루트 워크스페이스 멤버 = **`rhwp 0.7.13` 단 1개**. `rhwp-desktop` 는 루트 `cargo build`에
포착되지 않음(루트 `Cargo.toml` 에 `[workspace]` 부재 → 자연 격리). Tauri 의존성(tauri/wry/tao 등)
이 루트/WASM 빌드로 새지 않음을 확인.

### 3.4 백엔드 컴파일

`cargo build` (rhwp-desktop/src-tauri) **exit 0** — `Finished dev profile in 36.73s`,
434 packages, tauri 2.11.2 / wry 0.55.1. (직전 세션에서 확인.)

---

## 4. 생성 / 수정 파일

**신규** (`rhwp-desktop/` 28개): `package.json`, `package-lock.json`, `.gitignore`, `README.md`,
`src-tauri/{Cargo.toml, build.rs, .gitignore, tauri.conf.json}`,
`src-tauri/src/{lib.rs, main.rs}`, `src-tauri/capabilities/default.json`,
`src-tauri/icons/*`(17: 데스크톱 7 + Windows 타일 10).

**수정** (rhwp-studio, 2건): `vite.config.ts`(isDesktop 분기), `package.json`(build:desktop 스크립트).

**커밋 제외**(의도): `rhwp-studio/public/rhwp.js`(copy-wasm 재동기화 — §2.4 참조, revert).

---

## 5. Pages 무영향 증빙 요약

| 항목 | 웹 빌드(`build`) | 데스크톱 빌드(`build:desktop`) |
|------|-----------------|------------------------------|
| Service Worker / PWA | 생성 ✓ (기존 유지) | **미생성** (의도) |
| asset base | `/assets/` 절대 | `./assets/` 상대 |
| WASM 번들 | 정상 | 정상 |
| 분기 트리거 | (기본) | `VITE_TARGET=desktop` |

→ 데스크톱 전용 변경은 환경변수 게이트 뒤에만 존재. 웹/Pages 배포 산출물 불변.

---

## 6. 다음 단계 (Stage 2 예고)

- 네이티브 열기/저장 dialog 연동 (`tauri-plugin-dialog`).
- **web-inert 가드 브리지**(A안): `rhwp-studio` 에 `if (window.__TAURI_INTERNALS__)` 가드 +
  동적 import 훅 추가 — 브라우저에서는 no-op, 데스크톱에서만 네이티브↔웹 파일 핸드오프.
- 조사지점 C2(문서 오픈 진입점), C3(저장 경로) 확정.

---

## 7. 리스크 / 미해결

- **GUI 시각 확인**: `npm run dev` 로 HanPage 창에 rhwp-studio UI가 렌더되는지는 작업지시자
  육안 검증 영역(프로젝트 관행). 본 보고서는 빌드/격리/무영향 정적 검증까지 완료.
- **Cargo.lock 미커밋**: 루트 `.gitignore` 가 `Cargo.lock` 전역 무시 → `src-tauri/Cargo.lock`도
  미추적. 재현 가능한 인스톨러를 위해 Stage 4에서 lock 커밋 여부 재검토.
- Win/Linux 인스톨러, 코드서명·공증, 자동 업데이트는 2단계 이후(범위 밖).

---

## 승인 요청

Stage 1(스캐폴드 + dist 로드 + PWA off + base=`./` + Pages 무영향 검증) 완료.
**Stage 2 진행 승인을 요청합니다.**
