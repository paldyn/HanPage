# Task #18 — Windows 실행 파일명 `rhwp-desktop.exe` → `HanPage.exe` (수행 + 구현 계획서)

- 이슈: [paldyn/HanPage#18](https://github.com/paldyn/HanPage/issues/18) · 마일스톤 M100(v1.0.0) · #13 후속
- 브랜치: `local/task18` (origin/main `a7228f52` 분기) · 머지 타깃: **`main` PR** (#12·#13·#16·#17 선례)
- 성격: **설정(config) 수정 + 릴리스 버전 범프**. 소스 로직·크레이트 식별자 무변경.

## 1. 배경 / 목표

데스크톱 앱 사용자 노출 명칭은 이미 **HanPage**(productName·창 title·identifier `com.paldyn.hanpage`·설치 마법사 파일명 `HanPage_*-setup.exe`)이나, **Windows 설치 후 본체 실행 파일이 `rhwp-desktop.exe`** 로 노출된다. 시작메뉴 바로가기 대상·작업관리자 프로세스명·"실행 중 앱 종료" 프롬프트가 모두 `rhwp-desktop.exe`.

→ 본체 실행 파일명을 **`HanPage.exe`** 로 통일한다. (Rust 크레이트 식별자는 유지.)

## 2. 원인 분석 (확정)

- **Tauri v2 의 메인 바이너리 명명 규칙**: `tauri.conf.json` 에 `mainBinaryName` 이 없으면 메인 바이너리 이름을 **Cargo `[package] name`** 에서 가져온다. (Tauri v1 은 productName 으로 자동 리네임했으나 v2 는 크레이트명을 그대로 사용 — v1→v2 마이그레이션의 알려진 변경점.)
- 현재 상태: `tauri.conf.json` 에 `mainBinaryName` **없음** + `Cargo.toml` `[package] name = "rhwp-desktop"` → 컴파일 산출물 `rhwp-desktop.exe`, 번들러가 이를 그대로 설치.
- **노출 경로 정리**:

| 위치 | 현재 | 기대 |
|------|------|------|
| 설치 마법사 파일명 | `HanPage_0.7.13_x64-setup.exe` | (정상, productName 기반) |
| 설치되는 본체 exe | `rhwp-desktop.exe` ❌ | `HanPage.exe` |
| 시작메뉴 바로가기 대상 | `rhwp-desktop.exe` ❌ | `HanPage.exe` |
| 작업관리자 프로세스명 | `rhwp-desktop.exe` ❌ | `HanPage.exe` |
| NSIS "실행 중 앱 종료" 프롬프트 | `rhwp-desktop.exe` ❌ | `HanPage.exe` |
| macOS `.app` 내부 바이너리 | `rhwp-desktop` (은닉) | `HanPage` |

- **#13 정정**: Task #13 에서 `[package] name` 을 "비노출이라 유지" 로 분류했으나, **Windows 에서는 `[package] name` 이 실행 파일명으로 표면화**된다. `mainBinaryName` 으로 "사용자 노출 exe명" 과 "크레이트명" 을 분리하면 #13 의 원의도(노출=HanPage, 내부=유지)가 완성된다.

## 3. 범위

### 3-1. 변경

| 대상 | 변경 | 비고 |
|------|------|------|
| `tauri.conf.json` | 최상위 `"mainBinaryName": "HanPage"` **신규 1줄** | 핵심 수정 |
| `tauri.conf.json` | `"version": "0.7.13"` → `"0.7.14"` | 릴리스 버전 범프 |
| `HanPage-Desktop/src-tauri/Cargo.toml` | `version = "0.7.13"` → `"0.7.14"` | 동기화 |
| `HanPage-Desktop/package.json` | `"version": "0.7.13"` → `"0.7.14"` | 동기화 |
| `HanPage-Desktop/package-lock.json` | root·`packages[""]` version → `0.7.14` | `npm install` 로 자동 정합 |
| `src-tauri/Cargo.lock` | `rhwp-desktop` 패키지 version → `0.7.14` | `cargo build`/`cargo update -p` 로 자동 정합 |

### 3-2. 유지 (무변경)

- **Rust 크레이트 식별자**: `Cargo.toml` `[package] name = "rhwp-desktop"`, `[lib] name = "rhwp_desktop_lib"`, `main.rs` `rhwp_desktop_lib::run()` — **유지**.
- productName(`HanPage`)·identifier(`com.paldyn.hanpage`)·창 title·fileAssociations·아이콘 — 무변경.
- 워크플로(`desktop-release.yml` 등)·프런트(`rhwp-studio`)·엔진(`rhwp`) — 무관, 무변경.

### 3-3. 제외 (범위 밖)

- 기존 v0.7.13 설치본 → v0.7.14 업그레이드 시 구 `rhwp-desktop.exe` 잔존 가능성(§7 리스크) — 사실상 신규 설치 위주라 별도 마이그레이션 스크립트는 작성하지 않음.
- macOS 코드 서명·공증(Task #4, 일시정지)·기타 무관 항목.

## 4. 결정 포인트 (승인 요청)

1. **`mainBinaryName = "HanPage"`** (productName 과 동일). → 권장. (대안 없음 — 사용자 노출명은 HanPage 로 확정.)
2. **버전 범프 `0.7.13` → `0.7.14`** (patch). 새 릴리스 태그 `hanpage-desktop-v0.7.14` 와 정합. → 권장. 버전 보유 4파일(+2 lock) 일괄 동기화.
3. **릴리스 발행 시점**: 본 PR 머지 직후 태그 push (Stage 3). 기존 v0.7.13 릴리스는 **유지**(삭제·재태그 안 함).

## 5. 구현 단계 (3단계)

### Stage 1 — 설정 수정 + 버전 범프 + 로컬 검증
- `tauri.conf.json`: `mainBinaryName` 추가 + version 0.7.14.
- `Cargo.toml`·`package.json` version 0.7.14, `package-lock.json`(`npm install`)·`Cargo.lock`(`cargo build` 또는 `cargo update -p rhwp-desktop --precise 0.7.14`) 정합.
- **검증**:
  - JSON 유효성 + Tauri v2 `$schema` 키 유효성(`mainBinaryName` 인식).
  - diff 최소성: mainBinaryName 1줄 + version 필드들만. 크레이트명(`[package] name`/`[lib] name`) blob 무변경.
  - productName/identifier/fileAssociations 불변.
  - **(가능 시) macOS 로컬 `npm run build:frontend` + `tauri build --bundles app`** → `…/HanPage.app/Contents/MacOS/HanPage` 본체명 + `Info.plist` `CFBundleExecutable=HanPage` 확인(번들러 리네임 실증). 빌드 환경(WASM/studio) 미비로 비용 과다 시 CI 검증으로 갈음하고 사유 명시.
- 산출: `working/task_m100_hp18_stage1.md`.

### Stage 2 — 최종 보고서 + main PR
- `report/task_m100_hp18_report.md` 작성.
- 비-`local/` 브랜치(예: `task18-mainbin`) push → `gh pr create --base main`.
- 산출: 보고서 + PR.

### Stage 3 — 머지 + 릴리스 + 산출물 확인 + 이슈 클로즈
- (승인) PR 머지(main).
- **릴리스 태그 `hanpage-desktop-v0.7.14` push** → `desktop-release.yml` 트리거.
- CI 산출 확인: `HanPage_0.7.14_x64-setup.exe`(설치 본체 = `HanPage.exe`) + `HanPage_0.7.14_aarch64.dmg`, 릴리스 표시명 "HanPage Desktop-v0.7.14".
- (가능 시) NSIS 산출물 내부 또는 설치 후 본체 exe명 `HanPage.exe` 확인.
- (승인) 이슈 #18 클로즈 + 오늘할일 갱신.

## 6. 검증 계획 (요약)

| 케이스 | 방법 | 단계 |
|--------|------|------|
| config 스키마 정합 | `mainBinaryName` Tauri v2 키 유효 + JSON 파싱 | S1 |
| 크레이트명 무변경 | `Cargo.toml` `[package]`/`[lib]` name blob 비교 | S1 |
| 버전 일괄 정합 | 4파일 + 2 lock 전부 `0.7.14` | S1 |
| 무회귀(노출명) | productName/identifier/fileAssoc diff 0 | S1 |
| 본체 리네임(실증) | macOS `.app` 내부 바이너리 = `HanPage`(가능 시) | S1 |
| **Windows 본체명(권위)** | CI 산출 `.exe` 설치 본체 = `HanPage.exe` | S3 |

## 7. 리스크 / 주의

- **업그레이드 잔존**: v0.7.13(`rhwp-desktop.exe`) 설치본 위에 v0.7.14(`HanPage.exe`) 설치 시, NSIS 가 이전 언인스톨러를 실행하지 않으면 동일 설치폴더에 구 exe 가 남을 수 있음. 신규 설치 위주(사전 1.0)라 영향 경미 — 보고서에 주지.
- **Windows 검증은 CI 한정**: macOS 로컬에서 NSIS 산출 불가 → 최종 권위 검증은 CI 릴리스 산출물.
- **버전 정합 누락 방지**: 4개 version 필드 + 2개 lock 을 빠짐없이 0.7.14 로. 누락 시 번들명/릴리스명 불일치.
- 크레이트명은 절대 변경하지 않음(빌드 영향만 늘고 사용자 가치 0 — #13 결정 유지).

## 8. 배포 / 후속

- 본 PR 머지 + `hanpage-desktop-v0.7.14` 태그 push → CI 가 `HanPage.exe` 본체의 `.exe`/`.dmg` 산출 + "HanPage Desktop-v0.7.14" 릴리스 생성.
- 기존 v0.7.13 릴리스는 유지(이력 보존).
