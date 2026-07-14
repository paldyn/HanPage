# Task #13 — 리네임 rhwp-desktop → HanPage-Desktop (수행+구현 계획서)

- 이슈: [paldyn/HanPage#13](https://github.com/paldyn/HanPage/issues/13) · 마일스톤 M100(v1.0.0)
- 브랜치: `local/task13` (origin/main 분기) · 머지 타깃: **`main` PR** (#1/#5/#7/#12 선례)
- 성격: **리네임/설정 정리** (소스 로직 무변경). 데스크톱 앱 한정 — **GitHub Pages 무영향 유지**.

---

## 1. 배경 / 목표

데스크톱 앱의 사용자 노출 명칭은 이미 **HanPage**(`tauri.conf.json` productName, 창 title, identifier `com.paldyn.hanpage`)이나, **디렉터리/패키지/릴리스 명명은 옛 엔진 패밀리명 `rhwp-desktop`** 이 남아 있다. 작업지시자 지시:

- 디렉터리·패키지명을 `rhwp-desktop` → **`HanPage-Desktop`** 로 통일.
- 릴리스 표시명을 **`HanPage Desktop-v{버전}`** 형식으로.

> **유지(작업지시자 결정)**: Rust 크레이트 내부 식별자(`rhwp-desktop` 패키지 / `rhwp_desktop_lib` 라이브러리)는 **비노출**이라 그대로 둔다. 변경 시 `main.rs`/빌드 영향만 늘고 사용자 가치는 0.

## 2. 범위

| 구분 | 대상 |
|------|------|
| **변경** | ① 디렉터리 `rhwp-desktop/`→`HanPage-Desktop/` ② npm 패키지명 ③ `desktop-release.yml`(트리거 태그·경로·릴리스명) ④ `deploy-pages.yml` paths-ignore ⑤ `README.md` 디렉터리 참조 |
| **유지** | Rust 크레이트명(`rhwp-desktop`/`rhwp_desktop_lib`), `tauri.conf.json`(상대경로·productName 이미 HanPage), `src-tauri/` 소스, 상대경로 npm 스크립트(`../pkg`·`../rhwp-studio`) |
| **제외** | rhwp 엔진(`rhwp` 크레이트·`@rhwp/*`·vscode/chrome/firefox/safari), `mydocs/` 과거 이력 문서(task_m100_1*/hp5/hp7 등), 실제 `.dmg`/`.exe` 산출(차기 태그 push 시 CI에서만 검증 가능) |

## 3. 변경 인벤토리 (grep 전수 확인 완료)

### 3-1. 디렉터리 리네임
- `git mv rhwp-desktop HanPage-Desktop` — tracked 28파일(README·package*.json·`.gitignore`·`src-tauri/` 24) 일괄. 중첩 `.gitignore`(상대패턴 `node_modules`/`dist`/`/target`)는 디렉터리 이동에 동반되어 **무수정으로 유효**. (macOS 대소문자 무관 FS라도 `rhwp-desktop`↔`HanPage-Desktop`은 철자 자체가 달라 case-only 충돌 없음.)

### 3-2. 참조 갱신 (크레이트 내부명 제외)

| 파일 | 위치 | 현재 | 변경 |
|------|------|------|------|
| `deploy-pages.yml` | L25 | `- 'rhwp-desktop/**'` | `- 'HanPage-Desktop/**'` ⚠️Pages 무영향 보증 |
| `desktop-release.yml` | L8(주석) | `\`desktop-v*\` 태그` | `\`hanpage-desktop-v*\` 태그` |
| `desktop-release.yml` | L19(트리거) | `- 'desktop-v*'` | `- 'hanpage-desktop-v*'` |
| `desktop-release.yml` | L55(캐시) | `rhwp-desktop/src-tauri` | `HanPage-Desktop/src-tauri` |
| `desktop-release.yml` | L81(working-dir) | `rhwp-desktop` | `HanPage-Desktop` |
| `desktop-release.yml` | L90(projectPath) | `rhwp-desktop` | `HanPage-Desktop` |
| `desktop-release.yml` | L93(releaseName) | `format('HanPage {0}', ref_name)` | **bash 스텝 산출**(§4-B) |
| `desktop-release.yml` | L105-106(업로드) | `rhwp-desktop/src-tauri/...` | `HanPage-Desktop/src-tauri/...` |
| `package.json` | L2 | `"name": "rhwp-desktop"` | `"name": "hanpage-desktop"` |
| `package-lock.json` | L2,L8 | `"name": "rhwp-desktop"` | `"name": "hanpage-desktop"` |
| `README.md` | L1,L5,L14,L29 | `rhwp-desktop`(디렉터리/제목/paths-ignore/`cd`) | `HanPage-Desktop` |

### 3-3. 유지(무변경) — 크레이트 내부 식별자
- `src-tauri/Cargo.toml` L2 `name = "rhwp-desktop"`, L16 `[lib] name = "rhwp_desktop_lib"`, L10 주석 — **그대로**.
- `src-tauri/src/main.rs` L5 `rhwp_desktop_lib::run()` — **그대로**.

> 상대경로 무영향 확인: `package.json` 스크립트(`../pkg/`·`../rhwp-studio/`)와 `tauri.conf.json`(`../../rhwp-studio/dist`)은 디렉터리 자기 위치 기준 상대경로라 리네임 후에도 동일하게 해소된다 → **수정 불필요**.

## 4. 결정 포인트 (승인 요청)

- **A. npm 패키지명 = `hanpage-desktop`** (소문자; npm 명명규약상 대문자 불가). `private:true`라 미배포·내부 식별자일 뿐이며 기존 `rhwp-desktop` 패턴을 그대로 소문자 치환. → **추천**. (대안: `hanpage-desktop-app` 등은 불필요한 변형이라 미채택.)
- **B. 릴리스 표시명 메커니즘.** 새 태그는 `hanpage-desktop-v{ver}`(git 태그는 공백 불가). GH Actions `${{ }}` 표현식에는 **문자열 치환 함수가 없어** `format('HanPage {0}', ref_name)`로는 "HanPage hanpage-desktop-v0.7.13"가 되어 부적합. → tauri-action 직전에 **bash 스텝**으로 접두어를 제거해 표시명을 만든다(아래). 이는 단순 리네임을 넘는 **워크플로 로직 추가**라 명시적으로 승인을 구함.

  ```yaml
  # 릴리스 표시명: 태그 hanpage-desktop-v{ver} → "HanPage Desktop-v{ver}"
  - name: Compute release name
    id: relname
    if: startsWith(github.ref, 'refs/tags/')
    shell: bash
    run: |
      ver="${GITHUB_REF_NAME#hanpage-desktop-v}"
      echo "name=HanPage Desktop-v${ver}" >> "$GITHUB_OUTPUT"
  ```
  그리고 `releaseName: ${{ startsWith(github.ref, 'refs/tags/') && steps.relname.outputs.name || '' }}`.
  - 매트릭스(macOS·Windows) 양 러너가 동일 문자열을 산출 → tauri-action 릴리스 생성/첨부 멱등. `shell: bash`라 Windows Git-bash에서도 파라미터 확장(`${VAR#prefix}`)·`$GITHUB_OUTPUT` 동작.
  - `tagName`은 `github.ref_name`(=`hanpage-desktop-v{ver}`) 유지 → 릴리스의 git 태그는 그대로, **표시명만** "HanPage Desktop-v{ver}".

## 5. 구현 단계

### Stage 1 — 디렉터리 리네임 + 패키지명/README + 구조 무결성 확인
1. `git mv rhwp-desktop HanPage-Desktop`.
2. `package.json`·`package-lock.json` name → `hanpage-desktop`.
3. `README.md` 디렉터리 참조 4곳(제목 L1·명명규약 L5·paths-ignore L14·`cd` L29) → `HanPage-Desktop`. (L5는 "디렉터리=`HanPage-Desktop`/패키지=`hanpage-desktop`, 단 크레이트 내부명은 `rhwp-desktop` 유지"로 명확화.)
4. 검증: `git status`로 **rename(R) 인식** 확인 → `cd HanPage-Desktop && npm install` 로 스크립트·상대경로 해소 확인(또는 최소 `npm pkg get name`). `tauri.conf.json` 상대경로 trace. **전체 `tauri build`(.dmg/.exe)는 플랫폼·시간 의존이라 CI에 위임** — 본 단계는 구조 정합성까지.
5. `working/task_m100_hp13_stage1.md` 작성 → 승인 요청.

### Stage 2 — CI 워크플로 갱신
1. `desktop-release.yml`: 트리거 태그(L19)·주석(L8)·캐시(L55)·working-directory(L81)·projectPath(L90)·업로드 경로(L105-106) → `HanPage-Desktop`/`hanpage-desktop-v*`. releaseName(L93)을 §4-B bash 스텝 + `steps.relname.outputs.name` 으로 교체.
2. `deploy-pages.yml` L25 paths-ignore → `HanPage-Desktop/**`.
3. 검증: YAML 구문(가능 시 `actionlint`), releaseName 논리 trace(태그 `hanpage-desktop-v0.7.13` → "HanPage Desktop-v0.7.13"), paths-ignore가 데스크톱 전용 변경의 Pages 배포를 차단함을 확인(깃헙페이지 무영향).
4. `working/task_m100_hp13_stage2.md` 작성 → 승인 요청.

### Stage 3 — 최종 보고서 + main PR
1. `report/task_m100_hp13_report.md` + 오늘할일 갱신.
2. 푸시용 브랜치명 `task13-rename`(비-`local/`, `task12-scrollbar` 선례)로 push → `main` PR 생성.
3. (작업지시자 승인) PR 머지 + 이슈 #13 클로즈.

## 6. 수용 기준

- [ ] 디렉터리 `HanPage-Desktop/`, tracked 파일 이력 보존(R 인식).
- [ ] npm 패키지명 `hanpage-desktop`(package.json ↔ package-lock.json 일치).
- [ ] 상대경로 스크립트·`tauri.conf.json` 무수정으로 정상 해소(npm install OK).
- [ ] `desktop-release.yml`: 트리거 `hanpage-desktop-v*`, 경로 `HanPage-Desktop`, 표시명 "HanPage Desktop-v{ver}".
- [ ] `deploy-pages.yml` paths-ignore `HanPage-Desktop/**` → 데스크톱 변경 Pages 미트리거.
- [ ] 크레이트 식별자(`rhwp-desktop`/`rhwp_desktop_lib`) 무변경.
- [ ] 엔진/스튜디오/기타 워크플로 무영향.

## 7. 리스크 / 주의

- **실제 번들 산출(.dmg/.exe) 검증 불가(로컬)**: Tauri Windows 번들은 Windows 호스트, 전체 빌드는 시간 의존 → 본 타스크는 **리네임·설정 정합성**까지. 산출물은 작업지시자가 `hanpage-desktop-v*` 태그 push 시 CI에서 확인(기존 carryover와 연결).
- **Pages 무영향 보증**: `deploy-pages.yml` paths-ignore 갱신을 누락하면 데스크톱 전용 변경이 웹 재배포를 유발 → Stage 2 필수 항목.
- **태그 스킴 변경 주지**: 차기 데스크톱 릴리스는 `desktop-v*`가 아니라 **`hanpage-desktop-v*`** 로 태그해야 트리거됨(오늘할일/보고서에 명시).

## 8. 비범위

- rhwp 엔진·확장(vscode/chrome/firefox/safari)·`@rhwp/*`·Edward Kim 저작권/링크.
- `mydocs/` 과거 이력 문서의 `rhwp-desktop` 표기(이력 보존 — 무수정).
- 코드 서명·공증(Task #4, 별건 일시정지).
