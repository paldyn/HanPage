# HanPage-Desktop — HanPage 데스크톱 앱

[`rhwp-studio`](../rhwp-studio) 웹 에디터 + WASM 엔진을 [Tauri v2](https://v2.tauri.app/)
OS 웹뷰로 감싼 **HanPage** 데스크톱 앱이다. (제품 표시명: **HanPage**, 디렉터리
`HanPage-Desktop`, npm 패키지명 `hanpage-desktop`. 단 Rust 크레이트 내부 식별자는
비노출이라 `rhwp-desktop`/`rhwp_desktop_lib` 유지.)

## 설계 원칙 (1단계)

기존 웹 프런트엔드와 WASM 산출물을 **그대로** 로드하고, 브라우저가 줄 수 없는
네이티브 UX만 덧입힌다. 렌더/파싱 엔진(WASM)은 수정하지 않는다.

- **GitHub Pages 무영향**: 웹 기본 빌드(`rhwp-studio` `build`)는 변경하지 않는다.
  데스크톱은 `VITE_TARGET=desktop` 분기로 PWA/Service Worker를 끄고 상대 base(`./`)로
  빌드한다. 데스크톱 전용 변경은 `deploy-pages.yml` `paths-ignore`(`HanPage-Desktop/**`)로
  배포를 트리거하지 않는다.
- **빌드 격리**: `src-tauri`는 루트 워크스페이스에 포함되지 않는 독립 크레이트다.
  루트 `cargo build`/WASM 빌드에 영향을 주지 않는다.

## 사전 요구

- Rust (stable) — `rustup`
- Node.js 18+
- macOS: Xcode Command Line Tools (시스템 WebKit 사용)
- Tauri 전제조건: <https://v2.tauri.app/start/prerequisites/>

## 개발 / 빌드

```bash
cd HanPage-Desktop
npm install                 # @tauri-apps/cli 설치 (최초 1회)

npm run dev                 # 개발: rhwp-studio dev 서버(7700) + Tauri 창
npm run build               # 릴리스: 프런트 빌드 + .dmg/.app 번들 산출
```

- `npm run build:frontend` — WASM 복사 + `rhwp-studio` 데스크톱 빌드(`base=./`, PWA off)
- 빌드 산출물(프런트): `../rhwp-studio/dist` (Tauri `frontendDist`)
- 번들 산출물: `src-tauri/target/release/bundle/`

## 범위 로드맵

- **1단계 (현재, Task #1)**: rhwp-studio 래핑 + 네이티브 열기/저장·파일연결·메뉴·
  최근문서·윈도우 상태 + macOS `.dmg`.
- **2단계 이후**: 네이티브 `rlib` 코어 직접 호출, 자동 업데이트, 코드 서명·공증,
  Windows/Linux 인스톨러 CI.
