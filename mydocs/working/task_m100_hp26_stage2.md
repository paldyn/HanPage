# Task #26 Stage 2 완료 보고서 — 업데이트 흐름 (lib.rs)

- **이슈**: [paldyn/HanPage#26](https://github.com/paldyn/HanPage/issues/26) (M100)
- **단계**: Stage 2 / 4 · **계획서**: `plans/task_m100_hp26_impl.md` §4
- **일자**: 2026-06-09

## 1. 단계 목표

시작 시 자동 확인 + 메뉴 "업데이트 확인" + native 알림(지금 설치/나중에) + 다운로드·설치·재시작.

## 2. 구현 (lib.rs, 모두 `#[cfg(desktop)]`)

- **`check_update(app, manual)` async 함수**:
  - `app.updater()?.check().await` → `Some(update)` 이면 native 대화상자(`업데이트 있음 / 새 버전 vX.Y.Z / [지금 설치][나중에]`).
  - [지금 설치] → `update.download_and_install(...)` → `app.restart()`. 실패 시 오류 안내.
  - `None`: `manual=true`면 "이미 최신 버전입니다", `manual=false`(시작 시)면 무알림.
  - 오류: 로그(시작 시) / 대화상자(수동).
- **시작 확인**: `setup()`에서 `tauri::async_runtime::spawn(check_update(handle, false))` (전 desktop).
- **메뉴**: HanPage 서브메뉴에 `업데이트 확인`(id `app:check-update`) — **macOS 네이티브 메뉴**.
- **핸들러**: `on_menu_event`에서 `app:check-update` → `check_update(handle, true)`.

## 3. 검증

| 항목 | 결과 |
|------|------|
| `cargo check`(src-tauri) | ✅ 1.62s |
| `cargo clippy` | ✅ **0 경고/에러** |
| 흐름 리뷰 | check→알림→설치→재시작 / 최신·오류 분기 / `manual` 분기 OK |
| 엔진·시크릿·Pages | 무관/무영향 |

## 4. 플랫폼별 동작 (중요)

| 플랫폼 | 시작 시 자동 확인 | 메뉴 수동 확인 |
|--------|:---:|:---:|
| **macOS** | ✅ | ✅ (네이티브 메뉴 "업데이트 확인") |
| **Windows/Linux** | ✅ | ❌ (네이티브 메뉴 없음 — 웹 UI 메뉴 사용) |

> 네이티브 메뉴는 macOS 전용(이슈 #7: Win/Linux는 웹 UI `#menu-bar` 사용, 중복 방지로 네이티브 메뉴 미부착). **업데이트 전달(자동 확인)은 전 플랫폼 동작**하나, **수동 "업데이트 확인" 버튼은 macOS만**. Win/Linux 수동 경로는 **§6 결정 사항**.

## 5. 부트스트랩·macOS 주의 (재확인)

- 기존 `v0.7.13` 설치본엔 updater 없음 → 첫 updater 버전은 수동 1회 업데이트 후 자동화.
- macOS: 업데이트 전달 동작, 받은 새 버전 첫 실행 Gatekeeper 경고는 #4 완료 전 잔존.

## 6. 결정 필요 — Windows/Linux 수동 "업데이트 확인"

- **(A) 현행 유지**: 시작 시 자동 확인이 업데이트를 전달하므로 충분. 수동 버튼은 macOS만. (추가 작업 0)
- **(B) studio 메뉴 연동**: rhwp-studio 메뉴에 "업데이트 확인" + desktop-bridge 경유 Tauri command 추가 → Win/Linux도 수동 확인. (studio 프론트 소폭 변경)

## 7. 다음 단계

- **Stage 3** — `desktop-release.yml` 서명 env 주입(시크릿) + 매니페스트 자동화 확인.
- §6 결정 반영 후 진행.
