# Task #1 · Stage 3 완료 보고서 — 파일 연결 + 네이티브 메뉴바 + 최근 문서 + 창 상태

- **이슈**: #1 (M100, v1.0.0)
- **브랜치**: `local/task1` (base `main`)
- **Stage**: 3 / 4 — `.hwp`/`.hwpx` 파일 연결 + 네이티브 메뉴바 + 최근 문서 + 창 상태 저장/복원
- **계획서**: `mydocs/plans/task_m100_1.md`(수행), `mydocs/plans/task_m100_1_impl.md`(구현 §3)
- **일자**: 2026-05-30

---

## 1. Stage 3 목표

데스크톱 앱에 OS 통합 UX 4종을 추가한다.

1. **파일 연결** — Finder/탐색기에서 `.hwp`/`.hwpx` 더블클릭 → HanPage 가 해당 문서로 열림.
2. **네이티브 메뉴바** — 파일/편집/보기 메뉴를 OS 네이티브로 제공, 기존 rhwp-studio 커맨드에 위임.
3. **최근 문서** — 열었던 문서 경로를 영속화하고 시작 시 메뉴에 노출(재실행 시에도 유지).
4. **창 상태** — 창 크기/위치를 종료 시 저장하고 재실행 시 복원.

**불변 제약(유지)**: GitHub Pages(웹 빌드)에 일절 영향을 주지 않는다. rhwp-studio 에 추가하는
모든 코드는 브라우저에서 **완전한 no-op**(web-inert)이며, `@tauri-apps/*` npm 의존을 추가하지 않는다.

---

## 2. 수행 내용

### 2.1 파일 연결 (`tauri.conf.json` + `src-tauri/src/lib.rs`)

- **`tauri.conf.json`** `bundle.fileAssociations` 에 `hwp`/`hwpx` 2종 등록(role: Editor).
  번들 시 macOS `Info.plist`(CFBundleDocumentTypes), Win/Linux 연결 메타데이터 생성.
- **수신 경로(플랫폼별)**:
  - **macOS**: `RunEvent::Opened { urls }` 콜백에서 `url.to_file_path()` → 경로 추출
    (`#[cfg(macos/ios/android)]` 가드). 콜드/웜 스타트 모두 이 이벤트로 들어온다.
  - **Win/Linux**: 2번째 실행의 `argv` 를 `tauri-plugin-single-instance` 가 캡처 →
    `.hwp`/`.hwpx` 확장자만 필터링해 기존 창으로 전달(중복 실행 방지 + 창 포커스).
- **펜딩 큐 핸드오프**: 읽은 바이트를 `PendingDocuments(Mutex<Vec<OpenedFile>>)` 에 적재 →
  `app.emit(EVT_DOCS_READY)` 로 웹뷰에 신호. 웹뷰가 아직 준비 전(콜드 스타트)이어도 문서를
  잃지 않는다. 프런트는 init 시 1회 + 신호 수신 시 `cmd_take_pending_documents` 로 드레인.

### 2.2 네이티브 메뉴바 (`src-tauri/src/lib.rs` — `build_app_menu`)

Rust 에서 메뉴를 만들고, **사용자 정의 항목 id 를 rhwp-studio 커맨드 id 와 동일**하게 둔다.
클릭 시 `on_menu_event` 가 id 를 `EVT_MENU` 로 emit → 프런트 브리지가 그대로 `dispatch`.

| 메뉴 | 항목(커맨드 id) | 구현 |
|------|----------------|------|
| HanPage(앱) | About / Quit | predefined(`.about(None)`/`.quit()`) |
| 파일 | `file:new-doc`, `file:open`, `file:save`, `file:save-as`, 최근 문서▸ | 사용자 정의 → 커맨드 위임(열기/저장은 Stage 2 흐름 재사용) |
| 편집 | `edit:undo/redo/cut/copy/paste/select-all` | 사용자 정의 → 커맨드 위임 |
| 보기 | `view:zoom-in/zoom-out/zoom-100`, 전체 화면 | 줌은 커맨드 위임, 전체 화면은 predefined(`.fullscreen()`) |

### 2.3 최근 문서 (`tauri-plugin-store`)

- `record_recent()` — `recent.json` store 에 `{path,name}` 을 중복 제거 후 맨 앞에 추가,
  `RECENT_MAX(10)` 개로 truncate, 즉시 `save()`. 열기 dialog·파일 연결·최근 문서 클릭 공통.
