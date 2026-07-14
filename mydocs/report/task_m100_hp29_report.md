# Task #29 최종 결과보고서 — 웹 데스크톱 앱 다운로드 버튼

- **이슈**: [paldyn/HanPage#29](https://github.com/paldyn/HanPage/issues/29) (M100) · **일자**: 2026-06-14
- **결과**: ✅ **완료** — devel 반영(`0e36227c`). + macOS updater CI 빈틈 동반 수정.

## 1. 목표·결과

웹(hanpage.paldyn.com / rhwp-studio) **상단 헤더에 OS 자동감지 데스크톱 앱 다운로드 버튼**. → 구현·검증 완료.

## 2. 구현 (웹 UI 한정)

- `rhwp-studio/src/ui/app-download.ts`(신규): OS 감지(`navigator`) + GitHub Releases API로 OS별 asset URL 취득 → 다운로드.
  - macOS → `*.dmg` / Windows → `*-setup.exe` / 기타 → 릴리스 페이지(폴백). API 실패 시도 폴백.
  - **Tauri 데스크톱 앱 내부에서는 미표시**(`__TAURI_INTERNALS__` 판별).
- `rhwp-studio/src/main.ts`: MenuBar 뒤에 버튼 설치.
- `rhwp-studio/src/styles/menu-bar.css`: 헤더 우측 정렬 버튼 스타일.

## 3. 동반 수정 — macOS 자동 업데이트(Task #26 후속)

v0.7.15 출시 직후 `latest.json`에 **windows 엔트리만 있고 darwin 누락**을 발견. 원인: `desktop-release.yml` macOS 잡이 `--bundles dmg`로 설치파일만 만들고 **updater 아티팩트(.app.tar.gz) 미생성**.
- **수정**: macOS args `--bundles dmg` → **`--bundles app,dmg`**.
- **재빌드**(v0.7.15 태그 이동): `HanPage_aarch64.app.tar.gz`(+`.sig`) 생성 + `latest.json`에 `darwin-aarch64`/`darwin-aarch64-app` 추가 → **macOS 자동 업데이트 동작**.

## 4. 검증

| 항목 | 결과 |
|------|------|
| studio `tsc` | 변경 파일 0 에러(잔여 4 = stale pkg) |
| 버튼 패턴 ↔ 릴리스 asset | `.dmg`/`-setup.exe` 일치 ✅ |
| 릴리스 재빌드(macOS 아티팩트) | ✅ `.app.tar.gz`+`.sig` |
| `latest.json` 전 플랫폼 | windows + **darwin** ✅ |
| 엔진/데스크톱 무관 | 웹 UI만 |

## 5. 한계·후속

- **웹 라이브 반영**: 버튼은 devel에 있으나, 라이브 웹(hanpage.paldyn.com)은 **Pages가 main에서 배포**되므로 **devel→main 반영(별도 릴리스 PR) 후** 노출됨.
- Linux 빌드 없음(현 CI dmg/nsis만) → Linux는 릴리스 페이지 폴백.

## 6. 산출물

- 계획서: `plans/task_m100_hp29.md` · 최종 보고서: 본 문서. devel 반영 완료(버튼 + CI 수정).
