# Task #23 — upstream 재베이스 (구현 계획서)

- 이슈: [paldyn/HanPage#23](https://github.com/paldyn/HanPage/issues/23) · 수행 계획서: `task_m100_hp23.md`(승인됨)
- 브랜치: `local/task23`(origin/main 분기) · 재구축 산출 브랜치: `local/task23-rebase`(Stage 1 생성)
- 상태: **구현 계획 승인 대기**

## 0. 전제

수행 계획서(전략·범위·불변식) 승인 완료. 본 문서는 **재베이스 메커닉·엔진fix triage 확정·단계(5)**를 정의한다. 소스/히스토리 직접 변경은 단계별 승인 후, devel 반영(force-push)은 최종 보고 승인 후.

## 1. 확정된 사전 결정 (조사 완료)

### 1-1. 엔진fix 10건 triage = **전부 DROP** (upstream/devel 대체분 확인)

main 고유 56커밋 중 비-브랜딩·비-데스크톱 10건은 모두 upstream/devel에 동등 이상 반영됨 → 재적용 불필요(재베이스가 upstream 버전을 그대로 흡수).

| fork 커밋 | 주제 | upstream/devel 대체 근거 | 판정 |
|-----------|------|--------------------------|------|
| `ec9ce12f` | Task #741 후속 외부 file path 그림 | `4bfcee05`(동일 제목) + #741 saga | DROP |
| `e7ee056b` | Fix textbox picture rendering | `layout.rs` has_real_text/get_inline_shape_position/FFFC **존재** + 샘플 hy-001 보유 | DROP |
| `96214c03` | Task #993 HWPX→HWP cell contracts | `a6bd1cb5`(동일 제목) | DROP |
| `f9b5b584` | 이미지 렌더링 버그 예제 | 샘플 hy-002 upstream 보유 | DROP |
| `a5511168` | Task #1052 글상자 각주 누락 | `a0391ee8` merge(closes #1052) | DROP |
| `9de60328` | Task #1061 Stage 1 HWPX Equation | `9fbf518b` 완본(EQEDIT errata, **supersede**) | DROP |
| `acba85af` | Task #1064 el-school 조사 | `ee7b1e2f` merge(closes #1064) | DROP |
| `df34df9c` | samples shape-001(#1067) | `22318bb2` Task #1067 실제 fix | DROP |
| `66a49bb0` | PR #1101 fixture | `68fecab4`(동일 제목) | DROP |
| `55c4bb0a` | Git LFS pdf-large 추적 | upstream pdf-large 보유 + fork 자체 `270a27a5`로 룰 제거(상쇄) → 인프라(.gitattributes)에서 처리 | DROP(엔진 무관) |

> 결론: **paldyn 레이어 = 순수 비엔진**(리브랜딩 + HanPage-Desktop + CI/Pages). 엔진 코드 0줄 이식.

### 1-2. 재베이스 기준 = `upstream/devel` 고정

Stage 1에서 **현재 tip SHA를 고정 기록**(작업 시작 시점 `f83c43b5` 부근). 작업 중 upstream 추가 갱신은 차기 사이클. (upstream/main 대안은 채택 안 함 — devel이 최신 엔진.)

### 1-3. 메커닉 = **rebuild-fresh** (56커밋 체리픽 아님)

paldyn 레이어가 순수 비엔진이므로, 데스크톱 saga(rhwp-desktop 생성→#13 리네임→#18 수정)의 중간 상태를 재현하지 않고 **최종 상태를 얇은 신규 커밋으로 재구성**한다:

1. `local/task23-rebase` ← 고정한 upstream/devel.
2. `HanPage-Desktop/` **최종 상태**를 main에서 그대로 이식(자기완결 디렉터리, upstream 무충돌).
3. **리브랜딩 변환**을 upstream 신버전 44파일 위에 재적용(옛 diff 재생이 아니라 net 결과).
4. **CI/Pages** 재적용.
5. **mydocs paldyn 문서** 추가.

대안(체리픽 46커밋)은 중간 상태 충돌·노이즈로 비채택. rebuild-fresh가 깔끔.

## 2. paldyn 레이어 재적용 명세

### 2-1. HanPage-Desktop 앱 (무충돌)
- main의 `HanPage-Desktop/` 28파일 최종 상태 복사(`mainBinaryName="HanPage"`·version 0.7.13·크레이트 내부명 `rhwp-desktop`/`rhwp_desktop_lib` 유지).
- upstream에 `HanPage-Desktop/` 없음 → 순수 추가.

### 2-2. 리브랜딩 (44파일 충돌면, 기계적)
- 제품명 `hwpio`/`rhwp`(서비스 표면) → **HanPage**, 도메인 → `hanpage.paldyn.com`, 레포 → `paldyn/HanPage`, 로고/favicon → H-마크, `rhwp-studio/public/CNAME`, 재배포 라이선스 고지.
- 대상: README·README_EN·CLAUDE.md·CONTRIBUTING·.github/(SECURITY·CODE_OF_CONDUCT)·Cargo.toml(workspace 메타)·npm/·rhwp-studio(index.html·about-dialog.ts·font-loader.ts·vite.config.ts·아이콘)·rhwp-vscode(README·package.json)·확장 PRIVACY·web/fonts 라이선스·.gitattributes·.gitignore.
- **불변식**: 엔진 식별자(crate `rhwp`, `@rhwp/*`, `edwardkim.rhwp-vscode`, Edward Kim 저작권, `github.com/edwardkim/rhwp` 링크)는 **유지**. 서비스 표면만 리브랜딩.

### 2-3. CI/Pages
- `deploy-pages.yml`: gh-pages 직접 배포(peaceiris) + **paths-ignore `HanPage-Desktop/**`**(데스크톱 변경이 Pages 미트리거).
- `desktop-release.yml`: `hanpage-desktop-v*` 태그 트리거(존재 시 유지/이식).
- npm trusted publishing 조정(해당 시).

### 2-4. mydocs
- 기준 = upstream/devel mydocs(엔진 기록) **+ paldyn HanPage 전용 문서 추가/우선**:
  - `plans/task_m100_hp{1,5,7,12,13,18,23}*`·`report/*hp*`·`working/*hp*` (hp-접두어 = 신규 파일, 무충돌).
  - `orders/2026{0530,0531,0601,0602}.md` (HanPage 활동 기록) — 동일 날짜 충돌 시 **paldyn 버전 우선**(fork 활동 기록). 저위험·가역.

## 3. 단계 (5)

| 단계 | 내용 | 산출/검증 |
|------|------|-----------|
| **Stage 1** | 기준 고정(upstream/devel SHA 기록) + `local/task23-rebase` 생성 + triage 확정(§1-1) | stage1 보고서: 고정 SHA·triage 표·기반 빌드 sanity(`cargo build`) |
| **Stage 2** | HanPage-Desktop 이식(§2-1) | `HanPage-Desktop/` 28파일 존재·tauri.conf node-parse OK·크레이트 내부명 유지 |
| **Stage 3** | 리브랜딩 + CI/Pages 재적용(§2-2·2-3) | 44파일 해결·HanPage 브랜딩 잔존·엔진 식별자(rhwp/edwardkim) 보존·Pages 격리 |
| **Stage 4** | 검증 | `cargo build`+`cargo test`(네이티브)·**엔진 `src/` diff vs 고정 upstream = 0**·누락 33 PR 흡수 확인·브랜딩/데스크톱/mydocs 무손실 |
| **Stage 5** | 최종 보고서 + orders 갱신 → 승인 → **devel 반영** | 보고서·orders 커밋. 승인 후 백업 태그 생성 → `devel` force-push 후 검증 |

각 단계 완료 후 단계 보고서 + 승인. 소스/문서 커밋은 `local/task23`(문서)·`local/task23-rebase`(재구축)에서.

## 4. 검증 상세 (Stage 4)

- **엔진 동기화 증명**: `git diff <고정 upstream/devel> local/task23-rebase -- src/` = 0 (엔진은 upstream 그대로).
- **빌드/테스트**: `cargo build` / `cargo test`(로컬 네이티브). WASM은 필요 시 Docker. 데스크톱 풀빌드는 CI/릴리스 시점(비범위).
- **누락 PR 흡수**: §수행계획서 2-2 의 대표 PR(표 셀 그림 복사 #1228, 문단 id 전역 유니크 #1222, wrap=Square 커서 #1220) 소스 반영 확인.
- **무회귀**: HanPage·hanpage.paldyn.com·H-마크·CNAME·라이선스 고지 잔존 / `rhwp`·`edwardkim` 엔진 식별자 보존 / `HanPage-Desktop/` 전체 / mydocs HanPage 문서.

## 5. 리스크 / 롤백

- **devel force-push 전 백업**: `git tag backup/devel-pre-task23 origin/devel` (또는 `refs/backup/...`) 생성 → 문제 시 즉시 환원.
- **충돌 오해결**: 44파일은 엔진 외이므로 로직 회귀 위험 낮음. 바이너리(로고/favicon)는 "paldyn 채택".
- **triage 오판**: §1-1 전부 DROP은 upstream 대체분 직접 확인 기반. Stage 4 엔진 diff=0 가 역검증(엔진이 정확히 upstream과 일치).
- **main 비범위**: main은 본 작업에서 미변경. devel 검증·반영 후 별도 릴리스 PR로 처리.
