# Task #26 — 데스크톱 앱 자동 업데이트 (수행 계획서)

- **이슈**: [paldyn/HanPage#26](https://github.com/paldyn/HanPage/issues/26) (M100)
- **브랜치**: `local/task26` (devel `086ff8e8` 분기)
- **상태**: 수행계획 승인 대기
- **일자**: 2026-06-09

## 0. 목표·확정 동작

HanPage Desktop이 새 릴리스 출시 시 **업데이트 있음을 알리고 원클릭으로 받아 설치**한다.

| 항목 | 작업지시자 확정 |
|------|----------------|
| 동작 | **알림 + 원클릭 설치**(다운로드→설치→자동 재시작) |
| 확인 시점 | **시작 시 자동 확인 + 메뉴 "업데이트 확인"(수동)** |

## 1. 기술 접근 (Tauri v2 updater)

- **플러그인**: `tauri-plugin-updater`(Rust) — 체크/다운로드/설치를 Rust 측에서 구동(메뉴가 이미 lib.rs에 있어 일관).
- **배포원**: GitHub Releases + `latest.json` 매니페스트. 엔드포인트 = `https://github.com/paldyn/HanPage/releases/latest/download/latest.json`.
- **서명**: ed25519(minisign). **개인키 = GitHub 시크릿**(`TAURI_SIGNING_PRIVATE_KEY` + 비번), **공개키만** `tauri.conf.json`.
- **CI 멱등**: 이미 `tauri-apps/tauri-action@v0` 사용 → `createUpdaterArtifacts` 활성화 + 서명 시크릿 주입 시, 액션이 **서명·`latest.json` 생성·릴리스 첨부를 자동** 처리(추가 스크립트 최소).

## 2. 변경 범위 (데스크톱 한정)

| 파일 | 변경 |
|------|------|
| `HanPage-Desktop/src-tauri/Cargo.toml` | `tauri-plugin-updater = "2"` 추가 |
| `HanPage-Desktop/src-tauri/tauri.conf.json` | `plugins.updater {endpoints, pubkey}` + `bundle.createUpdaterArtifacts: true` |
| `HanPage-Desktop/src-tauri/src/lib.rs` | 플러그인 등록 + 시작 시 백그라운드 확인 + 메뉴 "업데이트 확인" 항목·핸들러 + 알림/다운로드/설치/재시작 흐름 |
| `.github/workflows/desktop-release.yml` | tauri-action 스텝에 `TAURI_SIGNING_PRIVATE_KEY`·`..._PASSWORD` env(시크릿) 주입 |
| (신규) 서명 키페어 | 생성 → 개인키는 **GitHub 시크릿으로만**(저장소 미커밋), 공개키는 config |

> JS `@tauri-apps/plugin-updater`는 Rust-구동 방식에선 불필요(프론트 변경 0). 1차는 native 흐름으로 견고하게.

## 3. UI 설계 ("요즘 앱" UX)

- **시작 시**: 백그라운드 확인 → 새 버전이면 native 알림 대화상자 `업데이트 있음 vX.Y.Z [지금 설치] [나중에]`.
- **메뉴**: `HanPage > 업데이트 확인` → 즉시 확인. 최신이면 `최신 버전입니다` 안내.
- **원클릭**: [지금 설치] → 다운로드(진행 표시) → 설치 → 자동 재시작.
- **대안(후속)**: studio 배너(desktop-bridge 경유) 고도화. 1차 범위 아님.

## 4. 보존 불변식·보안

| 항목 | 처리 |
|------|------|
| **시크릿 금지** | 서명 **개인키는 GitHub 시크릿으로만**, 저장소·로그 미노출·미커밋. 공개키만 config |
| GitHub Pages 무영향 | 데스크톱 전용. `deploy-pages.yml`은 `push:[main]`+`HanPage-Desktop/**` ignore → 무발동 |
| main 무영향 | devel 작업. main 반영은 후속 릴리스(비범위) |
| 크레이트 내부명 | `rhwp-desktop`/`rhwp_desktop_lib` 유지 |
| 엔진 무관 | `src/`(HWP/HWPX 파서) 변경 0 |

## 5. 리스크·주의

- **부트스트랩**: 기존 `v0.7.13` 설치본엔 updater가 없어 자동 감지 불가 → 첫 updater 버전은 **수동 1회 업데이트** 후부터 자동화(릴리스 노트 안내). 첫 updater 버전 = 차기 릴리스(예: v0.7.14/0.8.0).
- **macOS 미공증**: updater 서명(Tauri minisign)은 Apple 공증과 별개라 **업데이트 전달은 동작**. 단 받은 새 버전 첫 실행 시 Gatekeeper 경고는 **#4(공증, 일시정지)** 완료 전 잔존.
- **서명 키 분실 = 업데이트 서명 영구 불가** → 키 생성 직후 안전 보관(시크릿 + 별도 백업) 안내.
- **매니페스트 정합**: target/arch/버전 비교·엔드포인트 URL 정확성(검증 단계에서 모의 매니페스트로 확인).

## 6. 단계 개요 (구현계획서에서 3~6단계로 상세화)

1. 서명 키페어 생성 + `tauri.conf.json`(updater/pubkey/createUpdaterArtifacts) + 플러그인 의존·등록 → **로컬 빌드 통과**.
2. 시작 시 확인 + 메뉴 항목 + 알림/다운로드/설치/재시작 흐름(lib.rs).
3. `desktop-release.yml` 서명 시크릿 주입 + 매니페스트 자동화 확인.
4. 검증(로컬 빌드·모의 매니페스트로 체크 흐름·문서: 시크릿 등록 가이드·부트스트랩 안내) + 최종 보고.

## 7. 비범위

- `main` 릴리스/태깅(별도 릴리스 절차). studio 배너 고도화(후속). #4 코드 서명·공증(별도, 일시정지). 실제 릴리스 발행(키·시크릿 등록 후 작업지시자 시점 결정).