- `load_recent()` — 시작 시 store 를 읽어 메뉴 "최근 문서" 하위에 구성(id=`recent:<경로>`).
- `on_menu_event` 에서 `recent:` 접두어를 분해해 Rust 측에서 직접 `open_path()`(펜딩 큐 경유).
  목록이 비면 `(없음)` 비활성 항목(id=`recent:__none__`) 노출.

### 2.4 창 상태 (`tauri-plugin-window-state`)

`Builder::default().build()` 플러그인 등록만으로 창 크기/위치를 종료 시 저장·재실행 시 복원.

### 2.5 web-inert 브리지 확장 (`rhwp-studio/src/core/desktop-bridge.ts`)

Stage 2 의 열기/저장 핸들러는 그대로 두고, 네이티브 **푸시 2종**을 추가 연결한다.

- `TauriListen` 타입 + `window.__TAURI__.event.listen` 전역(역시 `@tauri-apps` import 무).
- `DesktopBridgeDeps { openDocument, dispatchCommand }` 를 `initDesktopBridge(deps?)` 로 주입.
- **펜딩 드레인** — `cmd_take_pending_documents` 호출 → `openDocument` 로 오픈. init 시 1회
  (콜드 스타트) + `EVT_DOCS_READY` 수신 시(웜 스타트).
- **메뉴 명령** — `EVT_MENU` 페이로드(커맨드 id) → `dispatchCommand`.
- 브라우저에서는 `deps` 가 주어져도 `isDesktopRuntime()` 가드에서 **즉시 return**(no-op 불변).

### 2.6 통합 지점 (`rhwp-studio/src/main.ts`)

`initDesktopBridge()` 1줄을 `initDesktopBridge({ openDocument, dispatchCommand })` 로 확장.
- `openDocument` = `eventBus.emit('open-document-bytes', { bytes, fileName, fileHandle: null })`
  — **`skipUnsavedGuard` 생략** → 리스너의 unsaved-guard 가 적용된다(파일 연결/최근 문서는
  사용자의 명시적 dialog 액션이 아니므로 미저장 보호가 필요. 메뉴 `file:open` 의 dialog 경로는
  Stage 2 에서 이미 가드 후 `skipUnsavedGuard: true` 로 구분됨).
- `dispatchCommand` = `dispatcher.dispatch(id)`(기존 커맨드 디스패처 재사용).

---

## 3. 검증 결과

| # | 검증 | 결과 |
|---|------|------|
| 1 | 웹 빌드 `npm run build` | `tsc` 클린, 109 모듈, **PWA on**(sw.js/workbox/manifest 생성) ✓ |
| 2 | 웹 번들 web-inert | dist 내 **`@tauri-apps` 0건**, `__TAURI_INTERNALS__` 가드만 포함 ✓ |
| 3 | `public/rhwp.js` | 웹 빌드는 미변경(copy-wasm 미실행) → **clean 유지** ✓ |
| 4 | 데스크톱 빌드 `build:frontend` | `tsc` 클린, 109 모듈, **PWA off**(sw.js/workbox/manifest 부재) ✓ |
| 5 | copy-wasm churn | `public/rhwp.js` 재동기화분 **revert**(Stage 1/2 동일) ✓ |
| 6 | `cargo clippy --all-targets` | **클린**(경고 0), Stage 3 플러그인 4종 정상 컴파일 ✓ |
| 7 | 루트 격리 `cargo metadata` | 워크스페이스 멤버 = **`rhwp` 1개**, 전체 그래프에 tauri/dialog/store/window-state/single-instance/fs/rfd/wry/muda **전부 부재** ✓ |

> **type 안전성**: 웹·데스크톱 양쪽 빌드의 `tsc` 가 클린이므로 `desktop-bridge.ts`/`main.ts`
> 변경이 두 타깃 모두에서 타입 정합.

---

## 4. 생성 / 수정 파일

**신규** (1건): `mydocs/working/task_m100_1_stage3.md`(본 보고서).

**수정** (5건):
- `rhwp-desktop/src-tauri/Cargo.toml` — store/window-state/serde/serde_json + (desktop)single-instance.
- `rhwp-desktop/src-tauri/src/lib.rs` — 파일 연결·메뉴·최근 문서·창 상태·펜딩 큐 + `cmd_take_pending_documents`.
- `rhwp-desktop/src-tauri/tauri.conf.json` — `bundle.fileAssociations`(hwp/hwpx).
- `rhwp-studio/src/core/desktop-bridge.ts` — `TauriListen`/`DesktopBridgeDeps` + 펜딩 드레인·메뉴 리스너.
- `rhwp-studio/src/main.ts` — `initDesktopBridge` 에 deps 주입(open/dispatch 연결).

