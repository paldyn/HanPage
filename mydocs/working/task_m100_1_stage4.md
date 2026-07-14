# Task #1 · Stage 4 완료 보고서 — 멀티플랫폼 번들(macOS 로컬 + Windows CI) + Pages 격리

- **이슈**: #1 (M100, v1.0.0)
- **브랜치**: `local/task1` (base `main`)
- **Stage**: 4 / 4 — macOS `.dmg` 로컬 산출 + Windows NSIS CI 자동 빌드 + Pages 무영향 가드
- **계획서**: `mydocs/plans/task_m100_1_impl.md`(§1~§3) + **`task_m100_1_impl_v2.md`(Stage 4 개정)**
- **일자**: 2026-05-30

---

## 1. Stage 4 목표

데스크톱 앱을 배포 가능한 인스톨러로 번들하고, GitHub Pages 무영향을 워크플로 레벨까지 보강한다.

- **macOS** `.dmg`/`.app` — 현재(macOS) 환경에서 로컬 빌드·검증.
- **Windows** NSIS `.exe` — macOS 로컬 빌드 불가(Tauri Windows 번들러는 Windows 호스트 필요)
  → **GitHub Actions `windows-latest` 러너**가 산출. (작업지시자 지시로 Windows 포함, CI 방식 채택)
- **Pages 무영향** — `deploy-pages.yml` `paths-ignore` + 데스크톱 릴리스 워크플로 트리거 분리.

---

## 2. 수행 내용

### 2.1 Windows 번들 타깃 + 크로스플랫폼 빌드 스크립트

- **`tauri.conf.json`** `bundle.targets`: `["dmg","app"]` → **`"all"`**. 플랫폼별 유효 타깃 자동 선택
  (macOS=app/dmg, Windows=nsis). CI 에서는 `--bundles` 로 플랫폼별 산출 명시.
- **`rhwp-desktop/package.json`** `copy-wasm` 을 Unix `cp` → **Node `fs.copyFileSync`** 로 교체.
  Windows 러너(cmd.exe)에는 `cp` 가 없어 깨지므로 전 플랫폼 동작하는 Node 기반으로 변경.
  데스크톱 전용 스크립트 → 웹/Pages 무영향, macOS 로컬 동작 동일.
- 파일 연결(`fileAssociations`)은 Stage 3 에서 hwp/hwpx 등록 완료 → Windows 는 NSIS 설치 시
  레지스트리 연결. **추가 설정 없음**.

### 2.2 CI 릴리스 워크플로 (`.github/workflows/desktop-release.yml`, 신규)

- **트리거**: `desktop-v*` 태그 push(→ GitHub Release 생성·첨부) + `workflow_dispatch`
  (→ 릴리스 없이 빌드만, 산출물은 워크플로 아티팩트 업로드 = 테스트용).
- **충돌 회피**: 기존 `release-binary.yml`(CLI `rhwp`, **`v*` 태그**)과 태그 패턴 분리.
  `main` push 가 아니므로 `deploy-pages.yml`(Pages)과도 트리거 도메인 분리.
- **matrix**: `macos-14`(aarch64 `.dmg`) + `windows-latest`(x64 NSIS `.exe`).
- **스텝**: checkout → Rust(+wasm32) → rust-cache → wasm-pack → **`wasm-pack build`**(pkg/)
  → Node 20 → `npm install`(studio + desktop) → **`tauri-apps/tauri-action@v0`**
  (`projectPath: rhwp-desktop`, `args: --bundles {dmg|nsis}`, 태그 시 릴리스 생성).
  - copy-wasm 이 `../pkg/` 를 참조하므로 **WASM 빌드를 선행**(tauri-action 의 beforeBuildCommand
    = `build:frontend` = copy-wasm + studio build:desktop 전에 pkg/ 가 존재해야 함).
- **서명 미설정**(범위 밖) → 미서명 산출(macOS Gatekeeper / Win SmartScreen 경고는 정상).

### 2.3 Pages 무영향 가드 (`deploy-pages.yml`)

- `paths-ignore` 에 **`rhwp-desktop/**`** 추가(기존 `rhwp-firefox/**`·`rhwp-vscode/**` 형제 패턴 정합).
- 데스크톱 전용 변경이 `main` 에 들어와도 Pages 재배포 미트리거.

---

## 3. 검증 결과

