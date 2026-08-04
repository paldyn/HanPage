# Task #40 (M100) — 엔진 업스트림 재동기화 0.7.15 → 0.7.18 수행계획서

- **이슈**: [paldyn/HanPage#40](https://github.com/paldyn/HanPage/issues/40)
- **브랜치**: `local/task40` (devel `b5f1e3dc` 분기) · 재구축 `local/task40-rebase`
- **기준 고정**: upstream/devel `dbb2d9ff` (0.7.18) ← 지난 고정점 `9172e3a2` (0.7.15)

## 1. 규모 / 특이사항

- 신규 커밋 2,139 · 변경 3,465파일(+387,891/−28,068)
- **pdf-large 2건 변경 포함** → filter-repo strip 필수 (업스트림 LFS 예산 초과 → GH008 회피)
- **업스트림 신규 추적**: 루트 `Cargo.lock`, `pdf-2020/`, `output` — 루트 lock은 업스트림 방침대로 수용하되 `.gitignore` 재적용 시 우리 예외(`!HanPage-Desktop/src-tauri/Cargo.lock`) 유지 주의

## 2. 재적용할 paldyn 레이어 (Task #23/#28 레시피 + 이후 증분)

업스트림도 진화한 델타 파일(⚠ = 수술적 재적용): `Cargo.toml`(repo URL), `rhwp-studio/package.json`(build:desktop+cross-env), `vite.config.ts`(isDesktop/PWA 브랜드), `src/main.ts`(initDesktopBridge + **Task #38 버튼 최상단 설치**), `.gitignore`(데스크톱 lock 예외).

통째 재적용(우리 전용): 브랜딩(HanPage), `HanPage-Desktop/`(updater + **src-tauri/Cargo.lock**), CI(`desktop-release.yml` — **Task #4 서명·공증 env + 툴체인 1.93.1 핀**, `deploy-pages.yml`), `rhwp-studio/src/ui/app-download.ts` + menu-bar.css 버튼, `src/core/desktop-bridge.ts`, mydocs(paldyn 패턴만: `task_m100_hp*`, orders, pr).

엔진 식별자 유지: crate `rhwp`, `@rhwp/*`, Edward Kim 저작권/크레딧.

## 3. 단계

1. **Stage 1 — 기준 고정 + 재구축**: `local/task40-rebase`를 `dbb2d9ff`에서 생성, paldyn 전용 파일 wholesale checkout(comm 기반), ⚠ 델타 5파일 수술 재적용, 버전 0.7.18 정합(데스크톱 tauri.conf/Cargo.toml/package.json)
2. **Stage 2 — strip + 검증**: pdf-large filter-repo strip(`rm -rf .git/filter-repo` 선행), 엔진 `src/` diff vs `dbb2d9ff` = 0 확인, 로컬 cargo build/test + WASM(Docker) + studio 빌드, 데스크톱 lock 재생성 여부 점검
3. **Stage 3 — 반영 + 릴리스**: 백업(태그+번들) → devel force-push → main take-devel 필요 여부 판단(이번엔 devel 히스토리 재작성이므로 **필요**) → Pages 배포 확인 → `hanpage-desktop-v0.7.18` 태그 → 서명+공증 릴리스 → 구 v0.7.15 릴리스 삭제(자동업데이트 latest 갱신)

## 4. 리스크

- 델타 재적용 누락(과거 사례: build:desktop 스크립트 유실 → Task #27) → 체크리스트(§2) 대조로 방지
- filter-repo 스테일 마커 → 실행 전 `.git/filter-repo` 삭제
- 루트 Cargo.lock 신규 추적으로 `.gitignore` 상호작용 변화 → strip/체크아웃 후 `git status` 정밀 확인
