# Task #1 구현 계획서 — HanPage 데스크톱 앱 (Tauri 1단계)

수행 계획서: [`task_m100_1.md`](task_m100_1.md)

파일 전달 방식: **(A) web-inert 가드 브리지 훅** 확정 (작업지시자 승인).

## 1. 구현 범위

`rhwp-studio` 빌드 산출물 + WASM을 Tauri v2 셸로 감싸 데스크톱 앱(표시명 "HanPage")을 만든다. 엔진(WASM) 무수정. macOS `.dmg` 산출까지가 1단계 완료 기준.

## 2. 의존성 (신규)

### Rust (`rhwp-desktop/src-tauri/Cargo.toml` — 루트 워크스페이스 미포함 독립 크레이트)
- `tauri` (v2)
- `tauri-plugin-dialog`
- `tauri-plugin-fs`
- `tauri-plugin-store`
- `tauri-plugin-window-state`
- `tauri-plugin-single-instance` (Win/Linux 파일 연결 argv 캡처)

### npm (`rhwp-desktop/package.json`)
- `@tauri-apps/cli` (devDep)
- `@tauri-apps/api`, `@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-fs`, `@tauri-apps/plugin-store`, `@tauri-apps/plugin-window-state`

> `rhwp-studio` 의 런타임 의존성은 추가하지 않는다(브리지 훅은 `@tauri-apps/api`를 **동적 import + 가드**로만 사용해 웹 번들에 정적 포함되지 않게 한다).

## 3. 기존 코드 조사 지점 (구현 중 확정)

1단계 통합을 위해 구현 시작 시 확인할 기존 rhwp-studio 코드:
- **C1. WASM 로딩 경로** — `rhwp-studio/src/wasm-bridge.ts`(추정)가 `rhwp_bg.wasm`을 절대경로(`/...`)로 fetch하는지. `base:'./'` 데스크톱 빌드와 호환되는지 (Stage 1).
- **C2. 문서 오픈 진입점** — 메인 앱이 `ArrayBuffer`/`File`로 문서를 여는 내부 함수(파일 input 핸들러 등). 브리지가 호출할 대상 (Stage 2).
- **C3. 저장/내보내기 경로** — 현재 `exportHwp`/`exportHwpx`(WASM) 결과를 브라우저 다운로드로 흘리는 지점. 네이티브 저장으로 분기할 후크 위치 (Stage 2).

> C1~C3은 진단 후 그 결과를 해당 Stage 보고서에 기록한다. 가정과 다르면 Stage 계획을 조정해 재승인 요청.

## 4. 구현 단계 (4 Stage)

### Stage 1 — Tauri 스캐폴드 + rhwp-studio dist 로드

**1-1. `rhwp-desktop/` 골격 + Tauri v2 init**
- `rhwp-desktop/{package.json, src-tauri/}` 생성
- `src-tauri/tauri.conf.json`:
  - `productName: "HanPage"`, `identifier: "com.paldyn.hanpage"`, `version`(= studio 0.7.13 연동 또는 독립)
  - `build.frontendDist: "../rhwp-studio/dist"`
  - `build.beforeBuildCommand`: WASM 복사 + studio 데스크톱 빌드 (1-2)
  - `build.devUrl` / `beforeDevCommand`: studio dev 서버(127.0.0.1:7700)
  - `app.windows[0]`: title "HanPage", 최소 크기, `withGlobalTauri` 검토
  - `app.security.csp`: WASM 실행 위해 `script-src 'self' 'wasm-unsafe-eval'` + canvaskit 허용

**1-2. 데스크톱 빌드 분기 (PWA off + base 조정)** *(rhwp-studio 수정 — web 동작 불변)*
- `rhwp-studio/vite.config.ts`: `process.env.VITE_TARGET === 'desktop'` 일 때만 `VitePWA` 제외 (웹은 기존대로 PWA 포함)
- `rhwp-studio/package.json`: `"build:desktop": "tsc && VITE_TARGET=desktop vite build --base=./"` 스크립트 추가
- 웹 기본 빌드(`build`)는 **무변경** → GitHub Pages 산출물 동일

