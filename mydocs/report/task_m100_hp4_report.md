# Task #4 (M100) — macOS 코드 서명 + 공증 최종 보고서

## 1. 목표

데스크톱 앱(HanPage Desktop) macOS 빌드를 **Developer ID 코드 서명 + Apple 공증(notarization)** 하여,
사용자가 내려받아 실행할 때 Gatekeeper **"확인되지 않은 개발자(unidentified developer)" 경고 없이 바로 실행**되도록 한다.

## 2. 결과 요약

- ✅ **v0.7.15 재발행** — 서명+공증된 dmg / app.tar.gz.
- ✅ **로컬 검증 통과**: Gatekeeper `accepted / source=Notarized Developer ID`.
- `latest.json`에 darwin 엔트리 유지 → 자동 업데이트 정상.
- Windows 서명은 별도(Authenticode 인증서 구매 필요, 미진행).

## 3. 작업 내역

### 3.1 Apple 시크릿 6개 등록 (GitHub repo secrets — 커밋 없음, gh secret set)

| 시크릿 | 내용 |
|--------|------|
| `APPLE_CERTIFICATE` | Developer ID Application `.p12` (base64) |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` 비밀번호 |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Wonmo Lee (8L78W6D8XF)` |
| `APPLE_API_KEY_P8` | App Store Connect API 키 `.p8` (base64) |
| `APPLE_API_KEY_ID` | `ZJ66UJ26W9` (공증) |
| `APPLE_API_ISSUER` | Issuer ID (공증) |

> 시크릿은 모두 GitHub repo secrets로만 등록. `.p12`/`.p8`/비밀번호는 저장소에 커밋하지 않음.

### 3.2 서명·공증 배선 — `desktop-release.yml` (커밋 `8a25d77a`)

- **Prepare notarization API key (macOS)** 스텝: `APPLE_API_KEY_P8`(base64) → `$RUNNER_TEMP/apple_api_key.p8` (`if: runner.os == 'macOS'`).
- **tauri-action env**: `APPLE_CERTIFICATE`/`_PASSWORD`/`_SIGNING_IDENTITY`(서명) + `APPLE_API_ISSUER`/`_KEY`/`_KEY_PATH`(공증).
- Apple env 부재(fork 등) 시 tauri-action이 서명·공증을 건너뜀 → 회귀 없음.

### 3.3 빌드 블로커 2건 발견·수정 (예상 외, 서명과 무관한 CI 재현성 문제)

재발행 빌드가 tauri의 **brotli 8.0.3** 컴파일에서 실패(`StandardAlloc: alloc::Allocator<T> 미충족`). 원인 2건:

**(A) 부동 Rust 툴체인 — 커밋 `8f4c0718`**
- `desktop-release.yml`이 `dtolnay/rust-toolchain@stable`을 사용 → 액션이 `RUSTUP_TOOLCHAIN=stable`을 설정해 루트 `rust-toolchain.toml`(1.93.1) 핀을 **무시**.
- 신규 stable **1.96.0**이 brotli 8.0.3 컴파일 실패. (직전 성공 빌드는 캐시된 brotli 재사용, 캐시 미스 시 노출.)
- → **`@1.93.1`로 핀**(엔진과 동일 툴체인). 로컬 1.93.1에서 brotli 정상 컴파일 확인.

**(B) 미추적 데스크톱 `Cargo.lock` — 커밋 `33cb421f`** ← 근본 원인
- `HanPage-Desktop/src-tauri/Cargo.lock`이 `.gitignore`의 `Cargo.lock` 규칙에 걸려 **미추적**.
- → CI가 매번 의존성을 새로 해석 → 깨진 **brotli-decompressor 5.0.2** 취득(5.0.1은 정상). 풀트리에서 brotli `std` feature는 정상 활성(tauri `compression`) → 순수 버전 회귀.
- → **락 커밋(brotli-decompressor 5.0.1 고정)** + `.gitignore`에 데스크톱 앱 lock 예외(`!HanPage-Desktop/src-tauri/Cargo.lock`) 추가. 앱(바이너리)은 재현 빌드를 위해 lock 커밋이 정석.

### 3.4 검증

- **CI 로그**: `Mac Developer ID Application: Wonmo Lee` 서명 → `Notarizing Finished with status Accepted ... (Processing complete)` → `Stapling app...`.
- **로컬**(내려받은 `HanPage_aarch64.app.tar.gz` 추출):
  - `codesign -dvvv`: Authority 체인 `Developer ID Application: Wonmo Lee (8L78W6D8XF)` → `Developer ID Certification Authority` → `Apple Root CA`.
  - `codesign --verify --deep --strict`: `valid on disk` + `satisfies its Designated Requirement` (exit 0).
  - `spctl -a -vvv -t exec`: **`accepted` / `source=Notarized Developer ID`**.
  - `xcrun stapler validate`: `The validate action worked!` (공증 티켓 스테이플 → 오프라인 동작).

## 4. 커밋

| 커밋 | 내용 |
|------|------|
| `8a25d77a` | macOS 서명·공증 배선(desktop-release.yml) |
| `8f4c0718` | CI Rust 툴체인 1.93.1 핀 |
| `33cb421f` | 데스크톱 Cargo.lock 커밋(brotli-decompressor 5.0.2 회피) + .gitignore 예외 |

devel `33cb421f` → main `a1f8e973` 반영, `hanpage-desktop-v0.7.15` 태그 재발행.

## 5. 후속

- **Windows 코드 서명(Authenticode)** — 별도 인증서 구매 필요, 미진행.
- **CI 재현성 교훈**(메모리 기록): 데스크톱 lock 커밋 유지 + 툴체인 핀. 향후 엔진 동기화/재베이스 시 두 항목이 유실되지 않도록 주의.
