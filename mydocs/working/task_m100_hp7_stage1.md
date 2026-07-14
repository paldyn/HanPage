# Task #7 (HanPage) — Stage 1 완료 보고서: 네이티브 메뉴 macOS 한정 + 로컬 컴파일 검증

- 이슈: [paldyn/HanPage#7](https://github.com/paldyn/HanPage/issues/7) · 브랜치: `local/task7`
- 계획서: `mydocs/plans/task_m100_hp7.md`

## 구현 (`rhwp-desktop/src-tauri/src/lib.rs` 1파일)

네이티브 메뉴를 **macOS 시스템 메뉴바 전용**으로 한정. 비-macOS(Win/Linux)는 메뉴를 창 내부에 그려 웹 UI 메뉴(`#menu-bar`)와 중복되므로 **부착하지 않는다**.

`#[cfg(target_os = "macos")]` 가드 5개 (메뉴 클러스터):

| 위치 | 항목 | 비고 |
|------|------|------|
| L28 | `use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder}` | 메뉴 빌더 임포트 |
| L65 | `struct RecentEntry` | 최근 문서 메뉴 항목(표시 전용) |
| L105 | `fn load_recent` | store→메뉴 구성 |
| L158 | `fn build_app_menu` | 메뉴 생성 |
| L336 | `setup` 내 메뉴 부착 블록 | `load_recent`→`build_app_menu`→`set_menu` 3줄을 `#[cfg] { … }` 로 감쌈 |

### 무조건(전 플랫폼 유지) — 데스크톱 핵심 기능 보존

- `app.manage(PendingDocuments)`, single-instance **파일 더블클릭 열기**, `cmd_open/save/take_pending`, `open_path`, `read_document`, `record_recent`, 플러그인(dialog/store/window-state).
- **`on_menu_event` + `EVT_MENU` 도 무조건 유지** — 빌더 체인을 깨지 않기 위함. 클로저는 크로스플랫폼 심볼(`open_path`/`emit`/`EVT_MENU`)만 참조하므로 전 플랫폼 컴파일되고, 메뉴가 없는 비-macOS에선 **이벤트가 발생하지 않아 무해(inert)**. `EVT_MENU` 사용처가 유지되므로 비-macOS에서 unused 경고도 없음. (계획서의 "orphan/unused 방지" 의도를 가드 없이 충족 → 체인 분해보다 깔끔.)

> 가드 대상 심볼(`RecentEntry`/`load_recent`/`build_app_menu`/menu 타입)은 **오직 클러스터 내부에서만** 참조됨(grep 확인). `record_recent`는 `serde_json` 직접 사용으로 `RecentEntry` 무관 → 공유 유지.

## 검증 — 양쪽 cfg 형상 로컬 컴파일 클린

호스트 = Apple Silicon(`aarch64-apple-darwin`). 두 형상을 모두 로컬에서 컴파일 점검:

1. **macOS 형상**(host 기본): `cargo check` **클린**(0 error/0 warning), 메뉴 코드 컴파일+부착 유지 → macOS 동작 불변(동일 코드 경로).
2. **비-macOS 형상**(Win/Linux): 가드 5개를 일시적으로 `not(target_os = "macos")`로 **반전**해 host에서 `cargo check` → **메뉴 부재(Win/Linux) 토큰셋**을 컴파일. 결과 **클린**(0 error/0 warning) → **orphan/unused 심볼 없음** 입증. 이후 가드 원복(검증: `not(macos)` 0건 / `macos` 5건, 최종 macOS `cargo check` 클린).

- 변경 파일: `rhwp-desktop/src-tauri/src/lib.rs` **1개만** (+16/−5).

## 남은 작업

- **실제 Windows 타깃 빌드 검증**: 호스트가 mac이라 직접 빌드 불가 → 차기 `desktop-v*` 릴리스 CI 빌드 산출물 + 작업지시자 시각 확인(창 내부 메뉴 사라짐). (Stage 2/3)
- 최종 보고 + main 머지 PR + (승인) 이슈 클로즈.

## 무영향 (불변 제약)

- `git diff` 범위: `rhwp-desktop/src-tauri/src/lib.rs` 1파일.
- GitHub Pages(web)·rhwp 엔진 식별자·아이콘(Task #5)/서명(Task #4)·CI 워크플로 **불변**. 프런트/web-inert 브리지 **무변경**(웹 메뉴가 열기/저장 독립 구동).
