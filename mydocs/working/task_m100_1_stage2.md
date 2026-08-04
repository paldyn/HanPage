# Task #1 · Stage 2 완료 보고서 — 네이티브 열기/저장 dialog + web-inert 브리지

- **이슈**: #1 (M100, v1.0.0)
- **브랜치**: `local/task1` (base `main`)
- **Stage**: 2 / 4 — 네이티브 열기/저장 dialog 연동 + web-inert 가드 브리지(A안)
- **계획서**: `mydocs/plans/task_m100_1.md`(수행), `mydocs/plans/task_m100_1_impl.md`(구현)
- **일자**: 2026-05-30

---

## 1. Stage 2 목표

데스크톱 앱에서 **네이티브 파일 dialog**로 `.hwp`/`.hwpx` 문서를 열고 저장한다.
기존 rhwp-studio 의 문서 오픈/저장 흐름(엔진·이벤트버스)은 그대로 재사용하고,
네이티브↔웹 파일 핸드오프만 얇은 브리지로 연결한다.

**불변 제약(유지)**: GitHub Pages(웹 빌드)에 일절 영향을 주지 않는다. 본 Stage 가
rhwp-studio 에 추가하는 코드는 브라우저에서 **완전한 no-op**(web-inert)이어야 한다.

---

## 2. 수행 내용

### 2.1 Rust 백엔드 — 앱 command 2종 (`src-tauri/src/lib.rs`)

dialog 와 파일 IO 를 **전부 Rust 측에서** 처리한다.

- **`cmd_open_document`** — `app.dialog().file().add_filter("한글 문서 (HWP/HWPX)", &["hwp","hwpx"]).blocking_pick_file()`
  → 선택 파일을 `std::fs::read` → `OpenedFile { name, path, data: Vec<u8> }` 반환. 취소 시 `Ok(None)`.
- **`cmd_save_document`** — `set_file_name(suggested_name).add_filter("한글 문서 (HWP)", &["hwp"]).blocking_save_file()`
  → 선택 경로에 `std::fs::write` → `SaveOutcome::Saved { path, name }`. 취소 시 `SaveOutcome::Cancelled`.
- `SaveOutcome` 은 `#[serde(tag = "status", rename_all = "camelCase")]` 로 프런트가 `status` 분기.
- `tauri::Builder` 에 `.plugin(tauri_plugin_dialog::init())` + `.invoke_handler(generate_handler![cmd_open_document, cmd_save_document])` 등록.

> **async command + `blocking_*`**: async command 는 메인 스레드가 아닌 async 런타임 워커에서
> 실행되므로 `blocking_pick_file()`(내부적으로 dialog 를 메인 스레드에 디스패치)을 호출해도
> 데드락이 없다. 메인 스레드에서 직접 호출하면 안 된다 — 코드 주석에 명시.

### 2.2 web-inert 가드 브리지 (`rhwp-studio/src/core/desktop-bridge.ts`, 신규)

브라우저에서 **영구 no-op** 인 단일 모듈. `@tauri-apps/*` npm 의존을 **일절 import 하지 않는다**.

- `isDesktopRuntime()` — `'__TAURI_INTERNALS__' in window` 로 Tauri 웹뷰 판별.
- `initDesktopBridge()` — 데스크톱이 아니면 **즉시 return**(no-op). 데스크톱에서만
  `window.__TAURI__.core.invoke`(전역, `withGlobalTauri: true` 로 주입됨)를 통해
  `openHandler` / `saveHandler` 를 채운다.
- `getDesktopOpenHandler()` / `getDesktopSaveHandler()` — 브라우저에선 항상 `null` 반환 →
  `file.ts` 의 가드 분기가 통째로 건너뛰어져 **기존 웹 동작이 그대로** 유지된다.

### 2.3 기존 흐름 연결 — 조사지점 C2 / C3 확정

- **C2 (오픈 진입점)**: `file:open` 커맨드의 unsaved-guard(`if (!canReplace) return;`) **직후**,
  네이티브 핸들러가 있으면 호출 → 받은 바이트를 기존 `eventBus.emit('open-document-bytes', {...})`
  로 흘려보낸다(= 웹과 동일한 문서 로드 경로 재사용). 핸들러 없으면(웹) 기존 File System Access 분기로.
- **C3 (저장 진입점)**: `saveCurrentDocument` 및 `file:save-as` 의 `services.wasm.exportHwp()` **직후**,
  네이티브 핸들러가 있으면 호출 → `status` 로 saved/cancelled/failed 분기. 핸들러 없으면(웹)
  기존 File System Access → 브라우저 다운로드 폴백 경로 그대로.
- 통합 지점 4곳: `main.ts`(import + `initialize()` 내 `initDesktopBridge()` 1줄),
  `file.ts`(import + open 1분기 + save 1분기 + save-as 1분기).

### 2.4 구현계획서 §2 대비 **의존성 정련**(deviation, 의도적)

구현계획서 §2-1 은 `tauri-plugin-dialog` + `tauri-plugin-fs` + `@tauri-apps/api`·`@tauri-apps/plugin-*`
npm 의존을 상정했으나, 다음과 같이 **더 단순·안전하게** 조정했다.

