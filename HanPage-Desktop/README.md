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

## 앱 아이콘 (04 한글 심볼)

원본 아이콘팩은 `src-tauri/icons/hanpage-04/`에 크기별 PNG, Windows ICO,
macOS ICNS, 투명 배경 심볼과 함께 보관한다. 앱에 적용되는 파일은 다음과 같다.

| 파일 | 적용 위치 |
| --- | --- |
| `src-tauri/icons/icon.png` | 1024×1024 원본 PNG, Unix 기본 런타임·PNG 번들 아이콘 |
| `src-tauri/icons/icon.ico` | Windows 실행 파일·창·NSIS 설치 프로그램 |
| `src-tauri/icons/icon.icns` | macOS 앱 번들·Dock |
| `src-tauri/icons/32x32.png`, `128x128.png`, `128x128@2x.png` | Tauri 번들 PNG (32/128/256px) |
| `src-tauri/icons/`의 추가 크기별 PNG | 16/24/48/64/256/512px PNG |
| `src-tauri/icons/Square*Logo.png`, `StoreLogo.png` | 같은 원본으로 생성한 Windows 타일 리소스 |

`src-tauri/tauri.conf.json`의 `bundle.icon`에서 위 기본 아이콘을 연결하며,
첫 PNG는 고해상도 `icon.png`를 사용한다. 기존 `npm run build` 및
`.github/workflows/desktop-release.yml`의 Windows NSIS/macOS app·DMG 빌드가
같은 설정을 사용하므로 재빌드하면 새 아이콘이 반영된다.

교체 전 아이콘 17개와 `tauri.conf.json`은
[`resources/branding/HanPage-previous-icons-20261001.zip`](resources/branding/HanPage-previous-icons-20261001.zip)에
백업했다. ZIP 안의 `backup-manifest.json`에는 각 원본의 SHA-256이 들어 있다.

공급된 ICO의 원본은 `src-tauri/icons/hanpage-04/HanPage.ico`에 그대로 보존한다. 실제
`src-tauri/icons/icon.ico`는 프레임 픽셀을 바꾸지 않고 256px 프레임을 첫 항목으로 정렬했다.
Tauri가 Windows 창 아이콘으로 첫 프레임을 읽으므로 고해상도 이미지를 사용하기 위함이다.

## Desktop 릴리스

앱 버전은 `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`,
`src-tauri/Cargo.lock`의 앱 항목과 `src-tauri/tauri.conf.json`에서 함께 맞춘다.
엔진·Studio 버전과 독립적으로 관리하며 현재 릴리스 후보는 0.8.6이다.

검증·병합된 commit에 `hanpage-desktop-v{버전}` 태그를 push하면
`desktop-release.yml`이 macOS aarch64와 Windows x64를 빌드하여 **초안 릴리스**에
첨부한다. 두 플랫폼의 설치파일, updater 서명과 `latest.json` 플랫폼 항목, macOS
서명·공증을 확인한 후 릴리스를 공개한다. devel 병합만으로 설치된 앱이 갱신되지 않는다.

변경 내용은 [변경 이력](CHANGELOG.md)을 참고한다.
