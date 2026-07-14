# Task #23 Stage 2 완료 보고서 — HanPage-Desktop 이식

- **이슈**: [paldyn/HanPage#23](https://github.com/paldyn/HanPage/issues/23) (M100)
- **단계**: Stage 2 / 5
- **계획서**: `mydocs/plans/task_m100_hp23_impl.md` §3 Stage 2 · 메커닉 §1-3(2)
- **일자**: 2026-06-02

## 1. 단계 목표 (구현 계획서 §3)

> **Stage 2** — HanPage-Desktop 이식: origin/main의 `HanPage-Desktop/` 최종 상태를 product 브랜치로 복사(self-contained, 엔진 충돌 0).

## 2. 이식 내용

| 항목 | 값 |
|------|-----|
| 출처 | origin/main `0156d8ef` (paldyn 권위 원본) |
| 대상 | `local/task23-rebase` (upstream base `f6ffe9d6`) |
| 방법 | `git checkout origin/main -- HanPage-Desktop/` (경로 추출) |
| 결과 | **28 tracked 파일, +783줄, 커밋 `c8ebe7f2`** |

**파일 구성 (28)**:
- 루트 4: `package.json` · `package-lock.json` · `README.md` · `.gitignore`
- `src-tauri/` 24: `Cargo.toml` · `build.rs` · `tauri.conf.json` · `capabilities/default.json` · `src/{lib.rs,main.rs}` · `.gitignore` · `icons/` 18종(HanPage 브랜드 아이콘: `icon.{png,icns,ico}` · `Square*Logo.png` · `StoreLogo.png` 등)

## 3. 이식 무결성 검증

| 검증 | 결과 |
|------|------|
| staged 28파일 전부 `HanPage-Desktop/` 하위 | ✅ (외부 0) |
| 엔진 `src/` 변경 | **0건** ✅ |
| working tree 잔여 변경(추가 외) | 없음 ✅ (순수 추가) |
| 이식 후 tracked 파일 수 | 28 (origin/main과 일치) ✅ |

## 4. 자기완결성 검증 (엔진 충돌 0 근거)

데스크톱 앱은 엔진 크레이트에 **Cargo 의존이 전혀 없는 독립 크레이트**임을 기반에서 확인했다 — 재베이스 시 Cargo 의존 불일치가 원천적으로 발생하지 않는다.

| 근거 | 확인 내용 |
|------|----------|
| `HanPage-Desktop/src-tauri/Cargo.toml` | `[dependencies]`에 `rhwp` 엔진 의존 **없음**. Tauri 플러그인만 의존(독립 크레이트, 주석으로 명시) |
| upstream 루트 `Cargo.toml` | `[workspace]` 섹션 **부재** → 루트 `cargo build`는 `rhwp` 엔진만 빌드, Tauri 의존 미포착 |
| 통합 지점 | `tauri.conf.json`: `frontendDist=../../rhwp-studio/dist`, `beforeBuildCommand=npm run build:frontend` → 엔진 통합은 **WASM/프론트 빌드 레이어**에서 수행 |
| 프론트 통합 대상 | `rhwp-studio/` 기반에 **존재** ✅ |

> 즉 엔진↔데스크톱 결합은 Cargo가 아니라 rhwp-studio(WASM) 프론트 빌드를 통해 이뤄지며, upstream 기반에 `rhwp-studio/`와 엔진 `src/`가 모두 존재하므로 통합 사슬이 충족된다.

## 5. 브랜딩 경계 (Stage 3 이월 사항)

- **데스크톱 자체 식별자**(이식된 `tauri.conf.json`에 이미 반영): `productName=HanPage`, `mainBinaryName=HanPage`, `identifier=com.paldyn.hanpage`, `version=0.7.13` — 별도 작업 불필요.
- **Rust 크레이트 내부명**(보존 불변식, 의도적 유지): `package.name=rhwp-desktop`, `lib.name=rhwp_desktop_lib` — **변경하지 않음**.
- **rhwp-studio 프론트 브랜딩**: 현재 기반의 rhwp-studio는 upstream 버전(브랜딩 미적용) → **Stage 3 리브랜딩**에서 HanPage 서비스 브랜딩(HanPage·hanpage.paldyn.com) 적용 대상.

## 6. 보존 불변식 점검

| 불변식 | 상태 |
|--------|------|
| rhwp 엔진 식별자 | 엔진 무변경 → 자연 보존 ✅ |
| `rhwp-desktop`/`rhwp_desktop_lib` 크레이트 내부명 | 유지 ✅ |
| 시크릿 금지 | 신규 시크릿 없음 ✅ |
| GitHub Pages 무영향 | 데스크톱 디렉터리 추가만 — Pages 워크플로 무관 ✅ |

## 7. 다음 단계

- **Stage 3** — 리브랜딩 + CI/Pages 재적용(약 44파일): upstream 기반의 rhwp-studio·문서·에셋·워크플로에 paldyn 서비스 브랜딩을 기계적 재적용. rhwp 엔진 식별자(`rhwp`/`@rhwp/*`/edwardkim)는 보존.
- **승인 대기** — 본 보고서 승인 후 Stage 3 착수.
