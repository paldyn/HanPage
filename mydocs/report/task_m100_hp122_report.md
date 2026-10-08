# Task #122 (M100) — Windows 설치 파일 코드 서명 검토와 릴리스 검증 보강 최종 보고서

Issue: #122 · 계획: [task_m100_hp122.md](../plans/task_m100_hp122.md) · base `5685104a56a6502b57a8f210f5ea14e8d45b6e88`

## 1. 결과 요약

Windows 설치 파일의 정식 서명 방법을 조사·구현했으나, PALDYN 이 사업자등록 없는 1인 개발이어서
게시자를 실명으로만 받을 수 있고 비용(연 약 $309) 대비 효과가 작아 **미서명 배포 유지**로 결정했다
(작업지시자 선택, 2026-10-08). 대신 릴리스 파이프라인의 부족한 부분을 보강하고 결정과 재개 조건을 문서로 남겼다.
Azure Artifact Signing 연동 구현은 커밋 [`39710e814`](https://github.com/paldyn/HanPage/commit/39710e814bbb938df679517d6b6ae3ede51e32c3)에 보존했다.

## 2. 최종 변경

| 파일 | 내용 |
|---|---|
| `.github/workflows/desktop-release.yml` | macOS 서명·공증 검증 단계(updater `.app`·DMG 안 `.app`: strict codesign·Team ID·Developer ID·Gatekeeper 공증·stapler, 태그 빌드는 Apple 설정 필수), Windows 서명 상태 보고 단계, 단일 릴리스 조건(`push` + `hanpage-desktop-v*` — dispatch 는 태그에서 실행해도 릴리스를 만들지 않음), `tauri-action` v0.6.2 SHA 고정, 낡은 주석 정정 |
| `HanPage-Desktop/package.json`, `package-lock.json` | `@tauri-apps/cli` 2.11.2 → 2.11.5 (향후 서명 시 NSIS 플러그인 DLL 서명에 필요한 수정 포함) |
| `mydocs/manual/desktop_auto_update.md` | 코드 서명 절(macOS 운영·Windows 미서명 결정 근거·설치 안내·향후 서명 규칙), 현재 릴리스 절차, 실제 `latest.json` 형태, 트러블슈팅 |
| `HanPage-Desktop/README.md` | Windows 설치 안내 절, 사전 요구, 로드맵(서명 보류), 릴리스 확인 항목 |
| `mydocs/manual/README.md`, `mydocs/README.md` | 가이드 라우팅·manifest 등록 |

## 3. 서명 방법 조사 결과 (2026-10-07, 반박 검증 포함)

| 방법 | 한국 개인 | 게시자 | 연 비용 | CI(빌드 안 서명) |
|---|---|---|---|---|
| Azure Artifact Signing | 불가(개인은 미국·캐나다만, 한국은 조직만 2026-07-23~) | — | 약 $120 | 가능 |
| SSL.com IV + eSigner | 가능(여권·주민증·운전면허) | 실명 | 약 $309 | Jsign 으로 가능(IV 의 CI 사용은 체험으로 확인 필요) |
| Certum 오픈소스 | 가능 | `Open Source Developer, 실명` | 약 €49 | 비공식 도구뿐 |
| Microsoft Store(MSIX) | 가능 | 실명 | 0 | 스토어 설치본만, 별도 패키징 |
| SignPath Foundation | 포크 규정 등으로 부적합 가능성 높음 | SignPath Foundation | 0 | 빌드 후 서명만 → updater 서명 충돌 |
| 개인사업자 등록 후 | 등록 필요 | 등록 상호 | 약 €209~ | 업체별 상이 |

## 4. 검증

| 항목 | 결과 |
|---|---|
| workflow YAML 파싱 | 통과 |
| CLI 2.11.5 macOS `--bundles app` 빌드 | 성공 |
| updater 개인키 없는 빌드 | `A public key has been found, but no private key` 실패 재현 — 주석 정정 근거 |
| `tauri-action` v0 = v0.6.2 = `84b9d35b5fc46c1e45415bdb6144030364f7ebc5` | GitHub API 확인 |
| 문서 링크 검사 / 메타데이터 검사 | 오류 0 / base 와 동일 |

## 5. 미검증·남은 일

- macOS 검증 단계와 Windows 보고 단계는 다음 태그 빌드(또는 dispatch 빌드)에서 처음 실행된다.
- CLI 2.11.5 의 Windows NSIS 빌드는 아직 실행하지 않았다(PR CI 는 desktop-release 를 돌리지 않음).
- 사업자등록 등 조건이 바뀌면 #122 를 다시 열고 위 표와 `39710e814` 구현을 출발점으로 삼는다.
