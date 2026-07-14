# Task #5 (HanPage) — 데스크톱 아이콘 macOS 규격화 (수행계획서)

> **파일명 규칙**: fork-native 과제는 `task_m100_hp{이슈}.md` (Task #4와 동일 네임스페이스).
> 단일 자산(아이콘) 스왑이라 **수행+구현을 본 문서에 통합**하고 별도 `_impl.md`는 생략한다.

- 이슈: [paldyn/HanPage#5](https://github.com/paldyn/HanPage/issues/5)
- 브랜치: `local/task5` (main 기준 분기 — 데스크톱 아이콘은 main에 존재)
- 마일스톤: M100 (v1.0.0) — 데스크톱 앱 후속, Task #4(서명)와 **독립**
- 상태: **시각 판정 완료(H 64%) → 구현(Stage 2) 진행**

## 1. 배경

현재 `rhwp-desktop/src-tauri/icons/` 아이콘은 흰 배경을 정사각형 끝까지 꽉 채운 **full-bleed** 형태(Tauri 기본 `tauri icon` 생성 결과)다. macOS는 앱 아이콘을 둥근 사각형(squircle) + 바깥 투명 여백으로 그리도록 권장(HIG)하는데, 여백이 없어 Dock/Finder에서 **다른 앱보다 크고 각지게** 보인다.

## 2. 목표 / 수용 기준

- [ ] 빨강·파랑 **H 아트워크 유지** + macOS 둥근 타일 + 투명 여백
- [ ] Dock/Finder에서 인접 앱과 **동일 크기·정렬**
- [ ] 데스크톱 아이콘 자산(`.png`/`.icns`/`.ico` 및 Windows Square 로고) **정상 재생성**

## 3. 기술 접근 (확정)

- **그리드**: 1024 캔버스 / **824 둥근 타일** / 모서리 반경 **185**(≈0.2237×824) / 여백 **100**
- **합성**: 원본 `icon.png`(512)에서 H 영역 추출 → 흰 둥근 타일 위 **H=타일의 64%** 크기로 중앙 배치
  - 도구: Pillow (`output/poc/icon/make_icon.py`), 둥근 모서리는 ×4 슈퍼샘플→다운스케일로 AA
- **마스터**: `output/poc/icon/hanpage_icon_64.png` (1024, **시각 판정 채택본**)
- **재생성**: `cd rhwp-desktop && npx tauri icon <master>` → `src-tauri/icons/` 전체 덮어쓰기

## 4. 범위 / 무영향 (하드 제약)

- 변경: **`rhwp-desktop/src-tauri/icons/`** 만 (데스크톱 전용 자산)
- 무영향: GitHub Pages(`deploy-pages.yml`/커밋 `Cargo.toml`), **rhwp 엔진 식별자**, Task #4(`desktop-release.yml`)
- `output/poc/icon/` 산출물은 `.gitignore` 대상 → 커밋 비포함(시각 판정용)

## 5. 단계

| Stage | 내용 | 게이트 |
|-------|------|--------|
| 1 | 후보 생성(H 52/58/64) + **시각 판정** → **H64% 채택** | (완료) |
| 2 | `icons/` 재생성 + 자산 검증(파일 세트/치수/시각 확인) | 보고 후 |
| 3 | main 머지 PR + 최종 보고 + (승인 후) 이슈 클로즈 | 각 행위별 승인 |

## 6. 위험·주의

- `tauri icon` 이 `icons/` 전체를 덮어씀 → 기존 파일은 git 추적이라 **되돌리기 가능**(`git checkout`).
- 아이콘은 **정적 자산**일 뿐 빌드 로직/엔진과 무관 → 회귀 위험 최소. 전체 `tauri build` 는 선택(자산 검증으로 충분, 필요 시 작업지시자 요청으로 로컬 빌드).
- 실제 Dock 반영은 **재빌드+재설치** 후 확인 가능(선택).
