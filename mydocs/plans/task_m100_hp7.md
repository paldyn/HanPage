# Task #7 (HanPage) — Windows 창 내부 네이티브 메뉴 제거 (수행계획서)

> **파일명 규칙**: fork-native 과제 `task_m100_hp{이슈}.md` (Task #4/#5와 동일 네임스페이스).
> 단일 파일(`lib.rs`) cfg 가드라 **수행+구현을 본 문서에 통합**하고 별도 `_impl.md`는 생략한다(hp5 선례).

- 이슈: [paldyn/HanPage#7](https://github.com/paldyn/HanPage/issues/7)
- 브랜치: `local/task7` (main 기준 분기 — 메뉴 코드는 main `lib.rs`에 존재)
- 마일스톤: M100 (v1.0.0) — 데스크톱 후속, Task #5(아이콘)/Task #4(서명)와 **독립**
- 상태: **수행계획 작성 → 승인 요청**

## 1. 배경

Windows 빌드에서 Tauri 네이티브 메뉴(`HanPage | 파일 | 편집 | 보기`)가 **창 내부 상단**에 렌더되어, 바로 아래 웹 UI 메뉴바(`#menu-bar`)와 **중복**된다(작업지시자 스크린샷의 빨간 영역). macOS는 같은 메뉴가 화면 상단 **시스템 메뉴바**로 들어가 자연스럽지만, Windows/Linux는 메뉴를 창 안에 그리므로 중복·이질감이 생긴다.

## 2. 목표 / 수용 기준

- [ ] Windows/Linux: 창 내부 네이티브 메뉴 **미표시**
- [ ] macOS: 시스템 메뉴바 메뉴 **그대로 유지**
- [ ] 열기/저장 등 **기능 회귀 없음**(웹 메뉴가 독립 구동)
- [ ] 전 플랫폼 **클린 컴파일**(비-macOS unused 경고 0)

## 3. 기술 접근 (확정)

핵심: 네이티브 메뉴를 **macOS 전용 기능**으로 한정한다. `app.set_menu()`는 macOS에선 시스템 메뉴바에 들어가지만 Win/Linux에선 창 내부에 그려지므로, 비-macOS에서 **메뉴 부착 자체를 제거**한다.

대상 파일: **`rhwp-desktop/src-tauri/src/lib.rs` (1파일)**

| 위치 | 항목 | 처리 |
|------|------|------|
| L326 `setup` | `load_recent` → `build_app_menu` → `app.set_menu(menu)?` 3줄 | `#[cfg(target_os = "macos")]` 가드 |
| L153 | `fn build_app_menu(...)` | `#[cfg(target_os = "macos")]` |
| L27 | `use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};` | `#[cfg(target_os = "macos")]` |
| L333 | `.on_menu_event(...)` 핸들러 | macOS 클러스터로 가드(`MenuBuilder`/빌더 체인 분기) |
| L38 | `const EVT_MENU` | 가드 후 사용처가 macOS뿐이면 동일 가드 |
| `load_recent` | 네이티브 메뉴 표시 전용 | 동일 가드(orphan/unused 방지) |

**유지(전 플랫폼 공유)**: `app.manage(PendingDocuments)`, single-instance 파일 연결, `cmd_open/save/take_pending`, `open_path`, `read_document`, `record_recent`, 플러그인(dialog/store/window-state). → 파일 더블클릭 열기·네이티브 dialog 등 **데스크톱 핵심 기능 전부 보존**.

> 원칙: **메뉴 표시 전용 코드 = macOS 한정 / 런타임 파일·커맨드 코드 = 무조건.** 비-macOS는 코드를 *제거*하는 방향이라 컴파일 에러 위험은 낮고, 남는 과제는 unused 경고 관리뿐이다. 정확한 cfg 경계는 구현 시 `cargo`(및 가능 시 cross `cargo check`) 경고를 따라 확정한다.

## 4. 범위 / 무영향 (하드 제약)

- 변경: **`rhwp-desktop/src-tauri/src/lib.rs` 1파일**.
- 무영향: GitHub Pages(web), **rhwp 엔진 식별자**, 아이콘(Task #5)/서명(Task #4), CI 워크플로(`desktop-release.yml`/`deploy-pages.yml`).
- 프런트(`rhwp-studio`)·web-inert 브리지 **무변경** — 웹 메뉴(`#menu-bar`)가 이미 `desktop-bridge.ts`를 통해 열기/저장을 독립 구동(네이티브 메뉴의 `hanpage://menu`는 중복 트리거였을 뿐).

## 5. 단계

| Stage | 내용 | 게이트 |
|-------|------|--------|
| 1 | `lib.rs` cfg 가드 구현 + **macOS 로컬 빌드**(메뉴 유지 확인, 클린 컴파일) | 보고 후 |
| 2 | **비-macOS 컴파일 검증**(로컬 cross `cargo check` 시도 / 불가 시 근거+CI 계획) + 최종 보고 | 보고 후 |
| 3 | main 머지 PR + (승인) 이슈 클로즈 | 각 행위별 승인 |

## 6. 검증 전략

- **macOS**: `cargo build`(또는 `tauri build`) **클린** + 앱 실행 시 시스템 메뉴바 메뉴 정상(회귀 0). 주 사용 플랫폼이므로 최우선.
- **비-macOS**: 호스트가 Apple Silicon이라 Windows **직접 빌드 불가**. 차선:
  - (시도) `cargo check --target x86_64-unknown-linux-gnu` 로 `not(macos)` cfg 컴파일 점검 — 타깃 std 필요, tauri linux sys-dep로 실패할 수 있음(`check`는 링크 불요).
  - (권위) 실제 Windows 검증은 차기 **`desktop-v*` 릴리스 CI 빌드** 산출물 + 작업지시자 시각 확인(창 내부 메뉴 사라짐).
- **회귀**: 열기/저장은 웹 메뉴 → `desktop-bridge` → `cmd_*` 경로로 독립 → macOS에서도 동작 동일.

## 7. 위험·주의

- **비-macOS unused 경고**: CI가 `-D warnings`면 Windows 빌드 실패 가능 → cfg 가드를 메뉴 클러스터로 정확히 한정해 orphan을 남기지 않는다(빌드/`check` 로그로 확인).
- 주 사용 플랫폼 **macOS 회귀 0**이 최우선. 변경은 비-macOS 분기만 실질 제거.
- rhwp-desktop tauri 크레이트는 루트 워크스페이스 **비멤버**(격리) → 루트 `cargo`/PR CI는 이 크레이트를 컴파일하지 않음. 따라서 검증은 `rhwp-desktop/src-tauri`에서 직접 `cargo` 실행 + 릴리스 CI에 의존.