| 항목 | 계획서 §2 | 실제 구현 | 사유 |
|------|----------|----------|------|
| 프런트 Tauri 호출 | `@tauri-apps/api` npm import | `window.__TAURI__.core.invoke` 전역 | rhwp-studio npm 의존 **0 변경** → 웹 번들에 Tauri 코드 정적/동적 모두 미포함 = 최대 web-inert |
| 파일 IO | `tauri-plugin-fs` | Rust `std::fs` | dialog 가 이미 Rust 측 → fs 도 Rust 에서 직접. JS ACL(`fs:*`) 권한 불필요 |
| dialog | `tauri-plugin-dialog` | `tauri-plugin-dialog` (유지) | Rust `DialogExt` 로만 사용 |

→ 결과: **rhwp-studio 의 `package.json` 무변경**, capabilities 무변경(`core:default` 유지).
앱 자체 command(`#[tauri::command]`)는 Tauri v2 ACL 대상이 아니므로 권한 추가가 필요 없다.

---

## 3. 검증 결과

### 3.1 웹 빌드 무변경 = Pages 무영향 (`npm run build`)

- `tsc` **클린**(타입 에러 0), 109 모듈 변환, PWA 산출물 생성(`sw.js`/`workbox-*`/`manifest.webmanifest`).
- 번들 내 **`@tauri-apps` 문자열 0건** ✓ — 브리지가 전역만 쓰므로 Tauri 코드가 웹 번들에 미포함.
- **npm 의존 무변경** ✓ — `package.json`/`package-lock.json` 그대로.

### 3.2 데스크톱 백엔드 컴파일 (`cargo build`)

`src-tauri` **exit 0** — `Finished dev profile in 16.06s`. `tauri-plugin-dialog v2.7.1`
(+ rfd 0.16.0 / tauri-plugin 2.6.2 / tauri-plugin-fs 2.5.1 [dialog 전이 의존]) 정상 컴파일.

### 3.3 데스크톱 프런트 빌드 (`npm run build:frontend`)

PWA off + 상대 base(`./assets/`) 유지. copy-wasm 으로 인한 `public/rhwp.js` 재동기화는
Stage 1 과 동일하게 **커밋 제외**(revert) — 웹 번들 소스라 Pages 영향 차단.

### 3.4 빌드 격리 (`cargo metadata --no-deps`)

루트 워크스페이스 멤버 = **`rhwp` 단 1개**. `tauri-plugin-dialog` 가 **루트 그래프에 부재**(False)
확인 — dialog/fs/rfd 등 네이티브 전용 의존이 루트/WASM 빌드로 새지 않음.

---

## 4. 생성 / 수정 파일

**신규** (1건): `rhwp-studio/src/core/desktop-bridge.ts` — web-inert 브리지(전역 invoke, no-op 가드).

**수정** (4건):
- `rhwp-desktop/src-tauri/Cargo.toml` — `tauri-plugin-dialog = "2"` 추가.
- `rhwp-desktop/src-tauri/src/lib.rs` — `cmd_open_document`/`cmd_save_document` + plugin/handler 등록.
- `rhwp-studio/src/main.ts` — import + `initialize()` 내 `initDesktopBridge()` 1줄.
- `rhwp-studio/src/command/commands/file.ts` — import + open/save/save-as 가드 분기 3곳.

**커밋 제외**(의도): `rhwp-studio/public/rhwp.js`(copy-wasm 재동기화 — Stage 1 §2.4 동일, revert).

---

## 5. Pages 무영향 증빙 요약

| 항목 | 웹(브라우저) | 데스크톱(Tauri) |
|------|-------------|----------------|
| `initDesktopBridge()` | 즉시 return = **no-op** | invoke 핸들러 등록 |
| `getDesktop*Handler()` | 항상 `null` → 가드 건너뜀 | 네이티브 dialog 호출 |
| `@tauri-apps` npm 의존 | **없음**(전역만 사용) | 〃 |
| 웹 번들 Tauri 코드 | **미포함**(정적/동적 모두) | 〃 |
| rhwp-studio `package.json` | **무변경** | 〃 |

→ rhwp-studio 추가 코드는 런타임 가드(`__TAURI_INTERNALS__`) 뒤에만 동작. 웹/Pages 배포 산출물 불변.

---

## 6. 다음 단계 (Stage 3 예고)

- `.hwp`/`.hwpx` 파일 연결(file association) + 네이티브 메뉴바(최소).
- 최근 문서(`tauri-plugin-store`) + 윈도우 상태 저장/복원(`tauri-plugin-window-state`).

---

## 7. 리스크 / 미해결

- **GUI 시각 확인**: `npm run dev` 로 HanPage 창에서 실제 열기/저장 dialog 가 뜨고 문서가
  로드/저장되는지는 작업지시자 육안 검증 영역(프로젝트 관행). 본 보고서는 빌드/격리/무영향
  정적 검증까지 완료.
- **대용량 파일**: 현재 바이트를 JSON 배열로 직렬화(invoke 경유). 일반 HWP(수십 KB~수 MB)는
  충분하나, 초대용량 문서 최적화(스트리밍/경로 핸드오프)는 후속 과제.
- Win/Linux 인스톨러, 코드서명·공증, 자동 업데이트는 범위 밖(유지).

---

## 승인 요청

Stage 2(네이티브 열기/저장 dialog + web-inert 브리지 + C2/C3 확정 + Pages 무영향 검증) 완료.
**Stage 3 진행 승인을 요청합니다.**
