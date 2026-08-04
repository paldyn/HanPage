# Task #7 (HanPage) — Windows 창 내부 네이티브 메뉴 제거: 최종 결과 보고서

- 이슈: [paldyn/HanPage#7](https://github.com/paldyn/HanPage/issues/7) · 브랜치: `local/task7` (main 기준)
- 마일스톤: M100 (v1.0.0) — 데스크톱 후속, Task #5(아이콘)/Task #4(서명)와 **독립**
- 계획서: `mydocs/plans/task_m100_hp7.md` · Stage 1 보고서: `mydocs/working/task_m100_hp7_stage1.md`
- 상태: **구현·로컬 검증 완료 → main 머지 PR + 이슈 클로즈 승인 대기**

## 1. 목표 / 수용 기준 달성

| 수용 기준 | 결과 |
|-----------|------|
| Windows/Linux: 창 내부 네이티브 메뉴 **미표시** | ✓ (비-macOS 메뉴 부착 제거) |
| macOS: 시스템 메뉴바 메뉴 **그대로 유지** | ✓ (코드 경로 불변) |
| 열기/저장 등 **기능 회귀 없음** | ✓ (웹 메뉴가 `desktop-bridge`로 독립 구동) |
| 전 플랫폼 **클린 컴파일**(비-macOS unused 0) | ✓ (양쪽 cfg 형상 로컬 `cargo check` 클린) |

## 2. 변경 (단일 파일)

`rhwp-desktop/src-tauri/src/lib.rs` (+16/−5). 네이티브 메뉴를 **macOS 시스템 메뉴바 전용**으로 한정. 원인: `app.set_menu()`가 macOS에선 시스템 메뉴바로 가지만 Win/Linux는 창 내부에 렌더 → 웹 UI 메뉴(`#menu-bar`)와 중복.

`#[cfg(target_os = "macos")]` 가드 5개:

| 위치 | 항목 |
|------|------|
| L28 | `use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder}` |
| L65 | `struct RecentEntry` (메뉴 표시 전용) |
| L105 | `fn load_recent` |
| L158 | `fn build_app_menu` |
| L336 | `setup` 내 메뉴 부착 블록(`load_recent`→`build_app_menu`→`set_menu`) |

**무조건 유지(전 플랫폼)**: `manage(PendingDocuments)`, single-instance 파일 더블클릭 열기, `cmd_open/save/take_pending`, `open_path`, `read_document`, `record_recent`, 플러그인. → 데스크톱 핵심 기능 보존.

**설계 결정 — `on_menu_event`/`EVT_MENU` 무조건 유지**: 빌더 체인을 깨지 않기 위함. 클로저가 크로스플랫폼 심볼(`open_path`/`emit`/`EVT_MENU`)만 참조 → 전 플랫폼 컴파일, 비-macOS에선 메뉴 부재로 **이벤트 미발생(inert)**. `EVT_MENU` 사용처가 유지되어 unused 경고도 없음(계획서의 orphan 방지 의도를 가드 없이 충족).

## 3. 검증 — 양쪽 cfg 형상 로컬 컴파일 클린

호스트 = Apple Silicon(`aarch64-apple-darwin`):

1. **macOS 형상**: `cargo check` 클린(0 error/0 warning), 메뉴 코드 유지 → macOS 동작 불변(동일 statements 실행).
2. **비-macOS 형상**: 가드 5개를 일시 `not(target_os = "macos")`로 **반전**해 host에서 `cargo check` → 메뉴 부재(Win/Linux) 토큰셋 컴파일 결과 **클린, orphan/unused 0**. 원복 검증(`not(macos)` 0 / `macos` 5, 최종 macOS check 클린).

> `RecentEntry`/`load_recent`/`build_app_menu`/menu 타입은 오직 메뉴 클러스터 내부에서만 참조(grep 확인)되어, 클러스터 가드 시 비-macOS에서 완전히 사라지고 잔여 참조가 없다.

## 4. 남은 검증 (외부 — CI 의존)

- **실제 Windows 타깃 빌드**: 호스트가 mac이라 직접 불가 → 차기 **`desktop-v*` 릴리스 CI 빌드**(`desktop-release.yml`) 산출물에서 컴파일 + 작업지시자 시각 확인(창 내부 메뉴 사라짐).
- rhwp-desktop tauri 크레이트는 루트 워크스페이스 **비멤버**(격리) → 루트 `cargo`/PR CI는 이 크레이트를 컴파일하지 않으므로, 권위 검증은 릴리스 CI에 의존.

## 5. 무영향 (불변 제약)

- 변경: `rhwp-desktop/src-tauri/src/lib.rs` **1파일** + 문서(`mydocs/`).
- GitHub Pages(web)·**rhwp 엔진 식별자**·아이콘(Task #5)/서명(Task #4)·CI 워크플로(`desktop-release.yml`/`deploy-pages.yml`) **불변**.
- 프런트(`rhwp-studio`)·web-inert 브리지 **무변경** — 웹 메뉴가 이미 열기/저장 독립 구동.

## 6. 브랜치 커밋 이력 (`local/task7`)

| 커밋 | 내용 |
|------|------|
| `cfda67f2` | 수행계획서 + 오늘할일 |
| `5deaaae2` | **Stage 1 — 네이티브 메뉴 macOS 한정(#[cfg] 가드)** + Stage 1 보고서 |

## 7. 다음 (Stage 3 — 승인 게이트)

- [ ] (작업지시자 승인) `local/task7` → `main` 머지 PR(`task7-menu`) 생성·머지.
- [ ] (작업지시자 승인) 이슈 #7 클로즈.
- 다운로드 사용자 반영은 차기 `desktop-v*` 릴리스 CI 빌드 시 자동 포함.
