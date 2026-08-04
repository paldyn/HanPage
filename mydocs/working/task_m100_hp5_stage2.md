# Task #5 (HanPage) — Stage 2 완료 보고서: 아이콘 재생성 + 검증

- 이슈: [paldyn/HanPage#5](https://github.com/paldyn/HanPage/issues/5) · 브랜치: `local/task5`
- 계획서: `task_m100_hp5.md` (Stage 2)

## 구현

- 마스터: `output/poc/icon/hanpage_icon_64.png` (시각 판정 채택본, H = 타일의 64%)
- `cd rhwp-desktop && npx tauri icon <master>` 로 `src-tauri/icons/` 재생성
- `tauri icon` 이 iOS/Android 아이콘(`icons/ios/`, `icons/android/`)도 생성 → **데스크톱 전용**이므로 두 하위폴더 **제거**
- 결과: 기존 **17개 자산만 갱신** — `icon.png`/`icon.icns`/`icon.ico` + `32/64/128/128@2x` + `Square*Logo`×9 + `StoreLogo`

## 검증

- **파일 세트**: `icons/` = 17개, 신규 하위폴더 없음 ✓
- **치수**: `icon.png` 512² / `128x128@2x.png` 256² / `hasAlpha: yes` ✓
- **유효성**(`file`): `icon.icns` = *Mac OS X icon*, `icon.ico` = *MS Windows icon (32+16)* ✓
- **알파 보존(핵심)**: 재생성 `icon.png` 픽셀 검사 —
  - 모서리 `(2,2)` = `RGBA(0,0,0,0)` 투명, 좌변 여백 `(2,256)` 투명
  - 타일 안쪽 `(120,120)` = `RGBA(255,255,255,255)` 흰색, 중앙 `(256,256)` = H(불투명)
  - → `tauri icon` 이 투명 여백을 흰색으로 **평탄화하지 않음** = Dock에서 흰 사각형 회귀 **없음** ✓
- **시각**: `output/poc/icon/_verify_dark.png` (어두운 배경 합성) — 흰 둥근 타일 + 투명 여백 + 중앙 H 확인 ✓

## 무영향 (불변 제약)

- `git diff` 범위: `rhwp-desktop/src-tauri/icons/` **17파일만**.
- `deploy-pages.yml` / 커밋 `Cargo.toml` / rhwp 엔진 식별자 / `desktop-release.yml`(Task #4) **불변**.

## 다음 (Stage 3)

- (작업지시자 승인) `local/task5` → `main` 머지 PR.
- (선택) 로컬 `tauri build` 재빌드 + 재설치로 **Dock 실제 반영** 확인.
- 최종 보고 + (승인 후) 이슈 #5 클로즈.
- 다운로드 사용자 반영은 차기 `desktop-v*` 릴리스 CI 빌드 시 자동 포함.
