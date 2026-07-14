# Task #26 최종 결과보고서 — 데스크톱 앱 자동 업데이트

- **이슈**: [paldyn/HanPage#26](https://github.com/paldyn/HanPage/issues/26) (M100)
- **브랜치**: `local/task26` (devel `086ff8e8` 분기)
- **일자**: 2026-06-11
- **상태**: Stage 1~4 완료·검증 통과 → 최종 보고 승인 후 반영

## 1. 개요·목표

HanPage Desktop이 새 릴리스 출시 시 **업데이트를 알리고 원클릭으로 받아 설치**하도록 구현. Tauri v2 updater + GitHub Releases + 서명 기반.

**확정 동작**: 알림 + 원클릭 설치 / 시작 시 자동 확인 + 메뉴 수동 확인.

## 2. 기술 접근

- `tauri-plugin-updater`(Rust, desktop 전용). 배포원 = GitHub Releases `latest.json`.
- 서명: 공개키=`tauri.conf.json`, **개인키=GitHub 시크릿**. CI(`tauri-action`)가 서명·매니페스트·첨부 자동.

## 3. 단계별 결과

| 단계 | 내용 | 커밋 | 검증 |
|------|------|------|------|
| **Stage 1** | 서명 키페어 + config(pubkey·createUpdaterArtifacts) + 플러그인 등록 | `ed359a0b` | `cargo check` ✅·개인키 미추적·시크릿 2개 등록 |
| **Stage 2** | 업데이트 흐름(lib.rs): 시작 확인 + macOS 메뉴 + 알림·설치·재시작 | `a40a13ca` | `cargo check`+`clippy` 0경고 |
| **Stage 3** | CI 서명 env 주입(desktop-release.yml) | `3c65ccb3` | YAML 유효·env 3키·시크릿 확인 |
| **Stage 4** | 검증 + 문서 + 최종 보고 | (본 커밋) | 무영향 확인·운영 가이드 |

## 4. 변경 범위

| 파일 | 변경 |
|------|------|
| `HanPage-Desktop/src-tauri/Cargo.toml` | `tauri-plugin-updater`(desktop 전용) |
| `HanPage-Desktop/src-tauri/src/lib.rs` | 플러그인 등록 + `check_update()` + 시작 확인 + 메뉴·핸들러 |
| `HanPage-Desktop/src-tauri/tauri.conf.json` | `createUpdaterArtifacts` + `plugins.updater{endpoints,pubkey}` |
| `.github/workflows/desktop-release.yml` | 서명 시크릿 env 2개 |
| `mydocs/manual/desktop_auto_update.md` | 운영 가이드(신규) |

## 5. 검증

| 항목 | 결과 |
|------|------|
| `cargo check`(src-tauri) | ✅ |
| `cargo clippy` | ✅ 0경고 |
| 워크플로 YAML | ✅ 유효, env 3키 |
| 엔진 `src/` 변경 | **0** ✅ |
| studio(`rhwp-studio`) 변경 | 0 (A안) ✅ |
| 추적 시크릿/개인키 | **0** ✅ |
| 크레이트 내부명 | `rhwp-desktop`/`rhwp_desktop_lib` 유지 ✅ |
| GitHub 시크릿 | `TAURI_SIGNING_PRIVATE_KEY`·`_PASSWORD` 2개 ✅ |

> 실제 업데이트 e2e(서명 검증·다운로드·설치)는 **첫 updater 포함 릴리스 시점**에 동작 검증(config·코드·CI는 정적 검증 완료). 데스크톱/WASM 풀빌드는 CI/릴리스(계획 비범위).

## 6. 플랫폼별 동작

| | 시작 자동 확인 | 메뉴 수동 확인 |
|--|:--:|:--:|
| macOS | ✅ | ✅ (네이티브 메뉴) |
| Windows/Linux | ✅ | ❌ (네이티브 메뉴 없음 — A안) |

업데이트 전달은 전 플랫폼. 수동 버튼은 macOS만(작업지시자 결정 A). Win/Linux 수동 메뉴는 후속 옵션 B(studio 연동) 가능.

## 7. 보존 불변식

- 엔진 `src/` 무관 · 크레이트 내부명 유지 · **개인키 미커밋(시크릿만)** · GitHub Pages 무영향(데스크톱 전용; `deploy-pages.yml`은 `push:[main]`+`HanPage-Desktop/**` ignore).

## 8. 한계·후속

- **부트스트랩**: 기존 `v0.7.13`엔 updater 없음 → 첫 updater 버전은 수동 1회 설치 후 자동화.
- **macOS Gatekeeper**: 업데이트 동작하나 미공증 경고는 **#4(일시정지)** 완료 전 잔존.
- **Win/Linux 수동 메뉴**: 후속 옵션 B.
- **실제 릴리스 발행**: 키·시크릿 준비 완료. 버전 올림 + 태그 push 시점은 작업지시자 결정(비범위).

## 9. 운영 가이드

`mydocs/manual/desktop_auto_update.md` — 키 관리·릴리스 절차·`latest.json` 형식·트러블슈팅.

## 10. 산출물

- 계획서: `plans/task_m100_hp26.md`·`task_m100_hp26_impl.md`
- 단계 보고서: `working/task_m100_hp26_stage{1,2,3}.md`
- 운영 가이드: `manual/desktop_auto_update.md`
- 최종 보고서: 본 문서
