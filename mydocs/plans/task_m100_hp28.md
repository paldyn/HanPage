# Task #28 — 데스크톱 0.7.15 출시 (엔진 0.7.15 동기화) 수행 계획서

- **이슈**: [paldyn/HanPage#28](https://github.com/paldyn/HanPage/issues/28) (M100)
- **브랜치**: `local/task28` (devel 분기) · 재구축: `local/task28-rebase`
- **상태**: 수행계획 승인 대기 · **일자**: 2026-06-14

## 0. 목표

오픈소스(rhwp) 버전을 따라 데스크톱을 **0.7.15**로 출시. 현재 엔진 0.7.13 → upstream **0.7.15** 재동기화 후, 기존 v0.7.13 릴리스를 v0.7.15로 교체.

## 1. 현황

- upstream rhwp 최신 = **0.7.15** (release 2026-06-06). 고정 기반 = `upstream/devel 9172e3a2`.
- 현재 devel 엔진 = 0.7.13(Task #23 고정 f6ffe9d6). 델타 = 533커밋·src 103파일. pdf-large LFS 재포함(strip 필요).
- 현재 devel paldyn 레이어 = 브랜딩 + HanPage-Desktop(#26 updater 포함) + CI/Pages + #27 build:desktop + mydocs. **이것을 0.7.15 기반에 재적용**.

## 2. 접근 — Task #23 재베이스 레시피 재사용

Task #23에서 확립한 방법을 0.7.15 기반에 그대로 적용(투자된 조사·규칙 재사용으로 신속):

- **기반** = upstream/devel 0.7.15 고정.
- **paldyn 레이어 재적용**(현 devel을 소스로):
  - 순수 paldyn(wholesale 채택): `HanPage-Desktop/`·studio 글루(`desktop-bridge.ts`·`file.ts`·`main.ts` 배선)·`public/{CNAME,LICENSE}`·로고/파비콘·`web/fonts` 라이선스·`build:desktop`+`cross-env`·CI(`deploy-pages.yml`·`desktop-release.yml`+서명 env)·mydocs.
  - 브랜딩(surgical, 0.7.15 신버전 위): 레포→`paldyn/HanPage`·도메인→`hanpage.paldyn.com`·제품명→HanPage. **엔진 식별자(rhwp/@rhwp/edwardkim) 보존**.
- **pdf-large + .gitattributes 제거**(LFS 예산 문제 동일 — filter-repo).
- **버전**: 엔진은 기반이 0.7.15라 자동 / 데스크톱 `tauri.conf.json`·`package.json` → **0.7.15**.

## 3. 보존 불변식

- rhwp 엔진 식별자 보존 · **자동 업데이트(#26) 유지**(updater config·플러그인·서명 env·키 시크릿) · 시크릿 금지 · GitHub Pages 무영향 · 크레이트 내부명 유지.

## 4. 단계 (4)

| 단계 | 내용 |
|------|------|
| **Stage 1** | 기준 고정(`9172e3a2`) + `local/task28-rebase` 생성 + sanity 빌드 |
| **Stage 2** | paldyn 레이어 재적용 + pdf-large strip + 데스크톱 버전 0.7.15 |
| **Stage 3** | 검증: `cargo build`/`test`·**엔진 src diff vs 0.7.15 = 0**·자동업데이트 유지·(CI dispatch 빌드+서명) |
| **Stage 4** | 최종 보고 → **백업 태그 + devel force-push** → 기존 `v0.7.13` 릴리스·태그 삭제 → `hanpage-desktop-v0.7.15` 태그 push(CI 자동 발행) |

## 5. 리스크/롤백

- **devel force-push**: 직전 `backup/devel-pre-task28` 태그(원격) + 번들 백업. 문제 시 즉시 환원.
- **pdf-large LFS**: Task #23와 동일 — filter-repo로 히스토리 제거(향후 동기화도 동일 절차).
- **릴리스 삭제**: 되돌리기 어려움 — Stage 4에서 작업지시자 최종 확인 후 실행.
- **실제 발행**: 태그 push 후 CI 빌드 ~12분. 서명 키(#26 시크릿) 그대로 사용.

## 6. 비범위

- main 반영(별도) · macOS 공증(#4 일시정지) · 새 기능(엔진/데스크톱) 추가.
