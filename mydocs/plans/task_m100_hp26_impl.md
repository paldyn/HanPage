# Task #26 — 데스크톱 자동 업데이트 (구현 계획서)

- **이슈**: [paldyn/HanPage#26](https://github.com/paldyn/HanPage/issues/26) · 수행계획서: `task_m100_hp26.md`(승인됨)
- **브랜치**: `local/task26` (devel `086ff8e8` 분기)
- **상태**: 구현계획 승인 대기

## 0. 전제

수행계획(접근·범위·불변식·확정 동작) 승인 완료. 본 문서는 **단계(4)·구체 변경·검증·보안 처리**를 정의. 소스 변경은 단계별 승인 후. 실제 릴리스 발행은 비범위.

## 1. 확정 사실 (조사 완료)

- CI는 이미 `tauri-apps/tauri-action@v0` 사용 → `createUpdaterArtifacts: true` + 서명 env 주입 시 **서명·`latest.json`·릴리스 첨부 자동**. CI 변경 = env 2줄.
- `lib.rs run()`: `Builder::default()` → `.plugin(...)`×4 → `.setup()`(메뉴 구성) → `.on_menu_event()` → `.run()`. 메뉴는 `build_app_menu()`의 `SubmenuBuilder`.
- 통합 지점: ①빌더에 updater 플러그인 ②`setup()`에 시작 확인 ③`build_app_menu()`에 "업데이트 확인" 항목 ④`on_menu_event()`에 핸들러.

## 2. 단계 (4)

| 단계 | 내용 | 검증 |
|------|------|------|
| **Stage 1** | 서명 키페어 + `tauri.conf.json`(updater/pubkey/createUpdaterArtifacts) + `Cargo.toml`(plugin) + `lib.rs`(plugin 등록) | 개인키 **미추적** 확인 · `src-tauri` `cargo check` 통과 · config 유효 |
| **Stage 2** | 업데이트 흐름(lib.rs): 시작 시 백그라운드 확인 + 메뉴 "업데이트 확인" + native 알림(지금 설치/나중에) + 다운로드/설치/재시작 | `cargo check` · 흐름 리뷰(브라우저/데스크톱 분기·에러 처리) |
| **Stage 3** | `desktop-release.yml` 서명 env 주입 + 시크릿 등록 가이드 | YAML 유효 · (가능 시) dispatch 빌드로 updater 아티팩트 생성 확인 |
| **Stage 4** | 검증·문서·최종 보고 | 로컬 빌드/모의 `latest.json`로 체크 흐름 · 부트스트랩·시크릿 가이드 문서 |

## 3. Stage 1 상세 — 서명 키 + config + 플러그인

### 3-1. 서명 키 (보안 핵심)
- `npm run tauri signer generate -- -w /tmp/hanpage-updater.key`(**저장소 외부** 경로) → 비번 설정 → 공개키 출력.
- **개인키·비번 → GitHub 시크릿**: `gh secret set TAURI_SIGNING_PRIVATE_KEY --repo paldyn/HanPage < /tmp/hanpage-updater.key` + `gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. **작업지시자와 비번·백업 조율**(키 분실 시 업데이트 서명 영구 불가 → 안전 백업 필수). 등록 후 `/tmp` 키파일 삭제.
- **공개키만** `tauri.conf.json`. 저장소에 개인키 흔적 0(커밋·로그·추적 모두).

### 3-2. tauri.conf.json
```jsonc
"bundle": { "createUpdaterArtifacts": true, ... },
"plugins": {
  "updater": {
    "endpoints": ["https://github.com/paldyn/HanPage/releases/latest/download/latest.json"],
    "pubkey": "<생성된 공개키>"
  }
}
```
### 3-3. Cargo.toml / lib.rs
- `tauri-plugin-updater = "2"` 추가.
- `run()` 빌더 체인에 `.plugin(tauri_plugin_updater::Builder::new().build())`.

## 4. Stage 2 상세 — 업데이트 흐름 (lib.rs)

- **공용 async 헬퍼** `check_update(app, manual: bool)`:
  - `app.updater()?.check().await` → `Some(update)` 면 native 대화상자(`tauri-plugin-dialog`): `새 버전 vX.Y.Z 가 있습니다. 지금 설치할까요? [지금 설치][나중에]`.
  - [지금 설치] → `update.download_and_install(on_chunk, on_done).await` → `app.restart()`.
  - `None` + `manual=true` → "최신 버전입니다" 안내. `manual=false`(시작 시)면 무알림.
  - 네트워크/오류는 조용히 로깅(시작 시) / 수동 시 간단 안내.
- **시작 확인**: `setup()`에서 `tauri::async_runtime::spawn(check_update(handle, false))`.
- **메뉴**: `build_app_menu()`의 HanPage 서브메뉴에 `MenuItemBuilder::with_id("app:check-update", "업데이트 확인")`.
- **핸들러**: `on_menu_event()`에 `"app:check-update" => spawn(check_update(handle, true))`.

## 5. Stage 3 상세 — CI

`desktop-release.yml` tauri-action 스텝 `env:`에 추가:
```yaml
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
```
- 시크릿 미등록 환경(fork·dispatch)에서 빌드가 깨지지 않도록 동작 확인(서명 없으면 updater 아티팩트만 생략, 일반 번들은 생성).

## 6. Stage 4 상세 — 검증·문서

- **흐름 검증**: 로컬에서 모의 `latest.json`(상위 버전)으로 체크→알림 경로 확인(가능 범위; 풀 번들·서명 검증은 CI/릴리스 시점).
- **문서**(`manual/` 또는 `tech/`): ①시크릿 등록·키 백업 가이드 ②**부트스트랩 안내**(기존 v0.7.13엔 updater 없음 → 첫 updater 버전은 수동 1회) ③릴리스 절차(태그→CI 서명·매니페스트).
- **무영향 확인**: `src/` 변경 0 · Pages 무발동 · 크레이트명 유지 · 개인키 미추적.

## 7. 리스크/롤백

- **개인키 노출 방지**: 저장소 외부 생성 + `gh secret set`(미커밋) + 사용 후 로컬 삭제. 매 단계 `git status`로 키파일 미추적 확인.
- **빌드 회귀**: src-tauri는 독립 크레이트(루트 빌드 무영향). 문제 시 브랜치 폐기로 즉시 롤백(devel 무관).
- **macOS 미공증/부트스트랩**: 기능 한계로 문서화(차단 아님). #4 완료 시 공증 결합.
