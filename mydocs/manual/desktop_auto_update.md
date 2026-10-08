---
kind: guide
status: active
canonical: mydocs/manual/desktop_auto_update.md
last_verified: 2026-10-08
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
| 코드 서명 | macOS Developer ID 서명·Apple 공증. Windows는 미서명 배포([§3](#3-코드-서명)) |

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
게시자를 확인하는 별도 서명이다. macOS는 둘 다 `desktop-release.yml`의 같은 `tauri build` 안에서 처리하고,
Windows는 updater 서명만 한다.

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

### 3.2 Windows — 미서명 배포 (#122 결정, 2026-10-08)

Windows 설치 파일(`HanPage_<버전>_x64-setup.exe`)은 Authenticode 서명 없이 배포한다. 자동 업데이트는
updater 서명(§2)으로 위변조를 검증하므로 이 결정과 무관하게 안전하게 동작한다.

**결정 근거** — PALDYN은 사업자등록이 없는 1인 개발이다. 2026-10-07 조사 기준으로 다음과 같다.

| 방법 | 개인 가능 여부 | 게시자 표시 | 연 비용 | 판단 |
|---|---|---|---|---|
| Azure Artifact Signing | 불가(개인은 미국·캐나다만, 한국은 조직만) | — | 약 $120 | 해당 없음 |
| SSL.com 개인(IV) + eSigner 클라우드 | 가능 | 본인 실명 | 약 $309 | 비용 대비 효과 작음 |
| Certum 오픈소스(SimplySign) | 가능 | `Open Source Developer, <실명>` | 약 €49 | CI 자동화가 비공식 도구뿐 |
| Microsoft Store(MSIX) | 가능 | 본인 실명 | 0 | 스토어 설치본만 해결, 별도 패키징 |
| 개인사업자 등록 후 사업자 명의 | 등록 필요 | 등록 상호(PALDYN) | 약 €209~ | 등록·세무 부담, 보류 |

등록되지 않은 브랜드명(PALDYN)은 어떤 경로로도 게시자 이름이 될 수 없다. 서명하더라도 SmartScreen
평판이 쌓이기 전에는 경고가 계속 뜰 수 있고(2024년부터 EV도 즉시 평판 없음), 그래서 실명 서명에 비용을
들이는 이점이 작다고 판단했다.

**사용자 안내** — 릴리스 노트와 [Desktop README](../../HanPage-Desktop/README.md#windows-설치-안내)에 둔다.

- 처음 실행하면 "Windows의 PC 보호" 화면이 뜰 수 있다. **추가 정보 → 실행**을 누르면 설치된다.
- 공식 배포처는 GitHub Releases(`paldyn/HanPage`)와 hanpage.paldyn.com 다운로드 버튼뿐이다.

**나중에 서명할 때의 규칙** (사업자등록 등으로 조건이 바뀌면 [#122](https://github.com/paldyn/HanPage/issues/122)를 다시 연다)

- 서명은 CI가 만든 `--config` 조각의 `bundle.windows.signCommand`로 `tauri build` **안에서** 한다.
  tauri-cli는 앱 exe·NSIS 플러그인 DLL·uninstaller·setup.exe를 서명한 뒤 updater `.sig`를 만든다.
  빌드가 끝난 뒤 따로 서명하면 `latest.json`의 updater 서명이 깨진다.
- `tauri.windows.conf.json`에 서명 설정을 넣지 않는다(로컬 Windows 빌드에도 자동 병합된다). 빈 문자열
  `signCommand`도 서명을 켜므로, 설정이 없을 때는 키 자체를 넣지 않는다.
- 비밀값은 `signCommand` 인자에 넣지 않는다(tauri가 명령줄을 로그·NSIS 스크립트에 남긴다). 환경 변수로 넘긴다.
- NSIS 플러그인 DLL 서명에는 `@tauri-apps/cli` 2.11.3 이상이 필요하다(현재 2.11.5).
- Azure Artifact Signing 연동 예시(서명 준비 단계·Authenticode 검증 단계)는 PR #124의 첫 커밋
  [`39710e814`](https://github.com/paldyn/HanPage/commit/39710e814bbb938df679517d6b6ae3ede51e32c3)에 남아 있다.

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
   - Windows job 요약의 `Windows 설치 파일: ... Authenticode NotSigned (미서명 배포, #122)` — 미서명 배포 중에는
     이 상태가 정상이다. 다른 상태가 나오면 의도하지 않은 서명 설정이 섞인 것이므로 원인을 확인한다.
   - 자산 6개(DMG, `.app.tar.gz`·`.sig`, `setup.exe`·`.sig`, `latest.json`)
   - `latest.json`의 플랫폼 4개와 같은 태그의 URL, 각 updater 서명이 실제 자산으로 검증되는지
5. 릴리스 노트에 Windows 설치 안내(§3.2)를 넣고 초안을 공개한다. 다음 실행부터 기존 사용자 앱이 `latest.json`을 확인해 새 버전을 감지한다.

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
- **Windows 배포**: 미서명 설치 파일로 배포한다(§3.2). 릴리스 노트에 "Windows의 PC 보호 → 추가 정보 → 실행" 안내를 넣는다.
- **게시 게이트**: 필수 번들·업데이트 서명·매니페스트·코드 서명·공증 검증이 실패하면 공개 게시를 완료하지 않는다. UI 로컬 검증은 실제 업데이트 설치와 공개 릴리스 검증을 대신하지 않는다.

## 8. 트러블슈팅

| 증상 | 원인·조치 |
|------|----------|
| 업데이트 안 뜸 | 기존 버전에 updater 없음(부트스트랩) / `latest.json` 미첨부 / 버전 비교 동일 |
| "signature 검증 실패" | config `pubkey` ↔ 서명 개인키 불일치(키 교체 후 pubkey 미반영) |
| CI 빌드가 "A public key has been found, but no private key"로 실패 | updater 시크릿 2개 미설정. config에 공개키가 있으면 개인키 없이 빌드할 수 없다 |
| macOS 공증 HTTP 403 | Apple Developer Program 약관 미동의(§3.1). 계정 소유자 동의 후 실패 job 재실행 |
| Windows 사용자 업데이트가 "signature 검증 실패" | 빌드 뒤 설치 파일을 다시 서명해 `.sig`와 내용이 달라짐. 서명은 `tauri build` 안에서만 한다 |
| Windows 설치 시 "Windows의 PC 보호" | 미서명 배포의 정상 동작(§3.2). 추가 정보 → 실행 |
| 매니페스트 404 | 엔드포인트 URL ↔ 릴리스에 `latest.json` 첨부 여부 확인 |
