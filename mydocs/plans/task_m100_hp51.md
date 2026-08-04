# Task #51 (M100) — 엔진 재동기화 0.7.18 → 0.8.2 + v0.8.2 릴리스 수행계획서

- **이슈**: [paldyn/HanPage#51](https://github.com/paldyn/HanPage/issues/51)
- **브랜치**: `local/task51` (devel 분기) · 재구축 `local/task51-rebase`
- **기준 고정**: upstream/devel `cf5d462dc` (0.8.2) ← 지난 고정점 `dbb2d9ff` (0.7.18)

## 1. 규모 / 특이사항

| 항목 | 값 |
|------|-----|
| 신규 커밋 | 2,552 |
| 변경 파일 | 4,512 (+463,794 / −45,829) |
| 버전 | 0.7.18 → **0.8.2** (minor 상승) |
| pdf-large | 이번 구간 변경 0건, 업스트림 트리에 16파일 존재 → **strip 필요** |

## 2. 선행 조건

Task #50 수정(`9177a6fd`, `local/task50`)을 **devel 에 먼저 병합**한다. 그래야 재베이스 시
paldyn 레이어 산출(comm devel vs 업스트림 고정점)에 자동 포함된다.

## 3. 재적용 대상

**수술 재적용(업스트림도 변경됨, 7종)**

| 파일 | paldyn 델타 |
|------|------------|
| `Cargo.toml` | repository URL → paldyn/HanPage |
| `.gitignore` | 데스크톱 lock 관련 정합 |
| `rhwp-studio/package.json` | `build:desktop` 스크립트 + `cross-env` |
| `rhwp-studio/vite.config.ts` | isDesktop 분기 + PWA 브랜드(HanPage) |
| `rhwp-studio/src/main.ts` | 다운로드 버튼 최상단 설치(#38), `initDesktopBridge` 배선 |
| `rhwp-studio/src/core/font-loader.ts` | **#50 폰트 타임아웃/예산** (업스트림은 CanvasKit 폰트 계획 함수만 추가, 타임아웃 없음 → 계속 필요) |
| `npm/editor/*`, `README*` 등 | 브랜드/URL (지난 회차와 동일 규칙) |

**그대로 적용(업스트림 무변경 또는 paldyn 전용)**: `desktop-bridge.ts`(#50 재드레인), `HanPage-Desktop/**`(updater·서명·`src-tauri/Cargo.lock`·`dragDropEnabled:false`), `.github/workflows/desktop-release.yml`(#4 서명·공증 + 툴체인 1.93.1 핀), `deploy-pages.yml`, `app-download.ts`, `style-bar.css`(#41 콤보 폭), 브랜딩 자산, mydocs(paldyn 패턴).

엔진 식별자 유지: crate `rhwp`, `@rhwp/*`, Edward Kim 저작권/크레딧.

## 4. 단계

1. **Stage 1** — #50 → devel 병합 후 `local/task51-rebase`(`cf5d462dc` 기준) 생성, comm 3분할(전용/통째/수술) + 파일별 3-way merge, 버전 0.8.2 정합(데스크톱 3파일), package-lock 재생성
2. **Stage 2** — pdf-large filter-repo strip(`.git/filter-repo` 선삭제) + 검증: 엔진 `src/` diff=0, `cargo build --release`·`cargo test --lib`, WASM(Docker), studio 빌드·테스트, 데스크톱 빌드, 데스크톱 lock 0.8.2
3. **Stage 3** — 백업(태그 2 + 번들) → devel force-push → main take-devel merge → Pages 배포 확인 → `hanpage-desktop-v0.8.2` 태그 → 서명·공증 릴리스 검증(spctl/stapler, latest.json 4플랫폼) → 구 v0.7.18 릴리스 삭제

## 5. 리스크

- **minor 상승(0.8.x)**: 엔진 API/스튜디오 구조 변경 폭이 클 수 있음 → 수술 대상 파일의 3-way merge 충돌이 지난 회차(12건)보다 많을 수 있다. 충돌은 "업스트림 구조 채택 + paldyn 델타 재이식" 원칙으로 해소.
- **오분류**: 업스트림 모듈 분해/개명이 paldyn 전용으로 잡히는 사례(지난 회차 `object_ops.rs`) → 전용 목록 전수 검사 필수.
- **filter-repo 스테일 마커** → 실행 전 `.git/filter-repo` 삭제.
- **#50 회귀 방지**: 재적용 후 폰트 타임아웃·전역 큐·재드레인 3종이 살아있는지 코드 검사 + CDN 차단 A/B 재실행.
