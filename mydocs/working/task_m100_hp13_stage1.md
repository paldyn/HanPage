# Task #13 Stage 1 — 디렉터리 리네임 + 패키지명/README + 구조 무결성 (완료 보고서)

- 이슈: [paldyn/HanPage#13](https://github.com/paldyn/HanPage/issues/13) · 브랜치: `local/task13`
- 단계: **Stage 1 (디렉터리·패키지·문서 리네임)** — CI 워크플로(Stage 2) 전, 디렉터리/패키지명 정합화.

---

## 1. 수정 내용

### 1-1. 디렉터리 리네임 (`git mv`, 이력 보존)
- `git mv rhwp-desktop HanPage-Desktop` — tracked **28파일** 일괄 이동, 전부 **rename(R) 인식**(`{rhwp-desktop => HanPage-Desktop}/...`, 0 insertions/0 deletions). 중첩 `.gitignore`(`HanPage-Desktop/.gitignore`·`src-tauri/.gitignore`, 상대패턴)는 동반 이동되어 **무수정 유효**. 미tracked `node_modules`/빌드산출물도 디렉터리 이동에 동반(물리 rename).

### 1-2. npm 패키지명 → `hanpage-desktop`
- `package.json` `name`(1곳) + `package-lock.json` `name`(root + `packages[""]`, 2곳) → `hanpage-desktop`. 세 필드 일치.

### 1-3. README 디렉터리 참조 (4곳)
- 제목 `# HanPage-Desktop — …`, 명명규약 문단(디렉터리 `HanPage-Desktop`/패키지 `hanpage-desktop`, **크레이트 내부명 `rhwp-desktop`/`rhwp_desktop_lib`는 유지** 명시), `paths-ignore`(`HanPage-Desktop/**`), `cd HanPage-Desktop`.

### 1-4. 유지(무변경) — 크레이트 내부 식별자
- `src-tauri/Cargo.toml`(`name="rhwp-desktop"`·`[lib] name="rhwp_desktop_lib"`), `src-tauri/src/main.rs`(`rhwp_desktop_lib::run()`) — **blob 해시 동일**로 무변경 확인:
  - `Cargo.toml` `95c533c9…` (staged 신규경로 == HEAD 구경로)
  - `main.rs` `f956691b…` (동일)

## 2. 검증 결과

| 항목 | 방법 | 결과 |
|------|------|------|
| 리네임 이력 보존 | `git status` / `git diff --cached -M` | 28파일 **R(rename)** 인식, 내용 변경 0 ✓ |
| 크레이트 내부명 무변경 | staged blob vs HEAD blob 해시 | Cargo.toml·main.rs **IDENTICAL** ✓ |
| JSON 유효성 + 이름 | `node -p require(...).name` | package.json / lock(root·pkg) 모두 `hanpage-desktop` ✓ |
| 패키지 정합성 | `cd HanPage-Desktop && npm install` | **up to date, 0 vulnerabilities** (lock 무churn) ✓ |
| 상대경로 해소 | `../pkg`·`../rhwp-studio`(`/public`,`/dist`) 실재 | 전부 OK → 스크립트·`tauri.conf.json` 무수정 해소 ✓ |
| 잔존 참조 점검 | `git grep rhwp-desktop\|rhwp_desktop -- HanPage-Desktop/` | **의도된 크레이트 내부명·README 유지 멘션만** 잔존 ✓ |

> 상대경로 무영향 근거: `package.json` 스크립트(`copy-wasm`의 `../pkg/`, `build:frontend`/`dev:frontend`의 `--prefix ../rhwp-studio`)와 `tauri.conf.json`(`frontendDist: ../../rhwp-studio/dist`)은 자기 위치 기준 상대경로 → 디렉터리명만 바뀌어도 동일 타깃으로 해소. 수정 불필요.

## 3. 범위 밖 / 차기

- **실제 번들 산출(.dmg/.exe) 검증 불가(로컬)**: Tauri Windows 번들은 Windows 호스트, 전체 빌드는 시간 의존 → 본 단계는 **디렉터리·패키지·문서 정합성**까지. 산출물은 Stage 2(CI 워크플로 갱신) 후 작업지시자 `hanpage-desktop-v*` 태그 push 시 CI에서 확인.
- CI 워크플로(`desktop-release.yml` 경로·태그·릴리스명, `deploy-pages.yml` paths-ignore)는 **Stage 2**에서.

## 4. 게이트

- [x] `git mv` 디렉터리 리네임 + 28파일 rename 인식(이력 보존)
- [x] npm 패키지명 `hanpage-desktop` 일치(package.json ↔ lock) + JSON 유효 + `npm install` 정합
- [x] README 디렉터리 참조 갱신(크레이트 내부명 유지 명시)
- [x] 크레이트 내부 식별자 무변경(blob 해시 동일) + 상대경로 해소 확인
- [ ] **(승인 대기)** Stage 2 — CI 워크플로 갱신(release 태그·경로·표시명 + pages paths-ignore)