| # | 검증 | 결과 |
|---|------|------|
| 1 | macOS 번들 `npm run build`(=`tauri build`) | **exit 0**, `Finished release in 47.41s`, `HanPage.app` + **`HanPage_0.7.13_aarch64.dmg`(35M)** 생성 ✓ |
| 2 | 크로스플랫폼 copy-wasm(node) | 번들 빌드의 `build:frontend` 단계에서 정상 동작(`public/` 동기화 → 데스크톱 dist 빌드 성공) ✓ |
| 3 | `public/rhwp.js` | copy-wasm churn **revert** → clean ✓ |
| 4 | 루트 격리 `cargo metadata` | 워크스페이스 멤버 = **`rhwp` 1개**(불변) ✓ |
| 5 | 워크플로 YAML 유효성 | `desktop-release.yml` + `deploy-pages.yml` 둘 다 파싱 OK ✓ |
| 6 | 트리거 분리 | 데스크톱=`desktop-v*` 태그, CLI=`v*` 태그, Pages=`main` push → **상호 분리** ✓ |
| 7 | 웹 빌드 무영향 | Stage 4 는 rhwp-studio 소스 **무변경**(rhwp-desktop + .github 만) → 웹 번들 정의상 불변(Stage 3 §3 증빙 유효) ✓ |

> **Windows `.exe` 실제 산출**: macOS 로컬 빌드 불가 → CI(GitHub) 영역. 작업지시자가 `desktop-v*`
> 태그 push 또는 Actions 수동 dispatch 로 트리거하면 `windows-latest` 러너가 NSIS `.exe` 를 산출한다.
> (본 보고서는 워크플로 정의·YAML 유효성·로컬 macOS 번들까지 정적 검증 완료.)

---

## 4. 생성 / 수정 파일

**신규** (3건):
- `.github/workflows/desktop-release.yml` — macOS/Windows 번들 CI(`desktop-v*` 태그).
- `mydocs/plans/task_m100_1_impl_v2.md` — Stage 4 개정 구현계획서.
- `mydocs/working/task_m100_1_stage4.md` — 본 보고서.

**수정** (3건):
- `rhwp-desktop/src-tauri/tauri.conf.json` — `bundle.targets: "all"`.
- `rhwp-desktop/package.json` — `copy-wasm` 크로스플랫폼(node).
- `.github/workflows/deploy-pages.yml` — `paths-ignore` 에 `rhwp-desktop/**`.

**커밋 제외**(의도): `rhwp-studio/public/rhwp.js`(copy-wasm 재동기화 — revert, Stage 1~3 동일).

---

## 5. Pages 무영향 증빙 요약 (워크플로 레벨)

| 워크플로 | 트리거 | 데스크톱 영향 |
|----------|--------|--------------|
| `deploy-pages.yml` | `main` push, `paths-ignore: rhwp-desktop/**` | 데스크톱 변경 시 **미트리거** |
| `desktop-release.yml` | `desktop-v*` 태그 / dispatch | Pages 와 트리거 분리(상호 무영향) |
| `release-binary.yml` | `v*` 태그 | 태그 패턴 분리(충돌 없음) |

→ 데스크톱 빌드/배포는 Pages 배포 경로와 트리거·산출물 모두 분리. Pages 배포 산출물 불변.

---

## 6. 다음 단계

- 최종 결과 보고서(`mydocs/report/task_m100_1_report.md`) + `orders/` 갱신.
- (작업지시자) `desktop-v*` 태그 push → CI Windows `.exe`/macOS `.dmg` 산출 확인.
- (승인 후) 이슈 #1 클로즈.

---

## 7. 리스크 / 미해결

- **Windows `.exe` 산출 검증**: CI 트리거(태그/dispatch) 후 실제 러너 빌드 성공·NSIS 설치 동작은
  GitHub Actions 실행 영역(로컬 정적 검증 범위 밖). 첫 트리거 시 tauri-action/wasm-pack 러너 호환
  확인 필요.
- **코드 서명·공증 미적용**(범위 밖): 미서명 → macOS Gatekeeper "확인되지 않은 개발자" / Windows
  SmartScreen 경고. 우클릭 열기/추가 실행으로 우회 가능. 후속 과제.
- **GUI 시각 확인**(작업지시자 영역): `.dmg` 설치·실행, 파일 연결 더블클릭, 메뉴/창 복원/최근 문서.
- Linux 인스톨러·universal/Intel mac·자동 업데이트는 범위 밖(유지).

---

## 승인 요청

Stage 4(macOS `.dmg` 로컬 산출 + Windows NSIS CI 워크플로 + Pages 무영향 가드) 완료.
**최종 결과 보고 및 이슈 #1 마무리 승인을 요청합니다.**
