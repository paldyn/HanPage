# Task #1 구현 계획서 v2 — Stage 4 개정 (Windows 포함 + CI 자동 빌드)

- **이슈**: #1 (M100, v1.0.0) · **브랜치**: `local/task1`
- **원본 계획서**: [`task_m100_1_impl.md`](task_m100_1_impl.md) — 본 문서는 그 **§4 Stage 4 를 개정**한다(§1~§3, 회귀 매트릭스 등은 유효).
- **개정 사유**: 작업지시자 지시 — "윈도우도 포함". 원본 §7(Out of Scope)의 "Win/Linux 인스톨러 실제 산출 + CI 릴리스 매트릭스" 중 **Windows + CI** 를 Stage 4 로 편입한다(Linux 는 범위 밖 유지).
- **빌드 방식 결정**(작업지시자 승인): **CI 자동 빌드**(GitHub Actions).

---

## 0. 전제 / 제약

- **macOS 로컬은 Windows 인스톨러를 빌드할 수 없다.** Tauri 의 Windows 번들러(NSIS)는 Windows 호스트가 필요하며, 크로스 컴파일은 공식 미지원. → Windows `.exe` 는 **GitHub Actions `windows-latest` 러너**가 산출한다.
- 로컬(현재 macOS)에서는 `.dmg`/`.app` 와 설정/워크플로 정합성만 검증한다. **실제 Windows `.exe` 산출·검증은 작업지시자가 `desktop-v*` 태그 push(또는 수동 dispatch)로 CI 를 트리거**해 확인한다.
- **코드 서명·공증은 범위 밖 유지**(원본 §7). 미서명 산출물 → macOS Gatekeeper / Windows SmartScreen 경고는 정상(문서화).
- **Pages 무영향 불변**: 본 Stage 의 어떤 변경도 GitHub Pages 배포 산출물·트리거에 영향을 주지 않는다.

---

## 1. 기존 워크플로 정합 (충돌 회피)

| 기존 워크플로 | 트리거 | 비고 |
|--------------|--------|------|
| `deploy-pages.yml` | `main` push (+`paths-ignore`) | Pages 배포. `rhwp-desktop/**` 추가 |
| `release-binary.yml` | **`v*` 태그** + dispatch | CLI `rhwp` 바이너리. **데스크톱과 태그 분리 필수** |
| `ci.yml` / `codeql.yml` / `npm-publish.yml` / `render-diff.yml` | 각자 | 데스크톱 무관 |

→ 데스크톱 릴리스는 **`desktop-v*` 태그** + `workflow_dispatch` 로 트리거해 `release-binary.yml`(`v*`)과 도메인 분리. `main` push 트리거를 쓰지 않으므로 Pages 워크플로와도 분리.

---

## 2. 개정 Stage 4 단계

### 4-1. Windows 번들 타깃 + 크로스플랫폼 빌드 스크립트

- **`rhwp-desktop/src-tauri/tauri.conf.json`**
  - `bundle.targets`: `["dmg", "app"]` → **`"all"`** (플랫폼별 유효 타깃 자동 선택: macOS=dmg/app, Windows=nsis). CI 에서는 `--bundles` 로 플랫폼별 명시 산출.
  - `fileAssociations` 는 Stage 3 에서 hwp/hwpx 등록 완료 → Windows 는 NSIS 설치 시 레지스트리 연결. **추가 변경 없음**.
  - `bundle.windows` 기본값(webviewInstallMode=downloadBootstrapper) 충분 → 명시 설정 불요.
- **`rhwp-desktop/package.json` — `copy-wasm` 크로스플랫폼화**
  - 현재 `cp ../pkg/... ../rhwp-studio/public/` 는 Windows 러너(cmd.exe)에서 `cp` 미동작.
  - → Node 기반 복사로 교체(전 플랫폼 동작, macOS 로컬 동작 동일):
    ```json
    "copy-wasm": "node -e \"const fs=require('fs');for(const f of ['rhwp_bg.wasm','rhwp.js'])fs.copyFileSync('../pkg/'+f,'../rhwp-studio/public/'+f)\""
    ```
  - 데스크톱 전용 스크립트 → 웹/Pages 무영향.

### 4-2. macOS 로컬 번들 (검증 가능)

- `cd rhwp-desktop && npm run build`(= `tauri build`) → `.dmg`/`.app` 산출 확인.
- 아이콘: `src-tauri/icons/`(32x32/128x128/icon.icns/icon.ico) 기존 존재 → 재생성 불요.

