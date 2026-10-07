# Task #122 (M100) — Windows 설치 파일 Authenticode 정식 서명 최종 보고서

Issue: #122 · 계획: [task_m100_hp122.md](../plans/task_m100_hp122.md) · base `5685104a56a6502b57a8f210f5ea14e8d45b6e88`

## 1. 결과 요약

Windows 설치 파일을 Azure Artifact Signing 으로 서명하는 CI 연동, macOS·Windows 서명 검증 단계, 운영 문서를
마련했다. Artifact Signing 계정과 GitHub 설정을 등록하면 다음 릴리스 빌드부터 서명된다. 그 전까지는
기존처럼 미서명 + 경고로 빌드된다. **실제 Windows 서명 실행은 계정이 없어 미검증이다.**

## 2. 변경 내역

| 파일 | 내용 |
|---|---|
| `.github/workflows/desktop-release.yml` | `Prepare Windows signing`(클라이언트 1.0.128·nupkg SHA-512 고정·dlib Microsoft 서명 확인·signtool·metadata.json·`--config` 조각), 설정 유무별 동작, `WINDOWS_SIGNING_REQUIRED` 스위치, 단일 "릴리스 빌드" 조건(`push` + `hanpage-desktop-v*`), `tauri-action` v0.6.2 SHA 고정, macOS·Windows 검증 단계, 낡은 주석 정정 |
| `HanPage-Desktop/package.json`, `package-lock.json` | `@tauri-apps/cli` 2.11.2 → 2.11.5 (NSIS 플러그인 DLL 서명 수정 포함) |
| `mydocs/manual/desktop_auto_update.md` | 코드 서명 절(§3), 현재 릴리스 절차(§4), 실제 `latest.json` 형태(§5), 트러블슈팅 6행 |
| `HanPage-Desktop/README.md` | 사전 요구(Windows·로컬 미서명), 로드맵, 릴리스 확인 항목·가이드 링크 |
| `mydocs/manual/README.md`, `mydocs/README.md` | 가이드 라우팅·manifest 등록 |

설정 유무별 동작:

| 상황 | Windows 결과 |
|---|---|
| 설정 6개 모두 없음 | 미서명 + `Windows 미서명` 경고 (`WINDOWS_SIGNING_REQUIRED='true'` 이고 릴리스 빌드면 실패) |
| 일부만 있음 | 실패 |
| 모두 있음 + 릴리스 빌드 또는 `windows_sign` dispatch | 서명 → 검증 단계가 Valid·타임스탬프 확인 |
| dispatch(태그에서 실행 포함) | 릴리스를 만들지 않음, `windows_sign` 일 때만 서명 |

## 3. 검증

| 항목 | 결과 |
|---|---|
| workflow YAML 파싱(ruby psych) | 통과 |
| CLI 2.11.5 + 샘플 `signCommand` 객체 조각으로 macOS `--bundles app` 빌드 | 번들 생성, 설정 스키마 수용 |
| updater 개인키 없이 빌드 | `A public key has been found, but no private key` 로 실패 — 기존 주석 정정 근거 |
| `tauri-action@v0` = v0.6.2 = `84b9d35b5fc46c1e45415bdb6144030364f7ebc5` | GitHub API 로 태그 역참조 확인 |
| Artifact Signing 클라이언트 1.0.128 SHA-512 | NuGet 카탈로그 `packageHash` 와 일치하는 값으로 고정 |
| Tauri NSIS 템플릿(2.11.5) | Uninstall 키 `...\Uninstall\${PRODUCTNAME}`·DisplayName, `/S` 에서 자동 실행 없음, 연결 명령 확인 |
| `check_markdown_links.py` | 오류 0 |
| `check_document_metadata.py` | 출력이 base 와 동일(신규 오류 0) |

독립 검토(Actions 의미·Windows 검증 로직·macOS 검증·보안/문서 4관점 + 반박 검증)에서 확인된 3건
(dispatch-on-tag 조건, 클라이언트 패키지 부분 검증, 서명 개시 후 미서명 통과)을 반영했다. 6건은 근거 부족으로 기각됐다.

## 4. 미검증·남은 일

- Windows 서명·검증 단계의 실제 실행(Artifact Signing 계정·신원 검증 필요). 첫 실행은 `windows_sign` dispatch 로 확인한다.
- macOS 검증 단계는 다음 릴리스 빌드에서 처음 실행된다. 0.8.9 수동 감사와 같은 검사를 옮긴 것이다.
- 작업지시자 조치: 법인 여부 확인 → Azure 구독·Artifact Signing 계정(Korea Central)·조직 신원 검증·Public Trust 프로필·
  Entra 앱 역할 → GitHub secrets 3개·variables 3개 → dispatch 검증 → 첫 서명 릴리스 후 `WINDOWS_SIGNING_REQUIRED='true'` PR.
- 보호된 GitHub environment 로 서명 비밀을 태그 빌드에만 노출하는 강화는 별도 과제로 남긴다.