**1-3. WASM 로딩 + CSP 검증 (C1)**
- 데스크톱 빌드 산출물을 Tauri 웹뷰에 로드하여 `rhwp_bg.wasm` 정상 로드·실행 확인
- 절대경로 fetch 문제 시: 로딩부 상대경로화 또는 Tauri asset 프로토콜 매핑 (최소 수정, web-inert)

**1-4. 워크스페이스 격리 확인**
- 루트 `cargo build` / `cargo build --release` 결과가 `src-tauri` 추가 전후 **불변** 확인 (루트에 `[workspace]` 신설 금지)

**검증**: 빈 HanPage 윈도우에 rhwp-studio UI가 렌더되고 WASM 초기화 성공.
**산출물**: `mydocs/working/task_m100_1_stage1.md` (+ 스크린샷)

---

### Stage 2 — 네이티브 열기/저장 dialog + 파일 전달 브리지 (web-inert)

**2-1. 플러그인 등록**
- `tauri-plugin-dialog`, `tauri-plugin-fs` + `src-tauri/capabilities/*.json` 권한 부여 (dialog, fs read/write 범위 한정)

**2-2. 열기 흐름 (네이티브 → 웹)**
- Rust: 열기 dialog → 선택 path → 파일 bytes read → webview로 `open-file` 이벤트(emit). 대용량 IPC 회피 위해 **path 전달 + 웹에서 `plugin-fs readFile`** 우선, 필요 시 bytes 직접 전달로 대체

**2-3. 데스크톱 브리지 훅 (A안 — C2)** *(rhwp-studio 수정 — web-inert)*
- `rhwp-studio/src/desktop-bridge.ts` 신규: `if (!('__TAURI_INTERNALS__' in window)) return;` 가드 → 가드 통과 시에만 `@tauri-apps/api`를 **동적 import**
- `open-file` 수신 → bytes → **기존 문서 오픈 함수(C2)** 호출
- `main.ts`에서 `desktop-bridge` 초기화 1줄 호출 (브라우저에선 즉시 return = no-op)

**2-4. web-inert 검증**
- 브라우저(웹 빌드)에서 브리지가 완전 no-op이고 기존 동작·번들 거동 동일함을 확인 (동적 import가 정적 번들에 끌려오지 않는지 포함)

**2-5. 저장 흐름 (C3)**
- 기존 `exportHwp`/`exportHwpx`(WASM) 결과 bytes → 저장 dialog → `plugin-fs writeFile`
- 데스크톱에선 브라우저 다운로드 대신 네이티브 저장으로 분기 (가드된 후크)

**검증**: 앱 메뉴/dialog로 `samples/`의 .hwp/.hwpx 열기 → 웹과 동일 렌더. 편집 후 저장 → 파일 생성. 웹 빌드 동작 불변.
**산출물**: `mydocs/working/task_m100_1_stage2.md`

---

### Stage 3 — 파일 연결 + 메뉴바 + 최근문서 + 윈도우 상태

**3-1. 파일 association (.hwp/.hwpx 더블클릭)**
- `tauri.conf.json` `bundle.fileAssociations`: `[{ext:"hwp"}, {ext:"hwpx"}]` (+ name/role/mimeType)
- macOS: `tauri::RunEvent::Opened { urls }` 처리 → 2-2 열기 흐름 재사용
- Win/Linux: `tauri-plugin-single-instance`로 argv 캡처 → 2-2 흐름. 2번째 인스턴스는 기존 창에 전달

**3-2. 메뉴바**
- Tauri `Menu`: 파일(열기/저장/다른이름으로저장/최근문서/종료), 편집(실행취소/복사/붙여넣기 — 웹 표준 위임), 보기(확대/축소/전체화면)
- 메뉴 액션 → 기존 studio 기능 또는 2장 흐름에 연결

