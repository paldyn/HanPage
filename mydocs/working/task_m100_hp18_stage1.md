# Task #18 Stage 1 — mainBinaryName 추가 + 버전 범프 (완료 보고서)

- 이슈: [paldyn/HanPage#18](https://github.com/paldyn/HanPage/issues/18) · 브랜치: `local/task18`
- 단계: **Stage 1** — 본체 실행 파일명을 `HanPage.exe`로 바꾸는 설정 수정 + 릴리스 버전 범프.

---

## 1. 수정 내용

| 파일 | 변경 |
|------|------|
| `HanPage-Desktop/src-tauri/tauri.conf.json` | 최상위 `"mainBinaryName": "HanPage"` **신규** + `"version"` `0.7.13`→`0.7.14` |
| `HanPage-Desktop/src-tauri/Cargo.toml` | `version` `0.7.13`→`0.7.14` (`[package] name`·`[lib] name` 무변경) |
| `HanPage-Desktop/package.json` | `"version"` `0.7.13`→`0.7.14` |
| `HanPage-Desktop/package-lock.json` | `npm install` 로 root·`packages[""]` version → `0.7.14` (0 vulnerabilities, dep churn 없음) |

> `src-tauri/Cargo.lock` 은 **미추적**(`git ls-files` 0건) → 커밋 대상 아님. CI 빌드가 `Cargo.toml`(0.7.14)에서 재생성.

### 핵심 수정 (mainBinaryName)

```jsonc
"productName": "HanPage",
"mainBinaryName": "HanPage",   // ← 신규
"version": "0.7.14",
```

## 2. 원인·수정 근거 (Tauri v2 공식 스키마)

`@tauri-apps/cli/config.schema.json` 의 `mainBinaryName` 정의(설치된 CLI 버전 기준):

> Overrides app's main binary filename. **By default, Tauri uses the output binary from `cargo`**, by setting this, we will rename that binary in `tauri-cli`'s `tauri build` command... Note: this config should not include the binary extension (e.g. `.exe`), we'll add that for you.

- **원인 확정**: 기본값 = cargo 산출 바이너리 = `[package] name` = `rhwp-desktop` → `rhwp-desktop.exe`.
- **수정 확정**: `mainBinaryName` 설정 시 `tauri build`/`bundle` 단계에서 해당 바이너리를 리네임 → `HanPage.exe`(확장자는 Tauri 가 자동 부가).
- **값 정합**: 확장자 없이 `"HanPage"` 로 기입(스키마 요구 준수).

### 대안 검토 (채택 안 함)

스키마는 "가능하면 package name 또는 `[[bin]] name` 변경" 을 우선 권고하나, 본 프로젝트는 **#13 결정으로 크레이트 식별자(`rhwp-desktop`/`rhwp_desktop_lib`)를 유지**한다. `mainBinaryName` 은 cargo 산출물명(크레이트 내부)은 그대로 두고 **번들 산출물명만** 바꾸는 가장 외과적 수단이라, 크레이트 manifest 에 사용자 노출명(`HanPage`)을 섞지 않는다. → `mainBinaryName` 채택.

## 3. 검증 결과

| 항목 | 방법 | 결과 |
|------|------|------|
| diff 최소성 | `git diff` (소스 3파일) | mainBinaryName 1줄 + version 3필드만 ✓ |
| JSON 유효성 | `node require(tauri.conf.json)` | 파싱 OK, `mainBinaryName="HanPage"` ✓ |
| `mainBinaryName` 키 유효 | Tauri v2 `config.schema.json` 존재 확인 | **공식 스키마 properties 에 존재** ✓ |
| 크레이트명 무변경 | 작업트리 vs `HEAD` `^name =` 비교 | `rhwp-desktop`(L2)·`rhwp_desktop_lib`(L16) **IDENTICAL** ✓ |
| 노출 메타 무회귀 | node 로 키 추출 | productName=HanPage·identifier=com.paldyn.hanpage·fileAssoc 2건 **불변** ✓ |
| 버전 일괄 정합 | 4파일(+lock 2필드) 확인 | 전부 `0.7.14` ✓ |
| lock churn | `npm install` 출력 | 0 vulnerabilities, dep 변동 없음(version 필드만) ✓ |

## 4. 기능 검증(번들 본체명) — CI 위임 사유

- **Windows `.exe` 본체명은 macOS 로컬에서 산출 불가**(NSIS 번들러는 Windows 호스트 필요) → 최종 권위 검증은 **CI 릴리스 산출물(Stage 3)**.
- macOS 로컬 `tauri build` 로 `.app` 내부 바이너리명을 proxy 검증할 수 있으나, (a) 동작이 **공식 스키마로 문서화**되어 있고 (b) 실제 관심 대상(Windows)은 CI 한정이며 (c) 풀 빌드(WASM+studio+cargo, CI 에서 wasm-opt 우회까지 필요)는 proxy 대비 비용 과다 → **계획서 §5 허용대로 CI 검증으로 갈음**.
- Stage 3 에서 확인: `HanPage_0.7.14_x64-setup.exe` 설치 본체 = `HanPage.exe`(+ 시작메뉴/프로세스명), `.dmg` 정상, 릴리스 표시명 "HanPage Desktop-v0.7.14".

## 5. 게이트

- [x] `tauri.conf.json` `mainBinaryName="HanPage"` 추가 (스키마 유효 확인)
- [x] 버전 `0.7.13`→`0.7.14` 4파일(+lock) 일괄 정합
- [x] 크레이트 식별자(`rhwp-desktop`/`rhwp_desktop_lib`) 무변경
- [x] 노출 메타(productName/identifier/fileAssoc) 무회귀 + diff 최소
- [ ] **(승인 대기)** Stage 2 — 최종 보고서 + main PR
- [ ] (Stage 3) 릴리스 태그 `hanpage-desktop-v0.7.14` → CI 산출 `.exe` 본체명 `HanPage.exe` 권위 확인
