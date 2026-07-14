# Task #13 Stage 2 — CI 워크플로 갱신 (완료 보고서)

- 이슈: [paldyn/HanPage#13](https://github.com/paldyn/HanPage/issues/13) · 브랜치: `local/task13`
- 단계: **Stage 2 (CI 워크플로)** — 디렉터리/패키지 리네임(Stage 1)에 맞춰 릴리스·Pages 워크플로 정합화.

---

## 1. 수정 내용

### 1-1. `.github/workflows/desktop-release.yml` (6개소)

| 위치 | 변경 |
|------|------|
| 주석(트리거 설명) | `\`desktop-v*\`` → `\`hanpage-desktop-v*\`` |
| `on.push.tags` | `- 'desktop-v*'` → `- 'hanpage-desktop-v*'` |
| `rust-cache` workspaces | `rhwp-desktop/src-tauri` → `HanPage-Desktop/src-tauri` |
| `Install desktop deps` working-directory | `rhwp-desktop` → `HanPage-Desktop` |
| tauri-action `projectPath` | `rhwp-desktop` → `HanPage-Desktop` |
| 업로드 path(dispatch) | `rhwp-desktop/src-tauri/target/...` → `HanPage-Desktop/...` |

**릴리스 표시명**: tauri-action 직전에 `Compute release name`(id `relname`) bash 스텝 신설:

```yaml
- name: Compute release name
  id: relname
  if: startsWith(github.ref, 'refs/tags/')
  shell: bash
  run: |
    ver="${GITHUB_REF_NAME#hanpage-desktop-v}"
    echo "name=HanPage Desktop-v${ver}" >> "$GITHUB_OUTPUT"
```

→ `releaseName: ${{ startsWith(github.ref, 'refs/tags/') && steps.relname.outputs.name || '' }}` (기존 `format('HanPage {0}', github.ref_name)` 대체).

- **근거**: git 태그는 공백 불가 → `hanpage-desktop-v{ver}`. GH Actions `${{ }}` 식엔 문자열 치환 함수가 없어 `format` 만으로는 "HanPage hanpage-desktop-v…"가 되어 부적합. bash 파라미터 확장으로 접두어 제거.
- `tagName` 은 `github.ref_name`(= `hanpage-desktop-v{ver}`) **유지** → 릴리스의 git 태그는 그대로, **표시명만** "HanPage Desktop-v{ver}".
- matrix 양 러너(macOS·Windows)가 동일 문자열 산출 → tauri-action 릴리스 생성/첨부 멱등. `shell: bash`라 Windows Git-bash에서도 `${VAR#prefix}`·`$GITHUB_OUTPUT` 동작.
- dispatch(비태그) 시 스텝 skip → `steps.relname.outputs.name` 공백 → releaseName `''`(= tagName 과 동일 분기). 빌드만 수행.

### 1-2. `.github/workflows/deploy-pages.yml` (1개소)
- `paths-ignore`: `- 'rhwp-desktop/**'` → `- 'HanPage-Desktop/**'`.
- **목적**: 데스크톱 전용 변경이 `main` push 시 **Pages 웹 배포를 트리거하지 않도록** 보증 유지(작업지시자 "깃헙페이지 무영향" 요건).

## 2. 검증 결과

| 항목 | 방법 | 결과 |
|------|------|------|
| 옛 참조 잔존 | `git grep rhwp-desktop -- .github/` | **0건** ✓ |
| 태그 표기 일관 | `git grep -E desktop-v -- .github/` | 전부 `hanpage-desktop-v*` ✓ |
| YAML 구문 | `ruby -ryaml YAML.load_file` (양 파일) | **OK / OK** ✓ |
| 스텝 구조·들여쓰기 | YAML 파싱 후 steps 순서·필드 추출 | `Compute release name`(if=태그) → `Build & bundle` 순서, `projectPath=HanPage-Desktop`, `releaseName=steps.relname.outputs.name` ✓ |
| releaseName 로직 | bash 파라미터 확장 재현 | `hanpage-desktop-v0.7.13` → "HanPage Desktop-v0.7.13", `…v1.0.0` → "…v1.0.0" ✓ |

## 3. 범위 밖 / 차기

- **실제 번들(.dmg/.exe) 산출·릴리스 생성**은 작업지시자가 `hanpage-desktop-v*` 태그를 push해야 검증 가능(CI 한정). 본 단계는 **워크플로 정의 정합성**까지.
- ⚠️ **태그 스킴 변경 주지**: 차기 데스크톱 릴리스는 `desktop-v*`가 아니라 **`hanpage-desktop-v*`** 로 태그해야 트리거됨. (최종 보고서·오늘할일에 명시 예정.)

## 4. 게이트

- [x] `desktop-release.yml` 트리거 태그·경로(cache/working-dir/projectPath/upload) → `HanPage-Desktop`/`hanpage-desktop-v*`
- [x] 릴리스 표시명 bash 스텝 → "HanPage Desktop-v{ver}" (tagName 은 태그 그대로)
- [x] `deploy-pages.yml` paths-ignore `HanPage-Desktop/**` (Pages 무영향 보증)
- [x] YAML 구문 검증(ruby) + 스텝 구조·로직 trace
- [ ] **(승인 대기)** Stage 3 — 최종 보고서 + main PR + (승인) 이슈 #13 클로즈
