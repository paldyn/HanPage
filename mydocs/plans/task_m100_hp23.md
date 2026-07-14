# Task #23 — upstream(rhwp) 엔진 동기화: `devel`을 `upstream/devel` 기준 재베이스 + paldyn 레이어 재적용 (수행 계획서)

- 이슈: [paldyn/HanPage#23](https://github.com/paldyn/HanPage/issues/23) · 마일스톤 M100 · #18 후속(fork 관리)
- 브랜치: `local/task23` (origin/**main** 분기 — 사유 §4-1)
- 상태: **수행 계획 승인 대기** (구현 계획서는 승인 후 별도 작성)

## 1. 배경 / 목표

작업지시자 질의 "지금 upstream에서 업데이트되고 있는 내용 반영할 수 있어?" 에 대한 조사 결과, fork가 upstream보다 **엔진 PR 33개**만큼 뒤처져 있음을 확인했다. 단순 `git merge` 가 불가능한 구조이므로 **재베이스 전략**(작업지시자 결정)으로 upstream 엔진 갱신을 반영한다.

- **목표**: fork(`paldyn/HanPage`)가 upstream(`edwardkim/rhwp`)의 최신 HWP 엔진 개선을 흡수하고, 동시에 paldyn 고유 자산(HanPage 리브랜딩·HanPage-Desktop 앱·CI/Pages)을 보존한다.
- **부수 목표**: 향후 동기화를 "얇은 paldyn 레이어 리베이스"로 영구 단순화한다.

## 2. 현황 조사 (확정)

### 2-1. fork 구조 — upstream 리플레이 + 얇은 paldyn 레이어

| 항목 | 값 |
|------|-----|
| 분기점(merge-base) | `854515f5` (2026-04-08, Task #76) — main·devel 공통 |
| fork `devel` 분기 후 커밋 | 1693 (그중 **1665 patch-동일** = upstream 리플레이, **28 고유**) |
| fork `main` 분기 후 고유 | **56** (devel의 28 + #5/#7/#12/#13/#18 + 릴리스) |
| upstream tip | **PR #1228 / Task #1221** (2026-06-02 01:10, 하루 ~6 PR로 활발) |
| fork 엔진 추격 정지점 | **PR #1132 / Task #1131** |

> fork는 upstream과 독립 개발이 아니라 **upstream 히스토리를 리플레이**(같은 Task/PR 번호, 해시만 다름)한 위에 paldyn 레이어를 얹은 구조다. 그래서 `git merge upstream/devel` 시 merge-base가 분기점(#76)으로 잡혀 **1665 리플레이 커밋이 거짓 충돌(수천 건)** → 단순 머지 비현실적.

### 2-2. 누락된 upstream 갱신 = fork에 없는 PR 33개

`#1076, #1137, #1148~#1150, #1159, #1162~#1164, #1169, #1174~#1185, #1190, #1193/#1194, #1202/#1203, #1206~#1208, #1212/#1213, #1220, #1222/#1223, #1225/#1226, #1228`

전부 **HWP 엔진 개선**: 표 셀(중첩) 그림 복사, HWP5 수식-only 셀 z-표 행 압축, 수식 줄 한글 압축 해소, wrap=Square 호스트 본문 커서 전진, 문단 id 전역 유니크, textFlow roundtrip 보존 등. **paldyn 브랜딩과 무관한 엔진 영역.**

### 2-3. main/devel 불일치 (재베이스로 함께 해소)

- **main** = 완전한 paldyn 제품: `HanPage-Desktop/`(#13 리네임 반영) + #5/#7/#12/#13/#18 전부.
- **devel** = stale: 옛 `rhwp-desktop/`, #5~#18 미반영(이 작업들은 fork-native main-flow로 main에서 직접 수행됨).
- → **paldyn 레이어의 권위 원본은 `main`**. 재베이스는 devel을 main 수준 paldyn + upstream 엔진으로 재구성하여 이 불일치도 해소한다.

## 3. 전략: 재베이스 (근거)

`upstream/devel`을 새 기반으로 삼고 **paldyn 고유 레이어(main 기준 56커밋 중 보존분)만** 위에 재적용한다.

- **엔진 `src/` 충돌 0** — 엔진은 upstream에서 그대로 채택(리플레이 드리프트 제거).
- **`HanPage-Desktop/` 충돌 0** — upstream에 없는 자기완결 디렉터리, 최종 상태 그대로 적용.
- **충돌면 = 브랜딩/문서/에셋 44파일** — README·CLAUDE.md·로고·package 메타·CNAME·도메인 등. 기계적 해결("paldyn 정체성 유지, upstream 브랜딩-등가 변경 무시").
- **향후 동기화 영구 단순화** — 이후엔 "upstream 갱신 + 얇은 paldyn 레이어 재적용" 반복으로 충분.

## 4. 작업 범위

### 4-1. 작업 브랜치 분기 (관례 예외)

`local/task23`은 관례(`local/devel` 분기)와 달리 **origin/main에서 분기**한다. 사유: (a) paldyn 레이어 권위 원본이 main(§2-3), (b) 재베이스 산출물(새 devel)은 upstream/devel 기반이라 어차피 기존 devel 계보와 무관. 실제 재베이스 메커닉(체리픽 vs 브랜딩 변환 재적용, 커밋별 triage)은 **구현 계획서**에서 확정.

### 4-2. paldyn 레이어 처리 분류 (main 고유 56커밋)

| 분류 | 건수 | 처리 |
|------|------|------|
| 리브랜딩(hwpio→HanPage·도메인·로고·라이선스) | 11 | **재적용** (upstream 신버전 파일 위에 브랜딩 변환) |
| HanPage-Desktop 앱(Task #1/#5/#7/#12/#13/#18) | ~33 | **재적용** (최종 상태 = `HanPage-Desktop/` + mainBinaryName) |
| CI/Pages(gh-pages 배포·deploy-pages.yml) | 4 | **재적용** |
| 엔진/샘플 fix(Task #741후속/#993/#1052/#1061/#1064, textbox/이미지 fix, fixtures) | 10 | **개별 triage** — upstream 대체분이면 drop, 미반영 고유분이면 cherry-pick (구현 계획서에서 patch-id·내용 대조) |

### 4-3. 추가/비범위

- **추가**: 본 Task #23 문서(계획·단계보고·최종보고)·orders는 재베이스 산출 devel에 포함.
- **비범위(본 작업 제외)**:
  - `main` 직접 재작성 — devel 검증 후 **별도 릴리스 PR**로 반영(methodology devel→main 흐름).
  - 신규 릴리스 태그/버전 범프 — 엔진 동기화는 코드 반영까지, 릴리스는 별도 결정.
  - upstream/devel vs upstream/main 기준 선택 — 기본 **upstream/devel**(최신 엔진) 제안, 안정성 우려 시 upstream/main 대안(구현 계획서 확정 항목).

## 5. 보존 불변식 (절대 위반 금지)

- **rhwp 엔진 식별자 유지**: crate `rhwp`, `@rhwp/*`, `edwardkim.rhwp-vscode`, Edward Kim 저작권, `github.com/edwardkim/rhwp` 링크. → 재베이스가 upstream을 기반으로 하므로 **자연히 보존**(엔진 정체성 = upstream).
- **paldyn 서비스 브랜딩 재적용**: 제품명 HanPage, `hanpage.paldyn.com`, `paldyn/HanPage` 레포 경로, H-마크 로고, 재배포 라이선스 고지.
- **HanPage-Desktop 앱 보존**: 디렉터리·Tauri 설정·`mainBinaryName="HanPage"`·크레이트 내부명(`rhwp-desktop`/`rhwp_desktop_lib`) 유지.
- **GitHub Pages 무영향**: 배포는 `gh-pages` 브랜치 트리거 한정. 재베이스(devel)는 Pages 재배포를 일으키지 않음. 기존 릴리스/태그(`hanpage-desktop-v0.7.13`) 불변.
- **시크릿 금지**: `.env`/`.p8`/`.p12`/인증서/비밀번호 커밋 금지.

## 6. 충돌면 / 리스크

- **충돌면 44파일**(브랜딩/문서/에셋, §3) — upstream도 동일 파일을 수정했기 때문. 전부 엔진 외 영역이라 로직 회귀 위험 낮음. 로고/favicon 등 바이너리는 "paldyn 채택" 단순 결정.
- **엔진 fix 10건 triage 오판 리스크** — upstream 대체분을 잘못 drop하면 회귀. → patch-id + 내용 대조로 보수적 판정(구현 계획서).
- **히스토리 재작성(force-push devel)** — 협업 영향. 단 `local/devel`·`local/task`는 로컬 전용, 원격 push는 `devel`만(§워크플로). devel force-push 전 **명시적 승인** 필수.
- **upstream 고속 갱신** — 작업 중에도 upstream이 진행. 동기화 기준 커밋을 고정(예: 작업 시작 시점 upstream/devel)하고, 추가 갱신은 차기 사이클.

## 7. 검증 방법

- **엔진 동기화 확인**: 재베이스 후 fork devel과 upstream/devel(고정 기준)의 `src/` diff = 0(엔진은 upstream 그대로).
- **빌드**: `cargo build` / `cargo test`(네이티브, 로컬). WASM은 Docker(필요 시). HanPage-Desktop은 `cargo build`(Tauri 크레이트) 또는 CI.
- **브랜딩 무회귀**: HanPage·hanpage.paldyn.com·H-마크·CNAME·라이선스 고지 잔존 확인. `rhwp`/`edwardkim` 엔진 식별자 보존 확인.
- **누락 PR 흡수 확인**: §2-2의 33개 PR 주제가 fork devel에 반영됨(예: 표 셀 그림 복사, 문단 id 전역 유니크) — patch-id 또는 소스 대조.
- **paldyn 레이어 무손실**: HanPage-Desktop/ 전체·CI/Pages·mydocs HanPage 문서 잔존.

## 8. 산출물

- 재베이스된 `local/task23`(= upstream/devel 기준 + paldyn 레이어) → 검증 후 `devel` 갱신(force-push, 승인 후).
- Task #23 문서: 수행 계획서(본 문서)·구현 계획서·단계별 보고서·최종 보고서.
- `main` 반영은 후속 릴리스 PR(비범위).

## 9. 진행 절차 (methodology)

1. [x] 이슈 #23 등록 + `local/task23` 분기(origin/main)
2. [ ] **수행 계획서(본 문서) → 승인 요청** ← 현재
3. [ ] 구현 계획서(재베이스 메커닉·엔진fix triage·단계 3~6) → 승인 요청
4. [ ] 단계별 진행 + 단계 보고서
5. [ ] 최종 보고서 + orders 갱신 → 승인 → devel 반영