**커밋 제외**(의도): `rhwp-studio/public/rhwp.js`(copy-wasm 재동기화 — revert, Stage 1/2 §동일).

---

## 5. 설계 결정 (deviation / 근거)

| 결정 | 내용 | 근거 |
|------|------|------|
| 최근 문서 메뉴 = **시작 시 빌드** | 런타임 동적 갱신이 아닌 재실행 시 반영 | 워커 스레드에서의 `set_menu` 스레드 안전성 회피. 검증 요구("재실행 시 노출") 충족 |
| 메뉴 항목 **가속기 없음** | 사용자 정의 항목에 단축키 미부여 | 스튜디오의 **문맥 인지 키보드 핸들러**(입력 필드 복사/붙여넣기, 캔버스 편집)를 메뉴가 가로채지 않도록. predefined(Quit/전체화면)만 표준 단축키 |
| 편집 메뉴 = **커맨드 위임** | predefined Edit 대신 `edit:*` 커맨드 | predefined 는 OS 클립보드로 동작 → 캔버스 에디터의 undo/copy 를 깨뜨림. 스튜디오 커맨드로 위임해야 정상 동작 |
| 의존성 정련(§2 계획 대비) | `@tauri-apps` npm·`tauri-plugin-fs` 미도입, `window.__TAURI__` 전역 + Rust `std::fs` | Stage 2 와 동일 기조. rhwp-studio `package.json` 무변경 = 최대 web-inert |
| `core:default` 유지 | capabilities 무변경 | 앱 자체 command(`#[tauri::command]`)·`event.listen`(core:event:default 포함) 모두 추가 권한 불요 |

---

## 6. Pages 무영향 증빙 요약

| 항목 | 웹(브라우저) | 데스크톱(Tauri) |
|------|-------------|----------------|
| `initDesktopBridge(deps)` | `isDesktopRuntime()` false → **즉시 return**(no-op) | 핸들러 등록 + 펜딩 드레인 + 리스너 |
| 펜딩 드레인 / 메뉴 리스너 | 미실행(가드 뒤) | `cmd_take_pending_documents` / `EVT_MENU` |
| `@tauri-apps` npm 의존 | **없음**(전역만 사용) | 〃 |
| 웹 번들 Tauri 코드 | **미포함**(dist 내 0건) | 〃 |
| rhwp-studio `package.json` | **무변경** | 〃 |
| 루트/WASM 빌드 그래프 | tauri 계열 **전부 부재** | src-tauri 독립 크레이트 |

→ rhwp-studio 추가 코드는 런타임 가드(`__TAURI_INTERNALS__`) 뒤에만 동작. 웹/Pages 배포 산출물 불변.

---

## 7. 다음 단계 (Stage 4 예고)

- macOS `.dmg`/`.app` 번들 산출(`tauri build`) + 아이콘.
- `deploy-pages.yml` `paths-ignore` 에 `rhwp-desktop/**` 추가(워크플로 레벨 Pages 격리).
- Pages 무영향 회귀 매트릭스 + 최종 결과 보고서(`mydocs/report/task_m100_1_report.md`) + orders 갱신.
- `Cargo.lock` 커밋 여부 재검토.

---

## 8. 리스크 / 미해결

- **GUI 시각 확인**(작업지시자 육안 검증 영역): Finder 더블클릭으로 문서 오픈, 메뉴 동작,
  창 크기/위치 복원, 재실행 시 최근 문서 노출. 본 보고서는 빌드/격리/무영향 정적 검증까지 완료.
- **파일 연결 실제 동작**은 번들(.app) 설치 후 OS 에 등록되어야 확인 가능 → Stage 4(번들) 후 검증.
- **대용량 파일**: 바이트를 JSON 배열로 직렬화(invoke 경유). 일반 HWP 충분, 초대용량 최적화는 후속.
- Win/Linux 인스톨러·코드서명·공증·자동 업데이트는 범위 밖(유지).

---

## 승인 요청

Stage 3(파일 연결 + 네이티브 메뉴바 + 최근 문서 + 창 상태 + web-inert 브리지 확장 +
Pages 무영향 검증) 완료. **Stage 4 진행 승인을 요청합니다.**
