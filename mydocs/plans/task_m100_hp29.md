# Task #29 — 웹 데스크톱 앱 다운로드 버튼 (수행 계획서)

- **이슈**: [paldyn/HanPage#29](https://github.com/paldyn/HanPage/issues/29) (M100) · **브랜치**: `local/task29`(devel 분기)
- **상태**: 수행계획 승인 대기 · **일자**: 2026-06-14

## 0. 목표·확정

웹(hanpage.paldyn.com / rhwp-studio) **상단 헤더에 OS 자동감지 데스크톱 앱 다운로드 버튼**. 작업지시자 확정: **상단 헤더 버튼**.

## 1. 기술 접근

- **배치**: `#menu-bar` 우측 끝에 버튼("데스크톱 앱 다운로드").
- **OS 감지**: `navigator.userAgentData?.platform` 우선, `navigator.userAgent`/`platform` 폴백 → macOS / Windows / 기타.
- **다운로드 대상**: GitHub Releases API(`api.github.com/repos/paldyn/HanPage/releases/latest`) → asset에서 OS별 선택:
  - macOS → `*.dmg`(aarch64) / Windows → `*-setup.exe`(x64) / 기타(Linux 등) → 릴리스 페이지.
  - 버전이 파일명에 박혀 고정 URL 불가 → API로 `browser_download_url` 취득. GitHub API는 CORS 허용(웹 fetch 가능).
- **데스크톱 앱 내부에서는 숨김**: Tauri 환경 감지(`__TAURI_INTERNALS__`/`__TAURI__` 또는 desktop-bridge 판별) 시 버튼 미표시.
- **폴백**: API 실패/asset 없음/OS 미감지 → 릴리스 페이지(`/releases/latest`) 링크.

## 2. 변경 범위 (웹 UI 한정)

| 파일 | 변경 |
|------|------|
| `rhwp-studio/src/ui/app-download.ts` (신규) | OS 감지 + 최신 릴리스 asset 조회 + 버튼 생성·클릭 핸들러 |
| `rhwp-studio/index.html` | `#menu-bar` 우측에 버튼 컨테이너(또는 main.ts에서 동적 삽입) |
| `rhwp-studio/src/main.ts` | 초기화 시 버튼 설치(데스크톱이면 skip) |
| `rhwp-studio/src/styles/*` | 버튼 스타일(sb-/md- 규약 준수) |

## 3. 불변식·무영향

- **데스크톱/엔진 무관**(웹 UI에만 추가) · GitHub Pages 배포 기존 흐름 · 시크릿 없음 · rhwp 엔진 식별자 무관.

## 4. 단계 (3)

| 단계 | 내용 |
|------|------|
| **Stage 1** | OS 감지 + 릴리스 asset 조회 유틸 + 헤더 버튼(설치·클릭) |
| **Stage 2** | 데스크톱 숨김 + 폴백 + 스타일 + studio 빌드 검증 |
| **Stage 3** | 최종 보고 + devel 반영 |

## 5. 비범위

- Linux 빌드 추가(현 CI는 dmg/nsis만) · 다운로드 통계 · 자동 업데이트(#26, 별개).
