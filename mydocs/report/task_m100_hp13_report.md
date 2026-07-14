# Task #13 — 리네임 rhwp-desktop → HanPage-Desktop (최종 결과 보고서)

- 이슈: [paldyn/HanPage#13](https://github.com/paldyn/HanPage/issues/13) · 마일스톤 M100(v1.0.0)
- 브랜치: `local/task13` (origin/main 분기) · 머지 타깃: **`main` PR** (#1/#5/#7/#12 선례)
- PR: [paldyn/HanPage#15](https://github.com/paldyn/HanPage/pull/15) (base `main` ← `task13-rename`)
- 상태: **리네임·CI 정합 완료 · 검증 통과 · main PR #15 생성** (머지/이슈 클로즈 승인 대기)

## 1. 배경 / 목표

데스크톱 앱의 사용자 노출 명칭은 이미 **HanPage**(productName·창 title·identifier `com.paldyn.hanpage`)이나, **디렉터리/패키지/릴리스 명명**에 옛 엔진 패밀리명 `rhwp-desktop`이 남아 있었다. 작업지시자 지시로 ① 디렉터리·패키지를 **`HanPage-Desktop`** 로 통일, ② 릴리스 표시명을 **"HanPage Desktop-v{버전}"** 형식으로 정리했다. **Rust 크레이트 내부 식별자(`rhwp-desktop`/`rhwp_desktop_lib`)는 비노출이라 유지**(변경 시 빌드 영향만 늘고 사용자 가치 0).

## 2. 변경 요약

### 2-1. 디렉터리·패키지·문서 (Stage 1, `fc628415`)

| 대상 | 변경 |
|------|------|
| 디렉터리 | `git mv rhwp-desktop HanPage-Desktop` — tracked **28파일** 전부 rename(R) 인식(이력 보존) |
| npm 패키지명 | `package.json` + `package-lock.json`(root·`packages[""]`) → `hanpage-desktop` |
| README | 제목·명명규약 문단·`paths-ignore`·`cd` 4곳 → `HanPage-Desktop` (크레이트 내부명 유지 사유 명시) |

### 2-2. CI 워크플로 (Stage 2, `8475a385`)

| 파일 | 변경 |
|------|------|
| `desktop-release.yml` | 트리거 태그 `desktop-v*`→`hanpage-desktop-v*`(주석 포함), 캐시·working-directory·`projectPath`·업로드 path → `HanPage-Desktop`; 릴리스 표시명 = 신규 `Compute release name` bash 스텝 산출 |
| `deploy-pages.yml` | `paths-ignore` `rhwp-desktop/**`→`HanPage-Desktop/**` (Pages 무영향 보증 유지) |

**릴리스 표시명 메커니즘**: git 태그는 공백 불가 → `hanpage-desktop-v{ver}`. GH Actions `${{ }}` 식엔 문자열 치환 함수가 없어, tauri-action 직전 bash 스텝이 `${GITHUB_REF_NAME#hanpage-desktop-v}`로 접두어를 떼어 `name=HanPage Desktop-v{ver}`를 출력 → `releaseName: steps.relname.outputs.name`. `tagName`은 git 태그(`hanpage-desktop-v{ver}`) 그대로 → **태그는 그대로, 표시명만** "HanPage Desktop-v{ver}". matrix 양 러너(macOS·Windows) 동일 산출 → 멱등.

## 3. 유지(무변경) 항목

- **Rust 크레이트 식별자**: `src-tauri/Cargo.toml`(`name="rhwp-desktop"`·`[lib] name="rhwp_desktop_lib"`), `src-tauri/src/main.rs`(`rhwp_desktop_lib::run()`) — blob 해시 동일 확인.
- **상대경로**: `package.json` 스크립트(`../pkg/`·`--prefix ../rhwp-studio`)·`tauri.conf.json`(`../../rhwp-studio/dist`) — 자기 위치 기준 상대경로라 디렉터리명만 바뀌어도 동일 해소 → **무수정**.
- **rhwp 엔진·확장**(`rhwp` 크레이트·`@rhwp/*`·vscode/chrome/firefox/safari)·`mydocs/` 과거 이력 문서 — 무관, 무수정.

## 4. 검증 결과

| 케이스 | 방법 | 결과 |
|--------|------|------|
| 리네임 이력 보존 | `git diff -M` / `git show --stat` | 28파일 rename 인식(README 86%·lock 99%·json 95%, 나머지 100%) ✓ |
| 크레이트 내부명 무변경 | staged blob vs HEAD blob 해시 | Cargo.toml·main.rs **IDENTICAL** ✓ |
| 패키지 정합성 | `cd HanPage-Desktop && npm install` | **up to date, 0 vulnerabilities**(lock 무churn) ✓ |
| 상대경로 해소 | `../pkg`·`../rhwp-studio`(`/public`,`/dist`) 실재 | 전부 OK ✓ |
| `.github` 옛 참조 | `git grep rhwp-desktop -- .github/` | **0건** ✓ |
| YAML 구문 | `ruby -ryaml YAML.load_file`(양 파일) | OK / OK ✓ |
| 릴리스 표시명 로직 | bash 파라미터 확장 재현 | `hanpage-desktop-v0.7.13`→"HanPage Desktop-v0.7.13" ✓ |

## 5. 영향 범위

- **데스크톱 앱 한정 리네임/설정 정리.** 소스 로직(엔진·스튜디오·셸) 무변경.
- **GitHub Pages 무영향 유지**: `deploy-pages.yml` paths-ignore가 `HanPage-Desktop/**` 로 갱신되어, 데스크톱 전용 변경은 `main` push 시에도 웹 배포를 트리거하지 않음.
- **데스크톱 반영**: 본 PR 머지 후, 작업지시자가 **`hanpage-desktop-v*`** 태그를 push하면 CI가 `.dmg`/`.exe` 산출 + "HanPage Desktop-v{ver}" 릴리스 생성.

## 6. 잔여 / 후속 (본 타스크 범위 밖)

- **실제 번들(.dmg/.exe) 산출·릴리스 생성**은 CI 한정 검증(태그 push 필요) → 본 타스크는 리네임·워크플로 정의 정합성까지.
- ⚠️ **태그 스킴 변경 주지**: 차기 데스크톱 릴리스는 `desktop-v*`가 아니라 **`hanpage-desktop-v*`** 로 태그해야 트리거됨.
- 이 리네임으로 #5/#7/#12 누적 수정 + 본 정리가 차기 `hanpage-desktop-v*` 릴리스에 함께 반영됨(기존 carryover와 연결).

## 7. 커밋

| 커밋 | 내용 |
|------|------|
| `99ace57d` | 수행+구현 계획서 + 오늘할일 |
| `fc628415` | Stage 1 — 디렉터리 리네임(28파일) + 패키지명 + README |
| `8475a385` | Stage 2 — CI 워크플로(릴리스 태그·경로·표시명 + Pages paths-ignore) |
| _(본 커밋)_ | Stage 3 — 최종 보고서 + 오늘할일 갱신 |
