# Task #26 Stage 1 완료 보고서 — 서명 키 + config + 플러그인 등록

- **이슈**: [paldyn/HanPage#26](https://github.com/paldyn/HanPage/issues/26) (M100)
- **단계**: Stage 1 / 4 · **계획서**: `plans/task_m100_hp26_impl.md` §3
- **일자**: 2026-06-09

## 1. 단계 목표

서명 키페어 + `tauri.conf.json`(updater/pubkey/createUpdaterArtifacts) + `Cargo.toml`·`lib.rs`(플러그인 등록) → 로컬 빌드 통과·개인키 미추적.

## 2. 변경 내용

| 파일 | 변경 |
|------|------|
| `src-tauri/Cargo.toml` | desktop 전용 target에 `tauri-plugin-updater = "2"` 추가 |
| `src-tauri/src/lib.rs` | `#[cfg(desktop)]` 블록에 `.plugin(tauri_plugin_updater::Builder::new().build())` (시작 확인·메뉴는 Stage 2) |
| `src-tauri/tauri.conf.json` | `bundle.createUpdaterArtifacts: true` + `plugins.updater {endpoints, pubkey}` |

- **endpoints**: `https://github.com/paldyn/HanPage/releases/latest/download/latest.json`
- **pubkey**: 생성된 ed25519/minisign 공개키(152B) 삽입.

## 3. 서명 키 (보안 처리)

- `tauri signer generate`로 키페어 생성 — **저장소 외부**(`/tmp/hanpage-updater.key`), 강력 랜덤 비번(파일).
- **GitHub 시크릿 2개 등록**(`gh secret set`, 값은 파일에서 직접 → 채팅·저장소 미노출):
  - `TAURI_SIGNING_PRIVATE_KEY`
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- **공개키만** `tauri.conf.json`. 저장소에 개인키 흔적 0(추적 0건 확인).
- ⚠ **백업 필요(작업지시자)**: `/tmp/hanpage-updater.key`(개인키)·`/tmp/hanpage-updater.pass`(비번)를 **안전 보관**(키 분실 시 동일 pubkey로 서명 영구 불가 → 사용자 전원 재설치 필요). 백업 확인 후 `/tmp` 삭제.

## 4. 검증

| 항목 | 결과 |
|------|------|
| `tauri.conf.json` JSON 유효 | ✅ (createUpdaterArtifacts=true·endpoints·pubkey 152B) |
| `cargo check`(src-tauri) | ✅ 1.75s (tauri_build config 검증 + 컴파일) |
| 개인키/비번 저장소 추적 | **0건** ✅ |
| git 변경 파일 | Cargo.toml·lib.rs·tauri.conf.json 3건뿐 ✅ |
| GitHub 시크릿 등록 | `TAURI_SIGNING_PRIVATE_KEY`·`_PASSWORD` 2개 ✅ |

## 5. 보존 불변식

- 엔진 `src/` 무관 · 크레이트 내부명(`rhwp-desktop`/`rhwp_desktop_lib`) 유지 · 개인키 미커밋(시크릿만) · GitHub Pages 무영향(데스크톱 전용).

## 6. 다음 단계

- **Stage 2** — 업데이트 흐름(lib.rs): 시작 시 자동 확인 + 메뉴 "업데이트 확인"(macOS 네이티브; Win/Linux는 시작 확인 위주, 메뉴 경로 후속 검토) + native 알림(지금 설치/나중에) + 다운로드·설치·재시작.
- **백업 확인 후** `/tmp` 키 삭제 + 승인 시 Stage 2 착수.
