---
kind: guide
status: active
canonical: mydocs/manual/desktop_auto_update.md
last_verified: 2026-10-07
---

# HanPage Desktop 자동 업데이트·릴리스 운영 가이드

Tauri v2 updater 기반. 새 릴리스 출시 시 앱이 **백그라운드로 받아두고, 사용자가 업데이트를 선택하면 적용**한다.
릴리스 절차와 macOS·Windows 코드 서명도 이 문서가 정본이다.

## 1. 구성 요약

| 요소 | 위치 |
|------|------|
| 플러그인 | `tauri-plugin-updater`(Rust, desktop 전용) — `HanPage-Desktop/src-tauri/Cargo.toml`·`lib.rs` |
| 설정 | `tauri.conf.json`: `bundle.createUpdaterArtifacts: true` + `plugins.updater {endpoints, pubkey}` |
| 배포원 | GitHub Releases + `latest.json` 매니페스트 |
| 엔드포인트 | `https://github.com/paldyn/HanPage/releases/latest/download/latest.json` |
| 서명 | ed25519/minisign. 공개키=config, **개인키=GitHub 시크릿** |
| CI | `desktop-release.yml`의 `tauri-action`이 서명·`latest.json`·첨부 자동 |
| 코드 서명 | macOS Developer ID 서명·Apple 공증, Windows Azure Artifact Signing([§3](#3-코드-서명)) |

## 2. 서명 키 (중요)

업데이트는 **서명 필수**. 키가 없으면 사용자 앱이 업데이트를 거부한다.

- **GitHub 시크릿 2개** (저장소 Settings → Secrets → Actions):
  - `TAURI_SIGNING_PRIVATE_KEY` — 개인키 본문
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — 개인키 비밀번호
- **공개키**는 `tauri.conf.json`의 `plugins.updater.pubkey`에 박혀 배포된다.
- ⚠ **개인키·비번 분실 = 동일 pubkey로 서명 영구 불가** → 새 키로 교체 시 기존 사용자는 **자동 업데이트 불가, 수동 재설치 필요**. 키 생성 직후 **안전 백업**(비밀번호 관리자/암호화 볼트)을 반드시 보관.

### 키 재생성(필요 시)
```bash
npx @tauri-apps/cli@2 signer generate -w /tmp/hanpage-updater.key   # 비번 입력
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo paldyn/HanPage < /tmp/hanpage-updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo paldyn/HanPage   # 비번 입력
# /tmp/hanpage-updater.key.pub 내용을 tauri.conf.json pubkey 에 반영, 백업 후 /tmp 삭제
```

## 3. 코드 서명

updater 서명(§2)은 업데이트 파일의 위변조를 막는다. 코드 서명은 운영체제가 설치 파일과 앱의
게시자를 확인하는 별도 서명이다. 둘 다 `desktop-release.yml`의 같은 `tauri build` 안에서 처리한다.

### 3.1 macOS — Developer ID 서명·공증 (Task #4)

| 시크릿 | 용도 |
|---|---|
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD` | Developer ID Application 인증서(p12, base64)와 비밀번호 |
| `APPLE_SIGNING_IDENTITY` | 서명 ID(`Developer ID Application: <이름> (<Team ID>)`) |
| `APPLE_API_KEY_P8`, `APPLE_API_KEY_ID`, `APPLE_API_ISSUER` | 공증용 App Store Connect API 키 |

- CI의 `Verify macOS signature and notarization` 단계가 updater 아카이브의 `.app`과 DMG 안의
  `.app`에 대해 `codesign --verify --deep --strict`, Team ID, `Developer ID Application`,
  Gatekeeper `Notarized Developer ID`, `xcrun stapler validate`, `hdiutil verify`를 확인한다.
  태그 빌드에서 Apple 서명 설정이 없으면 실패한다.
- Apple Developer Program 약관이 갱신되면 계정 소유자가 developer.apple.com에서 다시 동의할 때까지
  공증이 HTTP 403(`agreement missing or expired`)으로 실패한다. 동의 후 같은 태그의 실패 job만 다시 실행한다.

### 3.2 Windows — Azure Artifact Signing (Authenticode)

Microsoft의 관리형 서명 서비스(옛 이름 Trusted Signing)를 쓴다. 2026-07-23부터 한국 **법인**의
Public Trust 인증서가 지원된다(개인 개발자는 미국·캐나다만 가능). 키는 Microsoft HSM에 있고 CI에서
비대화식으로 서명한다. Basic 요금제는 월 $9.99, 월 5,000회 서명이다(빌드 1회 약 8회 서명).

**최초 1회 설정** (Azure 포털)

1. 유료 Azure 구독에서 `Microsoft.CodeSigning` 리소스 공급자를 등록한다.
2. Artifact Signing 계정을 **Korea Central** 지역에 만든다(엔드포인트 `https://krc.codesigning.azure.net`).
   엔드포인트는 계정 지역과 같아야 한다.
3. 조직(Organization) 신원 검증을 신청한다. 영문 법인명·주소가 공적 등록 정보와 정확히 같아야 하고,
   회사 도메인 이메일과 대표자의 정부 발급 신분증 확인이 필요하다. 처리에 1~20영업일이 걸리며
   제출 기회는 3번이다. 검증된 법인명이 인증서 Subject가 되고, Windows가 게시자로 표시한다.
4. **Public Trust** 인증서 프로필을 만든다. 프로필을 지우고 다시 만들면 게시자 식별값이 바뀌어
   SmartScreen 평판이 처음부터 다시 쌓이므로 같은 프로필을 계속 쓴다.
5. Microsoft Entra 앱 등록을 만들고 클라이언트 비밀을 발급한 뒤, 계정 또는 프로필에
   `Artifact Signing Certificate Profile Signer` 역할을 부여한다.

**GitHub 저장소 설정** (Settings → Secrets and variables → Actions)

| 종류 | 이름 | 값 |
|---|---|---|
| Secret | `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_CLIENT_SECRET` | 위 앱 등록의 자격 증명 |
| Variable | `ARTIFACT_SIGNING_ENDPOINT` | `https://krc.codesigning.azure.net` |
| Variable | `ARTIFACT_SIGNING_ACCOUNT` | Artifact Signing 계정 이름 |
| Variable | `ARTIFACT_SIGNING_PROFILE` | 인증서 프로필 이름 |

**CI 동작**

- `Prepare Windows signing` 단계가 signtool과 Microsoft Artifact Signing 클라이언트(dlib)를 준비하고
  `bundle.windows.signCommand`를 `--config`로 주입한다. 클라이언트는 버전과 패키지 SHA-512를 함께
  고정하고 dlib의 Microsoft 서명도 확인한다. 버전을 올릴 때는 NuGet 카탈로그의 `packageHash`로 해시도 바꾼다.
  저장소의 `tauri.conf.json`에는 서명 설정을 넣지 않으므로 로컬 빌드는 미서명이다.
- 위 6개가 **모두 없으면** 미서명으로 빌드하고 경고(`Windows 미서명`)를 남긴다. **일부만** 있으면 실패한다.
- 릴리스 빌드(`hanpage-desktop-v*` 태그 push)는 서명한다. `workflow_dispatch` 빌드는 태그에서 실행해도
  릴리스를 만들지 않으며, `windows_sign` 입력을 켠 경우에만 서명한다.
- **서명 개시 절차**: 설정 6개를 등록 → `windows_sign`을 켠 dispatch 빌드로 서명·검증 단계 성공 확인 →
  첫 서명 릴리스 확인 후 workflow의 `WINDOWS_SIGNING_REQUIRED`를 `'true'`로 바꾸는 PR을 병합한다.
  그 뒤에는 설정이 사라지거나 설치 파일이 미서명이면 릴리스 빌드가 실패한다. `'false'`인 동안에는
  미서명 릴리스 빌드도 job이 성공하므로 §4의 확인 항목을 반드시 본다.
- 서명은 `tauri build` 안에서 앱 exe → NSIS 플러그인 DLL → uninstaller → `setup.exe` 순으로 하고,
  그 다음 updater `.sig`를 만든다. **빌드가 끝난 뒤 따로 서명하면 `latest.json`의 updater 서명이
  깨지므로 금지한다.**
- `Verify Windows installer signature` 단계가 `setup.exe`, 설치 파일 안의 `HanPage.exe`·NSIS 플러그인
  DLL, 조용히 설치한 뒤의 `HanPage.exe`·`uninstall.exe`에 대해 서명 `Valid`와 타임스탬프를 확인한다.
  Artifact Signing 인증서는 72시간짜리라 타임스탬프가 없으면 곧 무효가 된다.
  `LangDLL.dll`은 Tauri가 서명하지 않는 알려진 예외다.
- NSIS 플러그인 DLL 서명은 `@tauri-apps/cli` 2.11.3 이상이 필요하다(2.11.2 이하는 미서명 DLL을 넣는다).

**사용자에게 보이는 효과와 한계**

- "알 수 없는 게시자" 대신 검증된 법인명이 표시된다. `bundle.publisher`(`PALDYN`)는 제거 항목의
  게시자 표기일 뿐 인증서 게시자를 바꾸지 않는다.
- SmartScreen 평판은 다운로드가 쌓이며 생긴다. 서명 초기에는 "Windows의 PC 보호" 안내가 계속 뜰 수 있다.
  2024년부터 EV 인증서도 즉시 평판을 주지 않는다.
- 클라이언트 비밀 만료와 신원 검증 갱신(만료 60일 전부터 안내)을 일정에 넣는다.

## 4. 릴리스 절차

1. 앱 버전을 `HanPage-Desktop/package.json`, `package-lock.json`, `src-tauri/Cargo.toml`,
   `src-tauri/Cargo.lock`의 앱 항목, `src-tauri/tauri.conf.json`에서 함께 올리고 PR로 병합한다.
2. 병합 commit에 태그를 만들어 push한다.
   ```bash
   git tag -a hanpage-desktop-v<버전> <병합 SHA> -m "HanPage Desktop <버전>"
   git push origin hanpage-desktop-v<버전>
   ```
3. CI(`tauri-action`)가 macOS(app·dmg)·Windows(nsis) 번들, updater 아티팩트, `latest.json`을
   **초안 릴리스**에 첨부한다.
4. 공개 전에 확인한다.
   - 두 플랫폼 job과 서명 검증 단계(§3.1·§3.2)가 모두 성공했는지
   - Artifact Signing을 설정한 뒤라면 Windows job 요약에 `Windows Authenticode 서명 확인 (타임스탬프 포함)`이
     있고 Subject가 검증된 법인명인지, `Windows 미서명` 경고나 `Windows 설치 파일: 미서명` 요약이 없는지
   - 자산 6개(DMG, `.app.tar.gz`·`.sig`, `setup.exe`·`.sig`, `latest.json`)
   - `latest.json`의 플랫폼 4개와 같은 태그의 URL, 각 updater 서명이 실제 자산으로 검증되는지
5. 초안을 공개한다. 다음 실행부터 기존 사용자 앱이 `latest.json`을 확인해 새 버전을 감지한다.

## 5. latest.json 형식 (tauri-action 자동 생성, 참고)

0.8.9 실제 매니페스트의 형태다(서명 생략). Windows 업데이트 파일은 내려받는 설치 파일과 같으므로
Authenticode 서명이 updater 서명보다 먼저 끝나야 한다.

```json
{
  "version": "0.8.9",
  "notes": "릴리스 노트",
  "pub_date": "2026-10-02T06:58:58.954Z",
  "platforms": {
    "darwin-aarch64":      { "signature": "<minisign 서명>", "url": ".../hanpage-desktop-v0.8.9/HanPage_aarch64.app.tar.gz" },
    "darwin-aarch64-app":  { "signature": "<minisign 서명>", "url": ".../hanpage-desktop-v0.8.9/HanPage_aarch64.app.tar.gz" },
    "windows-x86_64":      { "signature": "<minisign 서명>", "url": ".../hanpage-desktop-v0.8.9/HanPage_0.8.9_x64-setup.exe" },
    "windows-x86_64-nsis": { "signature": "<minisign 서명>", "url": ".../hanpage-desktop-v0.8.9/HanPage_0.8.9_x64-setup.exe" }
  }
}
```

## 6. 사용자 경험

- 시작할 때 조용히 새 버전을 확인하고 백그라운드로 다운로드한다. 최신이면 알림을 띄우지 않는다.
- 준비되면 상태 표시줄에 작은 **업데이트 준비됨** 버튼을 표시한다. 실행 중에는 상세 카드를 자동으로 열거나 편집 포커스를 옮기지 않는다. 버튼이나 메뉴에서 카드를 연 뒤 **업데이트**를 누르면 기존 저장 확인 절차를 거쳐 적용하고 다시 시작한다. Windows에서는 설치 프로그램이 열린다.
- **나중에** 또는 닫기를 선택해도 받아둔 파일은 앱이 실행 중인 동안 유지된다. 상태 표시줄의 **업데이트 확인 / 업데이트 준비됨** 버튼으로 언제든 다시 열 수 있다. 모든 Desktop 플랫폼의 `파일 > 업데이트 확인`과 macOS의 `HanPage > 업데이트 확인` 메뉴도 같은 카드를 열므로 상태 표시줄을 숨겨도 다시 접근할 수 있다.
- 앱을 종료했다가 다시 열면 버전을 다시 확인한다. 다운로드한 파일은 메모리에 보관하므로 필요한 경우 다시 받는다.
- 확인 중·다운로드·파일 검증·적용을 구분해서 표시한다. 다운로드는 실제 누적 용량과 전체 용량으로 진행률을 표시한다. 전체 크기를 모르면 받은 용량과 움직이는 진행 막대만 표시하며 임의의 퍼센트를 만들지 않는다.
- 적용 중에는 진행 막대와 다시 시작 안내를 표시하고 중복 클릭을 막는다. 파일 교체는 화면 스레드 밖에서 실행한다. 적용 전 저장 여부를 확인하며, 적용 도중에는 편집·단축키·네이티브 메뉴를 잠그고 실패하면 복구한다.
- 적용에 실패하면 받아둔 파일을 유지하며 **다시 업데이트**로 재시도한다. 네트워크 확인·다운로드 실패는 **다시 확인**으로 시작한다.
- 웹 빌드는 이 카드와 진입 버튼을 생성하지 않는다.

## 7. 주의 사항

- **부트스트랩**: updater가 없던 기존 버전(예: v0.7.13)은 자동 감지 불가. **첫 updater 포함 버전은 사용자가 수동 1회 설치**해야 이후 자동화된다. 릴리스 노트에 안내 권장.
- **macOS 배포**: 코드 서명·Apple 공증·stapler와 Gatekeeper 검증이 완료된 산출물을 게시한다. 0.8.7 실제 배포 검증은 [PR 114 기록](../pr/archives/pr_114_review.md)을 참고한다.
- **Windows 배포**: Artifact Signing 설정을 마친 뒤에는 Authenticode 서명·타임스탬프 검증이 완료된 설치 파일만 게시한다.
- **게시 게이트**: 필수 번들·업데이트 서명·매니페스트·코드 서명·공증 검증이 실패하면 공개 게시를 완료하지 않는다. UI 로컬 검증은 실제 업데이트 설치와 공개 릴리스 검증을 대신하지 않는다.

## 8. 트러블슈팅

| 증상 | 원인·조치 |
|------|----------|
| 업데이트 안 뜸 | 기존 버전에 updater 없음(부트스트랩) / `latest.json` 미첨부 / 버전 비교 동일 |
| "signature 검증 실패" | config `pubkey` ↔ 서명 개인키 불일치(키 교체 후 pubkey 미반영) |
| CI 빌드가 "A public key has been found, but no private key"로 실패 | updater 시크릿 2개 미설정. config에 공개키가 있으면 개인키 없이 빌드할 수 없다 |
| macOS 공증 HTTP 403 | Apple Developer Program 약관 미동의(§3.1). 계정 소유자 동의 후 실패 job 재실행 |
| Windows 빌드가 `failed to run ...signtool.exe`로 실패 | Artifact Signing 자격 증명·역할·엔드포인트 지역·클라이언트 비밀 만료 확인. 원인 출력은 tauri `--verbose`에서만 보인다 |
| Windows 서명 검증 단계 실패 | 메시지의 파일과 상태 확인. 타임스탬프 누락이면 타임스탬프 서버 응답 문제이므로 재실행 |
| Windows 사용자 업데이트가 "signature 검증 실패" | 빌드 뒤 설치 파일을 다시 서명해 `.sig`와 내용이 달라짐. 서명은 `tauri build` 안에서만 한다 |
| 서명했는데 SmartScreen 안내가 뜸 | 평판 축적 전의 정상 동작(§3.2). 인증서 프로필을 다시 만들지 않는다 |
| 매니페스트 404 | 엔드포인트 URL ↔ 릴리스에 `latest.json` 첨부 여부 확인 |