### 4-3. CI 릴리스 워크플로 (`.github/workflows/desktop-release.yml`, 신규)

- **트리거**: `push: tags: ['desktop-v*']` + `workflow_dispatch`(테스트용).
- **matrix**: `macos-14`(aarch64 `.dmg`) + `windows-latest`(x64 NSIS `.exe`). (Intel/universal mac 은 후속 옵션.)
- **스텝 순서**(copy-wasm 이 `../pkg/` 를 참조하므로 WASM 선행 빌드 필수):
  1. checkout
  2. Rust toolchain(stable, host target) + `wasm32-unknown-unknown`
  3. wasm-pack 설치 → `wasm-pack build --target web --release`(→ `pkg/`)
  4. Node 20 setup
  5. `npm install`(rhwp-studio) + `npm install`(rhwp-desktop)
  6. (Linux 빌드 아님 → webkitgtk 등 불요) **`tauri-apps/tauri-action@v0`**:
     - `projectPath: rhwp-desktop`
     - `args`: macOS=`--bundles dmg`, Windows=`--bundles nsis`(matrix 변수)
     - `tagName`/`releaseName`: `desktop-v*` 기반, 자산 자동 첨부
     - tauri-action 이 `beforeBuildCommand`(=`npm run build:frontend`=copy-wasm+studio build:desktop) → `tauri build` 수행
- **서명 미설정**(범위 밖) → 미서명 산출.
- **캐시**: `actions/cache` (cargo registry/git/target) — `release-binary.yml` 패턴 준용.

### 4-4. Pages 무영향 가드 (`deploy-pages.yml`)

- `paths-ignore` 에 **`rhwp-desktop/**`** 추가(기존 `rhwp-firefox/**`·`rhwp-vscode/**` 형제 패턴과 동일).
- `desktop-release.yml` 은 `desktop-v*` 태그 트리거 → `main` push 인 Pages 워크플로와 **트리거 도메인 분리**(상호 무영향).

### 4-5. 무영향 회귀 검증 (로컬 가능 범위)

| 검증 | 기준 |
|------|------|
| 루트 `cargo build`/`--release` | `src-tauri` 무관, 산출물 불변 |
| WASM 빌드 | 산출물 불변 |
| rhwp-studio **웹** 빌드 | `tsc` 클린, `@tauri-apps` 0, PWA on, `public/rhwp.js` clean |
| 데스크톱 빌드 | `tsc` 클린, PWA off, copy-wasm(node) 동작 동일 |
| `deploy-pages.yml` | `rhwp-desktop/**` paths-ignore 추가 |
| 워크플로 YAML | `desktop-release.yml` 문법 유효성(로컬 파싱/검토) |
| 엔진 식별자 | crate `rhwp`/`@rhwp/*`/Edward Kim 저작권 불변 |

> Windows `.exe` 실제 산출은 CI(GitHub) 영역 → 작업지시자가 `desktop-v*` 태그/ dispatch 로 트리거해 검증.

### 4-6. 최종 보고서

- `mydocs/working/task_m100_1_stage4.md`(Stage 4 완료 보고)
- `mydocs/report/task_m100_1_report.md`(아키텍처·산출물·무영향·잔존/후속) + `mydocs/orders/` 갱신.

---

## 3. 변경/신규 파일 (Stage 4)

**수정**
- `rhwp-desktop/src-tauri/tauri.conf.json` — `bundle.targets: "all"`.
- `rhwp-desktop/package.json` — `copy-wasm` 크로스플랫폼(node).
- `.github/workflows/deploy-pages.yml` — `paths-ignore` 에 `rhwp-desktop/**`.

**신규**
- `.github/workflows/desktop-release.yml` — macOS/Windows 번들 CI(`desktop-v*` 태그).
- `mydocs/working/task_m100_1_stage4.md`, `mydocs/report/task_m100_1_report.md`.

---

## 4. 잔존 (여전히 Out of Scope)

- 코드 서명·공증(macOS notarization / Windows Authenticode).
- Linux 인스톨러(`.deb`/AppImage), 자동 업데이트, universal/Intel mac.
- 네이티브 `rlib` 코어 직접 호출(WASM 유지).

---

## 승인 요청

위 **Stage 4 개정안(Windows NSIS + CI 자동 빌드, `desktop-v*` 분리, copy-wasm 크로스플랫폼, Pages 무영향 유지)** 진행 승인을 요청합니다. 승인 시 구현 → 로컬 검증 → 커밋 → CI 트리거 안내 순으로 진행합니다.