**3-3. 최근문서 + 윈도우 상태**
- `tauri-plugin-store`: 최근 연 파일 path 목록 저장/표시 (메뉴 하위)
- `tauri-plugin-window-state`: 창 크기/위치 저장·복원

**검증**: Finder에서 .hwp 더블클릭 → HanPage 실행·문서 표시. 메뉴 동작. 재실행 시 창 위치 복원·최근문서 노출.
**산출물**: `mydocs/working/task_m100_1_stage3.md`

---

### Stage 4 — 인스톨러 산출 + GitHub Pages 무영향 검증 + 최종 보고

**4-1. Pages 무영향 가드** *(deploy 워크플로 수정)*
- `.github/workflows/deploy-pages.yml` `paths-ignore`에 `rhwp-desktop/**` 추가
- 데스크톱 전용 변경 push 시 배포 미트리거 확인 (워크플로 dry 검토)

**4-2. 인스톨러 빌드**
- `tauri build` → macOS `.dmg` 산출 (1단계 1차 타깃)
- Windows `.msi` / Linux `.deb`: 해당 OS에서 빌드 가능함을 문서화 (실제 산출은 해당 OS/후속 CI=2단계)
- 아이콘: HanPage 아이콘 세트 생성(`tauri icon`)

**4-3. 무영향 회귀 검증 (필수)**
| 검증 | 기준 |
|------|------|
| 루트 `cargo build`/`--release` | `src-tauri` 추가 전후 산출물 불변 |
| WASM 빌드 | 산출물 불변 |
| rhwp-studio **웹** 빌드 | 브라우저 동작·번들 거동 동일 (브리지 web-inert) |
| `deploy-pages.yml` | `rhwp-desktop/**` paths-ignore 동작 |
| 엔진 식별자 | crate `rhwp`/`@rhwp/*`/Edward Kim 저작권 불변 |

**4-4. 최종 보고서**
- `mydocs/report/task_m100_1_report.md`: 아키텍처·산출물·무영향 검증 결과·잔존/2단계 권고

**검증**: `.dmg` 설치·실행 정상. 무영향 매트릭스 전 항목 통과.
**산출물**: `mydocs/working/task_m100_1_stage4.md` + `mydocs/report/task_m100_1_report.md`

## 5. 회귀 risk 매트릭스

| 변경 영역 | 영향 | risk | 완화책 |
|----------|------|------|--------|
| `rhwp-desktop/**` 신규 | 데스크톱만 | 낮음 | 독립 크레이트·디렉터리 |
| `vite.config.ts` PWA 분기 | 웹/데스크톱 빌드 | 중 | 웹 기본 빌드 무변경, `VITE_TARGET` 가드 |
| `desktop-bridge.ts` + `main.ts` 1줄 | 웹 런타임 | 중 | `__TAURI_INTERNALS__` 가드 + 동적 import + web-inert 검증 |
| `deploy-pages.yml` paths-ignore | 배포 트리거 | 낮음 | 추가만(기존 패턴 유지) |
| `src-tauri` 크레이트 | 루트 빌드 | 낮음 | workspace 미신설, 격리 검증(1-4) |

## 6. 단계별 산출물 / 커밋 단위

- 계획서 2종(`task_m100_1.md`, `task_m100_1_impl.md`) → 승인 후 1 커밋 (`Task #1: 수행·구현 계획서`)
- 각 Stage 완료 시: 소스 + `task_m100_1_stage{N}.md` → 1 커밋, 작업지시자 승인 후 다음 Stage
- 최종: `task_m100_1_report.md` + `orders/` 갱신 커밋

## 7. 잔존 분리 (Out of Scope — 2단계 이후)

- 네이티브 `rlib` 코어 직접 호출 (현재 WASM 유지)
- 자동 업데이트 / 코드 서명·공증
- Win/Linux 인스톨러 실제 산출 + CI 릴리스 매트릭스
- 외부 참조 이미지의 로컬 파일시스템 해석
- 딥링크 / 다중 윈도우 / 탭
