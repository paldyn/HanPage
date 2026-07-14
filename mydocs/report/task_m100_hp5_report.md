# Task #5 (HanPage) — 데스크톱 아이콘 macOS 규격화: 최종 결과 보고서

- 이슈: [paldyn/HanPage#5](https://github.com/paldyn/HanPage/issues/5) · 브랜치: `local/task5` (main 기준)
- 마일스톤: M100 (v1.0.0) — 데스크톱 앱 후속, Task #4(서명)와 **독립**
- 계획서: `mydocs/plans/task_m100_hp5.md` · Stage 2 보고서: `mydocs/working/task_m100_hp5_stage2.md`
- 상태: **구현·검증·로컬 재빌드 완료 → main 머지 PR + 이슈 클로즈 승인 대기**

## 1. 목표 / 수용 기준 달성

| 수용 기준 | 결과 |
|-----------|------|
| 빨강·파랑 **H 아트워크 유지** + macOS 둥근 타일 + 투명 여백 | ✓ |
| Dock/Finder에서 인접 앱과 **동일 크기·정렬** | ✓ (full-bleed → 824/1024 그리드 + 여백 100px) |
| 데스크톱 아이콘 자산(`.png`/`.icns`/`.ico`/Windows Square 로고) **정상 재생성** | ✓ (17개 자산) |

## 2. 최종 산출물

- **마스터**: `output/poc/icon/hanpage_icon_64.png` (1024², 작업지시자 시각 판정 채택본)
  - 그리드: 1024 캔버스 / **824 둥근 타일** / 모서리 반경 **185**(≈0.2237×824) / 여백 **100px**
  - H 글리프 = **타일 채움의 63%**(작업지시자 라벨 "64") 중앙 배치
- **재생성 자산**: `rhwp-desktop/src-tauri/icons/` **17파일** (`icon.png` 512² / `icon.icns` / `icon.ico` + `32/64/128/128@2x` + `Square*Logo`×9 + `StoreLogo`)
- 커밋: `c2a1bd03` (보정본) — Stage 2 커밋 `620fdba5`(구버전 스케일)을 **대체**

## 3. 핵심 사건 — 생성 스크립트 소스 버그 보정

Stage 2 산출 아이콘은 H 글리프가 타일의 **~42%로 작게** 보였다(시각 판정 시 "심볼이 작다" 반복 지적). 원인 추적 결과:

- **근본 원인**: `output/poc/icon/make_icon.py` 가 소스로 읽던 `rhwp-desktop/src-tauri/icons/icon.png` 를 `npx tauri icon` 이 매번 **덮어씀**. 직전 회차의 출력(흰 타일+H)이 다음 회차의 입력이 되어, `getbbox` 가 **타일 전체**(또는 AA 후광)를 H로 오인 → H 가 **이중 축소**됨. 라벨은 "64%"였으나 실제 채움은 31~42%.
- **보정**:
  1. 소스를 git 원본(full-bleed H, 커밋 `51053737`)에서 복원한 **고정 파일** `output/poc/icon/_src_fullbleed.png` 로 분리 (자기 출력 재입력 차단).
  2. **solid-ink 임계 bbox** 추출 — 흰색과 40레벨↑ 차이나는 픽셀만 H 잉크로 간주(AA 후광 오검출 제거).
  3. H = **타일 채움의 63%**(라벨 "64")로 재생성.
- **부수 안전장치**: H 크롭의 흰 배경 사각형이 둥근 타일 밖으로 삐져나오지 않도록 `ImageChops.darker(icon_alpha, tile_alpha)` 로 알파를 타일 모양에 클립.

> 교훈: `tauri icon` 의 출력 대상(`icons/icon.png`)을 생성 스크립트의 **입력으로 재사용 금지**. 소스는 항상 불변 원본을 분리 보관.

## 4. 검증

### 4.1 자산 정합 (정적)

- **파일 세트**: `icons/` = **17개**, iOS/Android 하위폴더 없음 ✓
- **치수/포맷**: `icon.png` = `PNG 512×512 8-bit RGBA non-interlaced` ✓ / `icon.icns` = *Mac OS X icon*, `icon.ico` = *MS Windows icon* ✓
- **알파(투명 여백) 보존 — 핵심**: 재생성 `icon.png` 픽셀 검사
  - 모서리 `(2,2)` = `RGBA(0,0,0,0)` **투명** (Dock 흰 사각형 회귀 없음)
  - 중앙 `(256,256)` = `RGBA(5,69,156,255)` **불투명 파랑**(H 잉크)
  - H 잉크 폭 ≈ 260px / 타일 412px(512 스케일) = **63% 채움** ✓
- **범위**: `git diff` 결과 `rhwp-desktop/src-tauri/icons/` **17파일만** ✓

### 4.2 로컬 재빌드 + Dock 실제 반영 (동적)

- `cd rhwp-desktop && npm run build`(= `tauri build`) **exit 0** →
  `HanPage.app` + `HanPage_0.7.13_aarch64.dmg` 번들 생성 ([로그](../../output/poc/icon/) 참조: 빌드 17.05s, bundle 2종).
- `/Applications/HanPage.app` 가 신규 번들과 동기화 → `icon.icns` 해시 `2d9189bc…` 소스/번들/설치본 **일치**.
- Launch Services 재등록(`lsregister -f`) + `killall Dock` → **Dock 실제 반영 확인**.
- 시각 확인: `output/poc/icon/_installed64_proof.png` (어두운 배경, 1024→Dock 크기 다단) — 흰 둥근 타일 + 투명 여백 + 중앙 H 63% ✓

## 5. 무영향 (불변 제약 — 준수)

- 변경: **`rhwp-desktop/src-tauri/icons/` 17파일만**.
- `deploy-pages.yml` / 커밋 `Cargo.toml` / **rhwp 엔진 식별자**(crate `rhwp`, `@rhwp/*`, Edward Kim 저작권/링크) / Task #4(`desktop-release.yml`) **전부 불변**.
- `output/poc/icon/` 산출물(생성 스크립트·후보·증빙·`_src_fullbleed.png`)은 `.gitignore` 대상 → **커밋 비포함**.
- 빌드 중 `tauri build` 의 `copy-wasm` 단계가 `rhwp-studio/public/rhwp.js` 를 `pkg/` 빌드본으로 덮어쓴 **빌드 부산물 drift** 가 작업트리에 노출되었으나, Task #5 범위 밖이라 **baseline 복원**(`git checkout`)하여 커밋에서 제외. (별도 관찰 사항 §7)

## 6. 브랜치 커밋 이력 (`local/task5`)

| 커밋 | 내용 |
|------|------|
| `4e09e0b9` | 수행계획서 + 오늘할일 |
| `620fdba5` | Stage 2 — 아이콘 재생성 (구버전 스케일, 보정으로 대체됨) |
| `c2a1bd03` | **아이콘 H 글리프 스케일 보정** (소스 버그 수정, H 63% 채움) |

## 7. 관찰 사항 (Out of Scope — 별도 처리 권장)

- `rhwp-studio/public/rhwp.js`(커밋본)와 `pkg/rhwp.js`(로컬 빌드본) 사이에 **wasm-bindgen 재생성 drift** 존재. 단순 함수해시 차이뿐 아니라 글루 버그 수정으로 보이는 변경(`HwpViewer.pageCount()` 가 `wasm.hwpdocument_pageCount` → `wasm.hwpviewer_pageCount`)도 포함. 커밋된 `public/rhwp.js` 가 **stale** 일 가능성. Task #5와 무관하므로 **별도 이슈로 조사** 권장.

## 8. 다음 (Stage 3 — 승인 게이트)

- [ ] (작업지시자 승인) `local/task5` → `main` 머지 PR 생성·머지.
- [ ] (작업지시자 승인) 이슈 #5 클로즈.
- 다운로드 사용자 반영은 차기 `desktop-v*` 릴리스 CI 빌드 시 자동 포함.
