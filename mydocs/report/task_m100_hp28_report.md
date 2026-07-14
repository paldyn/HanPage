# Task #28 최종 결과보고서 — 데스크톱 0.7.15 출시 (엔진 0.7.15 동기화)

- **이슈**: [paldyn/HanPage#28](https://github.com/paldyn/HanPage/issues/28) (M100) · **일자**: 2026-06-14
- **결과**: ✅ **완료** — `HanPage Desktop-v0.7.15` 발행, devel=`0e36227c`(이후 #29 머지)

## 1. 목표·결과

오픈소스(rhwp) 버전을 따라 데스크톱을 **0.7.15**로 출시. 엔진 0.7.13 → upstream **0.7.15** 재동기화 후 기존 v0.7.13 릴리스를 v0.7.15로 교체. → 달성.

## 2. 수행 (Task #23 재베이스 레시피 재사용)

- **기반**: upstream/devel `9172e3a2`(0.7.15) 고정. 델타 533커밋·src 103파일.
- **paldyn 레이어 재적용**: wholesale 65 + 삭제 5 + surgical 7(브랜드 URL·build:desktop·vite 데스크톱/브랜드·main.ts 배선) + mydocs 46.
- **pdf-large**: HEAD 제거 + filter-repo 히스토리 제거(LFS 예산 문제 동일).
- **버전**: 엔진 0.7.15(기반) + 데스크톱 0.7.15.

## 3. 검증

| 항목 | 결과 |
|------|------|
| 엔진 `cargo test` | ✅ **2310 passed / 0 failed** |
| 데스크톱 cargo check | ✅ rhwp-desktop v0.7.15 |
| 엔진 src diff vs 0.7.15 | **0** |
| 자동 업데이트(#26) 유지 | ✅ updater·서명 env·키 |
| 엔진 식별자 보존 | ✅ rhwp·@rhwp·edwardkim |
| 시크릿/개인키 | 0 |
| CI dispatch(빌드+서명) | ✅ Win+mac, `.sig` 생성 |

## 4. 출시

- 백업 태그 `backup/devel-pre-task28`(원격) + 번들 → **devel force-push**(`efb097ec`→재구축).
- 기존 **릴리스/태그 `v0.7.13` 삭제** → **`hanpage-desktop-v0.7.15`** 태그 → CI 자동 발행.
- **릴리스 자산**: `HanPage_0.7.15_aarch64.dmg`·`_x64-setup.exe`(+`.sig`)·`*.app.tar.gz`(+`.sig`)·`latest.json`(windows+darwin).

## 5. 효과

- 새로 받는 사용자는 처음부터 **0.7.15 + 자동 업데이트** 버전을 받음(부트스트랩 해소).
- 자동 업데이트: **Windows·macOS 모두 동작**(latest.json 전 플랫폼 — macOS는 #29 후속 CI 수정으로 완성).

## 6. 한계·후속

- **main 미반영**: 본 작업은 devel 한정. 웹(hanpage.paldyn.com, Pages는 main 배포)·main 반영은 별도 릴리스 PR.
- macOS 공증(#4 일시정지): 미공증 Gatekeeper 경고 잔존.
- 차기 upstream 동기화도 pdf-large filter-repo 필요(구조적).

## 7. 산출물

- 계획서: `plans/task_m100_hp28.md` · 최종 보고서: 본 문서. devel 반영 완료.
