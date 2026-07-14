# Task #1 수행 계획서 — HanPage 데스크톱 앱 (Tauri 1단계)

## 1. 이슈

GitHub Issue: [#1](https://github.com/paldyn/HanPage/issues/1)
- 마일스톤: v1.0.0 (M100)
- 브랜치: `local/task1` (분기 베이스: `main`)
- 성격: **신규 기능** (HWP 정합 버그가 아님). paldyn 재배포 포크 전용 산출물.

## 2. 목표 및 배경

HanPage 웹 에디터(`rhwp-studio` + WASM)를 **데스크톱 앱**으로도 제공한다.

**1단계 원칙**: 기존 웹 프런트엔드와 WASM 산출물을 **그대로** 데스크톱 셸로 감싸고, 브라우저가 줄 수 없는 **네이티브 UX만** 덧입힌다. 렌더링/파싱 엔진(WASM)은 손대지 않는다.

### 프레임워크 선택 — Tauri v2

| 항목 | Tauri v2 | Electron |
|------|----------|----------|
| 백엔드 | Rust (기존 생태계와 동일 언어) | Node.js |
| 번들 크기 | 수~십수 MB (OS 웹뷰 사용) | 100 MB+ (Chromium 내장) |
| 메모리 | 경량 | 무거움 |
| 기존 자산 재사용 | rhwp 크레이트가 이미 `rlib` 빌드 가능 → 2단계 네이티브 코어 연동 경로 확보 | 별도 |

→ **Tauri v2 채택.** 1단계는 WASM을 그대로 쓰되, Rust 백엔드라는 점이 2단계(네이티브 `rlib` 코어 직접 호출) 확장에 유리하다.

## 3. 범위

### 3.1 포함 (1단계)
- 새 디렉터리 `rhwp-desktop/` (모노레포 sub-project 패턴: `rhwp-ios/`, `rhwp-vscode/`와 동일 위치)
- `rhwp-studio` 빌드 산출물 + WASM을 Tauri 웹뷰에 번들로 로드
- 네이티브 파일 연결: `.hwp` / `.hwpx` 더블클릭 → 앱에서 열기 (OS 파일 association)
- 네이티브 열기/저장 대화상자 (Tauri dialog)
- 최소 메뉴바 (파일/편집/보기)
- 최근 문서 목록 (recent documents)
- 윈도우 상태(크기/위치) 저장·복원
- 인스톨러 산출: macOS `.dmg` / Windows `.msi` / Linux `.deb`

### 3.2 제외 (2단계 이후)
- 네이티브 `rlib` 코어 직접 호출 (1단계는 WASM 그대로 유지)
- 자동 업데이트 (updater)
- 코드 서명 / 공증 (notarization)
- CI 릴리스 자동화
- 외부 참조 이미지의 로컬 파일시스템 해석 (현재 웹과 동일하게 미해석)

## 4. 기술 접근

### 4.1 아키텍처 개요

```
┌──────────────────────────────────────────────┐
│  Tauri 앱 (rhwp-desktop)                       │
│  ┌────────────────────────────────────────┐   │
│  │  OS WebView                            │   │
│  │   = rhwp-studio dist (HTML/JS/CSS)     │   │
│  │   + rhwp_bg.wasm (~12 MB, 그대로)       │   │
│  └────────────────────────────────────────┘   │
│            ▲ 파일 bytes 전달                    │
│  ┌─────────┴──────────────────────────────┐   │
│  │  Rust 백엔드 (src-tauri)                │   │
│  │   - 파일 association 이벤트 수신         │   │
│  │   - 열기/저장 dialog                    │   │
│  │   - 메뉴 / 최근문서 / 윈도우 상태        │   │
│  │   (rhwp 크레이트 의존 없음 — WASM이 렌더)│   │
│  └────────────────────────────────────────┘   │
└──────────────────────────────────────────────┘
```

### 4.2 디렉터리 구조 (신규)

```
rhwp-desktop/
├── package.json          # tauri CLI 래퍼 + 프런트 빌드 오케스트레이션
├── src-tauri/
│   ├── Cargo.toml        # 독립 크레이트 (루트 워크스페이스 미포함)
│   ├── tauri.conf.json   # 윈도우/번들/frontendDist 설정
│   ├── build.rs
│   ├── icons/            # 앱 아이콘 (HanPage)
│   └── src/
│       ├── main.rs
│       └── lib.rs        # 파일오픈/메뉴/dialog 명령
└── README.md
```

- **frontendDist** → `../rhwp-studio/dist` (기존 빌드 산출물 재사용)
- **beforeBuildCommand** → WASM 복사 + `rhwp-studio` 빌드 (배포 워크플로와 동일 순서)

### 4.3 빌드 파이프라인 (기존과 동일 순서)

배포 워크플로(`deploy-pages.yml`)의 검증된 순서를 그대로 따른다:
1. `cp pkg/rhwp_bg.wasm pkg/rhwp.js rhwp-studio/public/`
2. `rhwp-studio` vite 빌드 → `rhwp-studio/dist`
3. `tauri build` 가 `dist`를 번들

### 4.4 주요 기술 쟁점 (1단계에서 반드시 해결)

| # | 쟁점 | 내용 | 대응 |
|---|------|------|------|
| T1 | **PWA Service Worker** | `vite.config.ts`의 `VitePWA(registerType:autoUpdate)`가 웹뷰 커스텀 프로토콜에서 불필요·문제 소지 | 데스크톱 빌드에선 PWA 비활성화 (env 플래그로 분기, 예: `VITE_TARGET=desktop`) |
| T2 | **base 경로** | 웹 배포는 `--base=/`. Tauri 자산 프로토콜에선 상대경로(`./`)가 안전할 수 있음 | 데스크톱 빌드 시 base 조정·검증 |
| T3 | **네이티브→웹 파일 전달** | 네이티브로 연 파일 bytes를 기존 문서 오픈 경로로 주입해야 함 | 4.5 참조 (핵심 설계 결정) |
| T4 | **워크스페이스 격리** | `src-tauri` 크레이트가 루트 `rhwp` 빌드에 영향 주면 안 됨 | 루트에 `[workspace]` 없음 확인됨 → 독립 크레이트로 자연 격리. 루트에 workspace 섹션 신설 금지 |

### 4.5 네이티브→웹 파일 전달 (핵심 설계 결정)

1단계 "그대로 감싸기"의 유일한 통합 지점. 네이티브가 연 파일(.hwp/.hwpx) bytes를 `rhwp-studio`의 기존 문서 오픈 코드 경로로 넘겨야 한다. 두 방식:

- **(A) 권장 — 웹 무해(inert) 가드 훅**: `rhwp-studio`에 `if (window.__TAURI__) { … }` 로 가드된 소형 데스크톱 브리지 모듈 추가. Tauri `open-file` 이벤트 수신 → 기존 오픈 함수에 bytes 전달. 브라우저에선 완전 no-op이라 **웹(GitHub Pages) 동작 동일**. 단, `rhwp-studio/` 변경이므로 merge 시 Pages 재배포 1회 발생(산출물은 기능상 동일).
- **(B) 대안 — studio 무수정**: Tauri `initializationScript`로 외부에서 파일 핸드오프를 구동. `rhwp-studio` 소스 무변경 → Pages 영향 0이지만 결합이 취약.

→ **수행계획서 단계에서는 (A)를 권장안으로 제시**. 가드의 웹 무해성을 단위로 검증한다. 최종 채택은 작업지시자 피드백으로 확정.

## 5. 네이티브 기능 상세 (1단계)

| 기능 | 구현 수단 (Tauri v2) |
|------|---------------------|
| .hwp/.hwpx 파일 연결 | `tauri.conf.json` `bundle` 파일 association + 단일 인스턴스 + open 이벤트 |
| 열기/저장 dialog | `@tauri-apps/plugin-dialog` |
| 메뉴바 (파일/편집/보기) | Tauri `Menu` API |
| 최근 문서 | `@tauri-apps/plugin-store` 또는 OS recent docs |
| 윈도우 상태 | `@tauri-apps/plugin-window-state` |
| 앱 표시명 | "HanPage" (productName) |

## 6. GitHub Pages 무영향 보장 (작업지시자 핵심 제약)

1. **`deploy-pages.yml` `paths-ignore`에 `rhwp-desktop/**` 추가** → 데스크톱 전용 변경 push는 배포 미트리거.
2. **WASM/렌더 엔진 무수정** → 웹 렌더 결과 불변.
3. **rhwp-studio 변경 최소화**: 4.5(A) 채택 시에도 가드된 web-inert 훅만 → 웹 동작 동일 (단, Pages 재배포 1회는 발생; 산출물 기능 동일).
4. **rhwp 엔진 식별자 유지**: crate명 `rhwp`, `@rhwp/*`, Edward Kim 저작권, `github.com/edwardkim/rhwp` 링크 불변.

## 7. Stage 구조 (개요 — 상세는 구현계획서 `task_m100_1_impl.md`)

| Stage | 내용 |
|-------|------|
| S1 | Tauri v2 스캐폴드 + `rhwp-desktop/` 골격 + 빈 윈도우에 rhwp-studio dist 로드 (PWA off, base 조정) |
| S2 | 네이티브 파일 열기/저장 dialog + 4.5 파일 전달 브리지 (web-inert 검증) |
| S3 | 파일 association (.hwp/.hwpx 더블클릭) + 메뉴바 + 최근문서 + 윈도우 상태 |
| S4 | 인스톨러 산출(.dmg 우선) + Pages 무영향 검증 + 최종 보고 |

> 단계 수는 3~6 범위. 구현계획서에서 확정·세분화한다.

## 8. Risk / 대응

| Risk | 영향 | 대응 |
|------|------|------|
| 웹뷰별 렌더 차이 (WebKit/WebView2/WebKitGTK) | WASM/Canvas 렌더 불일치 가능 | macOS(WebKit) 우선 검증, 타 OS는 산출만 |
| PWA SW가 데스크톱에서 잔존 | 캐시/업데이트 오동작 | T1 — 데스크톱 빌드 PWA 완전 비활성 |
| rhwp-studio 훅이 웹에 누수 | Pages 동작 변경 | 4.5 — `window.__TAURI__` 가드 + 단위 검증 |
| src-tauri가 루트 빌드 오염 | `cargo build`/WASM 영향 | T4 — 독립 크레이트, workspace 미신설 |
| 크로스 인스톨러(Win/Linux) | macOS에서 직접 산출 불가 | 1단계는 .dmg 우선, Win/Linux는 해당 OS/후속 CI(2단계) |

## 9. 검증 전략

- **빌드**: `rhwp-studio` 데스크톱 빌드 성공 + `tauri build` 성공
- **기능**: 앱에서 .hwp/.hwpx 열기 → 렌더 정상 (웹과 동일 결과)
- **네이티브**: 더블클릭 연결 / dialog / 메뉴 / 최근문서 / 윈도우 상태 동작
- **무영향 회귀**:
  - 루트 `cargo build` 결과 불변 (src-tauri 미포함 확인)
  - WASM 빌드 산출물 불변
  - `rhwp-studio` 웹 빌드 결과가 브라우저에서 동일 동작 (훅 web-inert)
  - `deploy-pages.yml` paths-ignore 동작 확인

## 10. 예상 작업량 / 산출물

- **작업량**: 중간 — Stage 4단계. macOS .dmg 산출까지가 1단계 완료 기준.
- **신규 산출물**: `rhwp-desktop/` 일체, `task_m100_1_impl.md`(구현계획서), 단계별 `task_m100_1_stage{N}.md`, 최종 `task_m100_1_report.md`.
- **기존 파일 수정(최소)**: `deploy-pages.yml`(paths-ignore 1줄), (4.5(A) 채택 시) `rhwp-studio` 데스크톱 브리지 가드 훅 + 빌드 분기.

진행 보고는 각 Stage 완료 시 작업지시자 승인 후 다음 Stage.
