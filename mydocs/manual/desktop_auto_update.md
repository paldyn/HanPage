---
kind: guide
status: active
canonical: mydocs/manual/desktop_auto_update.md
last_verified: 2026-11-04
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

- CI의 `Verify macOS signature and notarization` 단계가 번들 `.app`, 실제로 올리는 updater 아카이브
  (`HanPage.app.tar.gz`)를 푼 `.app`, DMG 안의 `.app`에 대해 디렉터리 여부, `codesign --verify --deep --strict`, Team ID,
  `Developer ID Application`, Gatekeeper `Notarized Developer ID`, `xcrun stapler validate`를 확인하고 세 `.app`의 CDHash가
  같은지 본다. Team ID는 `APPLE_SIGNING_IDENTITY`에서 읽은 값과 따로, 워크플로에 고정한 배포 Team ID(`8L78W6D8XF`)와
  비교한다. 아카이브는 `HanPage.app` 밖 항목이 없어야 하고(updater는 첫 경로만 떼고 모든 항목을 푼다), DMG는 `hdiutil verify`,
  같은 Team의 Developer ID 서명, 볼륨 최상위가 `HanPage.app`과 `/Applications` 링크뿐인지(점으로 시작하는 숨김 항목 제외)
  확인한다. tauri-action이 먼저 초안에 올리므로 태그 빌드는 tauri-action이 알려 준 릴리스 ID로 그 릴리스를 읽어, 태그가 같고
  아직 초안인지, updater 아카이브·DMG의 digest가 검증한 파일과 같은지 대조한다(API 일시 오류에 대비해 세 번까지 시도,
  #170). 실패하면 원인별 `::error`를 남기고(§8), 태그 빌드에서 Apple 서명 설정이 없으면 실패한다. macOS job은
  `macos-15` 러너에서 돈다(macOS 14 이미지는 2026-11-02에 은퇴했다).
- 지금 쓰는 Developer ID Application 인증서는 2027-02-01에 만료된다(갱신하면 이 날짜를 고친다). 검증 단계와 감사 스크립트(§4)가
  만료 90일 전부터 경고한다. 갱신 절차는 §3.1.1을 따른다.
- DMG 컨테이너는 Developer ID로 서명만 하고 공증·스테이플하지 않는다. 공증·스테이플 대상은 안의 `.app`이다(0.8.x 전체 동일).
- Apple Developer Program 약관이 갱신되면 계정 소유자가 developer.apple.com에서 다시 동의할 때까지
  공증이 HTTP 403(`agreement missing or expired`)으로 실패한다. 동의 후 같은 태그의 실패 job만 다시 실행한다.

#### 3.1.1 Developer ID 인증서 갱신

근거는 Apple의 [교체 안내](https://developer.apple.com/help/account/certificates/replace-developer-id-certificates/)와
[2026-10-01 공지](https://developer.apple.com/news/?id=w4atic4c)다.

- 구 `Developer ID Certification Authority`는 2027-02-01에 끝나고, 그 아래에서 나온 인증서는 발급일과 관계없이 그날부터 서명에 쓸 수 없다.
  2026-06-14에 발급한 인증서도 여기에 묶인다.
- 이미 서명·공증한 릴리스는 보안 타임스탬프로 계속 동작한다. 그 뒤에 만드는 릴리스만 새 인증서가 필요하다.
- Apple은 G2에서 나온 인증서가 1년 유효하고 해마다 갱신해야 한다고 안내한다. 실제 만료일은 받은 인증서에서 확인한다.
- 계정 소유자(Account Holder)만 만들 수 있다. 같은 종류를 5개까지 둘 수 있어 만료 전에 미리 만들어 시험할 수 있다.

1. 키체인 접근 → 인증서 지원 → **인증 기관에서 인증서 요청**으로 CSR을 디스크에 저장한다.
2. developer.apple.com → Certificates → `+` → **Developer ID Application**을 고른다. 중간 기관을 물으면 **G2 Sub-CA**를 고른다. 다른 항목은
   2027년에 끝나는 인증서를 줄 수 있다. CSR을 올리고 `.cer`을 받는다.
3. `.cer`을 더블클릭해 키체인에 넣는다. 내 인증서에 같은 이름이 둘 보이면 발급자 이름의 조직 단위가 `G2`인 항목을 고른다(구 인증서는
   `Apple Certification Authority`). 둘 다 `G2`면 만료일이 늦은 쪽이다. 개인 키와 함께 `.p12`로 내보낸다(비밀번호 지정).
4. 시크릿을 바꾼다. 이름과 Team ID가 그대로면 `APPLE_SIGNING_IDENTITY`는 바꾸지 않는다. Team ID가 바뀌면 워크플로의
   `EXPECTED_TEAM_ID`와 감사 스크립트의 `TEAM_ID`도 함께 바꾼다.
   ```bash
   base64 -i <파일>.p12 | gh secret set APPLE_CERTIFICATE --repo paldyn/HanPage
   gh secret set APPLE_CERTIFICATE_PASSWORD --repo paldyn/HanPage
   ```
5. `desktop-release`를 수동 실행해(§4의 2단계) 서명·공증·검증이 통과하고 만료 경고가 사라지는지 확인한다. 이어서 §4.1 리허설로 새 인증서의
   자산이 감사를 지나는지 본다.
6. `.p12` 파일을 지우고, §3.1의 만료일을 받은 인증서의 날짜로 고친다.

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
   `src-tauri/Cargo.lock`의 앱 항목, `src-tauri/tauri.conf.json`에서 함께 올리고 변경 이력을 적어 PR을 연다.
2. 태그는 옮길 수 없으므로 PR CI와 함께 PR 브랜치에서 `desktop-release`를 수동 실행해 두 플랫폼 번들과 서명 검증 단계가
   성공하는지 확인한다. 단계가 성공으로 끝났다는 것만으로는 부족하다. 서명 설정이 없으면 수동 실행은 검증을 건너뛰고
   성공으로 끝나므로, 실제 검증 출력이 있는지 본다. job 요약(`### macOS 서명·공증 확인`)은 GitHub에 로그인한 브라우저에서만
   보이므로, CLI로는 macOS job의 annotation 제목에 `macOS 미서명`이 없고(`gh run view`는 제목 없이 메시지만 보여 준다) macOS
   job 로그에 실행 결과 `The validate action worked!`가 세 번(번들·아카이브·DMG의 `.app`) 있는지 확인한다. 이 문장은 단계
   스크립트 원문에 없어 건너뛴 실행에서는 나오지 않는다.
   ```bash
   gh workflow run desktop-release.yml --repo paldyn/HanPage --ref <PR 브랜치>
   gh api repos/paldyn/HanPage/check-runs/<macOS job ID>/annotations --jq '.[].title'
   gh run view --job <macOS job ID> --log --repo paldyn/HanPage | grep -c 'The validate action worked!'
   ```
3. PR을 병합하고, 병합 commit의 tree가 시험한 source와 같은지 또는 차이가 앱에 들어가지 않는 문서뿐인지 확인해 배포 기록에
   남긴다. 빌드 입력이 다르면(PR 뒤 devel에 다른 변경이 들어온 경우 등) 병합 직후 `devel`에서 다시 수동 실행하고
   (`--ref devel`), 그 실행의 head SHA가 병합 SHA인지 확인한 뒤 태그를 만든다.
4. 병합 commit에 태그를 만들어 push한다.
   ```bash
   git tag -a hanpage-desktop-v<버전> <병합 SHA> -m "HanPage Desktop <버전>"
   git push origin hanpage-desktop-v<버전>
   ```
5. CI(`tauri-action`)가 macOS(app·dmg)·Windows(nsis) 번들, updater 아티팩트, `latest.json`을
   **초안 릴리스**에 첨부한다. tauri-action은 `tauri build`가 실패하면 한 번 더 시도한다(`retryAttempts: 1`, macOS 러너의
   `bundle_dmg.sh` 일시 실패 대비, §8). 그래도 한 플랫폼 job이 실패하면 태그를 옮기거나 시크릿을 바꾸지 말고 같은 run의
   실패 job만 다시 실행한다(`gh run rerun <run ID> --failed --repo paldyn/HanPage`). 초안에 이미 올라간 다른 플랫폼 자산은
   그대로이고, `latest.json`은 다시 실행한 job이 합친다. 재시도가 성공하면 job은 성공으로 끝나고 annotation도 남지 않으므로,
   돌았는지는 로그로 본다(`gh run view <run ID> --log --repo paldyn/HanPage | grep 'Attempt 1 failed'`). 첫 실패의 출력은 그 줄 바로 위에
   있다. 태그마다 보이면 §8의 DMG 행을 본다.
6. 공개 전에 확인한다.
   - 두 플랫폼 job과 서명 검증 단계(§3.1·§3.2)가 모두 성공했는지
   - Windows job 요약의 `Windows 설치 파일: ... Authenticode NotSigned (미서명 배포, #122)` — 미서명 배포 중에는
     이 상태가 정상이다. 다른 상태가 나오면 의도하지 않은 서명 설정이 섞인 것이므로 원인을 확인한다. 이 요약도 로그인한
     브라우저에서만 보이며, CLI로는 아래 감사의 Authenticode 검사가 같은 사실을 확인한다.
   - 초안 자산을 빈 폴더에 받아 감사 스크립트를 macOS에서 실행하고, 결과 JSON을 배포 기록의 증적으로 남긴다. 스크립트는
     자산을 실행하지 않는다. `--release`는 초안의 자산 목록·digest가 감사한 파일과 같은지 대조한다. 같은 스크립트를 이전
     공개 릴리스의 자산에도 한 번 돌려 통과하는지 보면(대조군) 스크립트와 실행 환경의 이상을 먼저 걸러낼 수 있다.
     ```bash
     git fetch --tags origin
     gh release view hanpage-desktop-v<버전> --repo paldyn/HanPage --json databaseId --jq .databaseId   # 릴리스 ID
     gh release download hanpage-desktop-v<버전> --repo paldyn/HanPage --dir <빈 폴더>
     node HanPage-Desktop/scripts/release-audit.mjs <빈 폴더> <버전> --release <릴리스 ID> > draft-audit.json
     ```
     경고(`warning:`)는 터미널과 결과 JSON의 `warnings`에 남는다. 실패로 치지 않지만 내용을 보고 조치한다(인증서 만료는 §3.1.1).
     두 job이 같은 순간에 초안을 만들면 초안이 둘일 수 있으므로(§8) 같은 태그의 릴리스가 하나인지도 본다.
     ```bash
     gh api --paginate 'repos/paldyn/HanPage/releases?per_page=100' --jq '.[] | select(.tag_name=="hanpage-desktop-v<버전>") | .id'
     ```
     자산 6개, `latest.json`(최상위 키·RFC 3339 `pub_date`·버전·빈 `notes`·플랫폼 4개·같은 태그 URL·`.sig`와 바이트 단위로
     같은 서명), 이전 릴리스와 같은 updater 공개키(기존 설치본이 업데이트를 받아들이는 조건), 두 updater 서명의 엄격한
     형식·검증·변조 거부, updater 앱·DMG 앱의 디렉터리 여부·버전·Developer ID·공증·stapler·같은 CDHash, DMG 컨테이너의
     Developer ID 서명과 볼륨 구성, updater 아카이브 항목, setup.exe 버전 리소스와 Authenticode 없음(§3.2), 릴리스 자산의
     digest와 대상 commit을 확인한다.
7. 릴리스 노트에 Windows 설치 안내(§3.2)를 넣는다
   (`gh release edit hanpage-desktop-v<버전> --repo paldyn/HanPage --notes-file <파일>`). REST API로 초안을 직접 고칠 때는
   `tag_name`을 함께 보낸다. 빠뜨리면 초안의 태그가 `untagged-…`로 풀리고, 그대로 공개하면 잘못된 태그가 생겨
   `latest.json`의 URL이 모두 404가 된다. 공개는 이 결과를 보고 메인터너가 정한다. 공개 직전에 초안의 `tag_name`·대상
   commit(병합 SHA)을 다시 보고, 같은 폴더로 감사를 `--release`와 함께 다시 실행해 감사 뒤 자산이 바뀌지 않았는지 확인한다.
8. 초안을 공개한다. 공개 직후 `releases/latest/download/latest.json`이 1분 정도 이전 태그로 redirect될 수 있으니, 쿼리 없는
   주소가 새 태그로 바뀌고 내용이 감사한 `latest.json`과 같은지 확인한 뒤 끝낸다(`curl -sL <주소> | shasum -a 256`과
   `draft-audit.json`의 `assets["latest.json"].sha256` 비교). 다음 실행부터 기존 사용자 앱이 `latest.json`을 확인해 새 버전을
   감지한다. 공개한 뒤에는 태그 workflow를 다시 실행하지 않는다. tauri-action이 공개 자산을 지우고 다시 올리기 때문이다.
   다시 실행해도 두 job 모두 tauri-action 전에 `공개 릴리스` 오류로 멈추고(초안은 `releases/tags`로 조회되지 않아 이 확인을
   지난다), macOS 검증 단계도 릴리스가 초안이 아니면 `초안 자산` 오류를 낸다.

### 4.1 태그 경로 리허설

공개 릴리스 확인, 초안 여부·digest 대조처럼 태그 빌드에서만 도는 단계는 수동 실행으로 시험할 수 없고, 태그는 옮기지 않으므로 여기서
결함이 나면 그 버전을 버려야 한다. 이 단계, tauri-action, 서명 인증서를 바꿨다면 실제 태그 전에 리허설 태그로 확인한다. 초안만 만들어지고
공개 릴리스와 `latest` 주소는 바뀌지 않는다. 이 초안은 공개하지 않는다. 태그와 초안을 지워도 commit과 run 기록은 공개 저장소에 남으므로
비밀을 넣지 않는다.

태그의 `<버전>`은 그 commit의 `tauri.conf.json` 버전이다. 병합할 PR head에 바로 붙이면 병합할 워크플로가 그대로 돈다.

```bash
git tag -a hanpage-desktop-v<버전>-rehearsal.<N> <commit> -m "rehearsal"
git push origin hanpage-desktop-v<버전>-rehearsal.<N>
```

run이 끝나면 macOS job만 다시 실행한다. 초안이 이미 있을 때의 공개 릴리스 확인과 `latest.json` 합치기를 지나고, 성공한 job을 다시 돌리므로 같은
이름의 자산 교체까지 지난다. 실패 job을 다시 실행해 복구할 때의 경로를 포함한다. 재실행 뒤에는 `runs/<ID>/jobs`와 `gh run view --job`이 마지막
시도를 보여 주므로, 첫 실행의 단계 결론과 로그를 먼저 저장한다(나중에는 `gh run view <run ID> --attempt 1 --log`).

```bash
gh run rerun <run ID> --job <macOS job ID> --repo paldyn/HanPage
gh run watch <run ID> --exit-status --repo paldyn/HanPage
```

재실행이 끝나면 초안을 감사한다. 버전 인자는 숫자 그대로 두고 `--rehearsal`로 태그를 정한다.

```bash
git fetch --tags origin
gh release view hanpage-desktop-v<버전>-rehearsal.<N> --repo paldyn/HanPage --json databaseId --jq .databaseId   # 릴리스 ID
gh release download hanpage-desktop-v<버전>-rehearsal.<N> --repo paldyn/HanPage --dir <빈 폴더>
node HanPage-Desktop/scripts/release-audit.mjs <빈 폴더> <버전> --release <릴리스 ID> --rehearsal <N> > rehearsal-audit.json
```

재시도는 태그 없이 본다. 일회용 브랜치의 tauri-action 단계 바로 앞에 아래 단계를 넣고 그 브랜치를 수동 실행한다(§4의 2단계). 대역은 첫 호출에서
자신을 지우고 실패하므로 첫 `tauri build`가 `bundle_dmg.sh` 중간(볼륨 마운트 뒤)에서 멈춘다. 재시도 여유가 남아야 하므로 태그 리허설에는 넣지
않는다. tauri-action 단계 로그에 `Attempt 1 failed, retrying...`이 한 번 있고 job이 성공하면 재시도가 돈 것이다(`forced SetFile failure`는
대역을 만드는 단계의 원문으로도 찍혀 job 전체 grep으로는 판정하지 못한다). 이 시험은 재시도 배선과 이미지가 붙은 채 남은 상태에 대한 내성만 입증한다. 대역이 없는 두 번째
시도가 실제 DMG 오류로 실패하면 재시도 결함이 아니므로, 그 출력을 남기고 다시 실행한다.

```yaml
- name: Rehearsal only - fail the first DMG attempt
  if: runner.os == 'macOS'
  shell: bash
  run: |
    mkdir -p "$RUNNER_TEMP/shim"
    printf '#!/bin/bash\nrm -f "$0"\necho "rehearsal: forced SetFile failure" >&2\nexit 1\n' > "$RUNNER_TEMP/shim/SetFile"
    chmod +x "$RUNNER_TEMP/shim/SetFile"
    echo "$RUNNER_TEMP/shim" >> "$GITHUB_PATH"
```

리허설은 워크플로와 감사만 확인한다. 공개 릴리스를 막는 분기와 설치된 앱이 받는 경로는 지나지 않는다. 결과(run URL, commit, 감사 JSON)를 기록에
남긴 뒤 초안과 태그를 지운다. 초안이 없으면 둘째 줄 대신 `git push origin --delete <태그>`를 쓴다.

```bash
git tag -d hanpage-desktop-v<버전>-rehearsal.<N>
gh release delete hanpage-desktop-v<버전>-rehearsal.<N> --repo paldyn/HanPage --cleanup-tag
```

## 5. latest.json 형식 (tauri-action 자동 생성, 참고)

0.8.9 실제 매니페스트의 형태다(서명 생략). Windows 업데이트 파일은 내려받는 설치 파일과 같으므로
Authenticode 서명이 updater 서명보다 먼저 끝나야 한다. 릴리스 노트는 GitHub 릴리스 본문에 쓰고 `notes`는 비워 둔다
(§4의 감사 스크립트가 확인한다).

```json
{
  "version": "0.8.9",
  "notes": "",
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
- 준비되면 상태 표시줄에 작은 **업데이트 준비됨** 버튼을 표시하고, 화면 오른쪽 아래(상태 표시줄 위)에 **새 버전 X 준비됨** 작은 알림을 버전마다 한 번 띄운다. 알림은 포커스를 가져가지 않고 8초 뒤 닫히며(마우스를 올려 두거나 키보드로 들어가 있으면 유지, Esc로 닫기), 모달이 열려 있거나 앱 창이 뒤에 있으면(다른 앱 사용·최소화) 그동안 미루고 창을 떠나면 닫히는 시간도 멈춘다. 메뉴·팝업·모달보다 아래에 놓여 사용자가 연 화면을 가리지 않는다. **업데이트**는 상세 카드를 열어 같은 적용 흐름을 시작하고 **나중에**는 알림만 닫는다. 실행 중에는 상세 카드를 자동으로 열거나 편집 포커스를 옮기지 않는다. 버튼이나 메뉴에서 카드를 연 뒤 **업데이트**를 누르면 기존 저장 확인 절차를 거쳐 적용하고 다시 시작한다. Windows에서는 설치 프로그램이 열린다.
- 창이 여러 개일 때(#156): 작은 알림은 실행마다 앱 전체에서 버전당 한 번만 뜬다(처음 띄우는 창을 네이티브가 정한다). **업데이트**를 누르면 그 창의 저장 확인·AI 도우미 정리보다 먼저 다른 창을 확인해, 저장하지 않은 문서가 있으면 적용하지 않고 그 창을 앞으로 가져온다(적용 직전에 네이티브가 한 번 더 확인한다). 적용에 실패하면 내렸던 AI 도우미를 다시 올린다. 적용이 시작되면 모든 창에 적용 중 카드와 입력 보호를 띄운다.
- **나중에** 또는 닫기를 선택해도 받아둔 파일은 앱이 실행 중인 동안 유지된다. 상태 표시줄의 **업데이트 확인 / 업데이트 준비됨** 버튼으로 언제든 다시 열 수 있다. 모든 Desktop 플랫폼의 `파일 > 업데이트 확인`과 macOS의 `HanPage > 업데이트 확인` 메뉴도 같은 카드를 열므로 상태 표시줄을 숨겨도 다시 접근할 수 있다.
- 앱을 종료했다가 다시 열면 버전을 다시 확인한다. 다운로드한 파일은 메모리에 보관하므로 필요한 경우 다시 받는다.
- 확인 중·다운로드·파일 검증·적용을 구분해서 표시한다. 다운로드는 실제 누적 용량과 전체 용량으로 진행률을 표시한다. 전체 크기를 모르면 받은 용량과 움직이는 진행 막대만 표시하며 임의의 퍼센트를 만들지 않는다.
- 적용 중에는 진행 막대와 다시 시작 안내를 표시하고 중복 클릭을 막는다. 파일 교체는 화면 스레드 밖에서 실행한다. 적용 전 저장 여부를 확인하며, 적용 도중에는 편집·단축키·네이티브 메뉴를 잠그고 실패하면 복구한다.
- 적용에 실패하면 받아둔 파일을 유지하며 **다시 업데이트**로 재시도한다. 네트워크 확인·다운로드 실패는 **다시 확인**으로 시작한다.
- 웹 빌드는 이 카드와 진입 버튼을 생성하지 않는다.

## 7. 주의 사항

- **부트스트랩**: updater가 없던 기존 버전(예: v0.7.13)은 자동 감지 불가. **첫 updater 포함 버전은 사용자가 수동 1회 설치**해야 이후 자동화된다. 릴리스 노트에 안내 권장.
- **macOS 배포**: `.app`은 코드 서명·Apple 공증·stapler와 Gatekeeper 검증을 마친 것을, DMG는 그 `.app`을 담고 Developer ID로 서명한 것을 게시한다(DMG 자체는 공증하지 않는다, §3.1). 0.8.7 실제 배포 검증은 [PR 114 기록](../pr/archives/pr_114_review.md)을 참고한다.
- **Windows 배포**: 미서명 설치 파일로 배포한다(§3.2). 릴리스 노트에 "Windows의 PC 보호 → 추가 정보 → 실행" 안내를 넣는다.
- **tauri-action 1.x**: 0.6.2에 머문다(PR #137). 1.0.0은 `latest.json`의 다운로드 주소를 `github.com/.../releases/download/...`가 아니라
  `api.github.com/.../releases/assets/<id>`로 쓰고 이를 고르는 입력이 없다. 업데이터가 `Accept: application/octet-stream`을 보내 받을 수는
  있지만, 로그인 없는 API는 IP당 시간당 60회이고 다운로드 시도마다 1회를 쓴다. 같은 IP의 다른 API 사용과 한도를 나누고, 받은 파일은 메모리에만
  있어 적용을 미룬 PC는 실행할 때마다 다시 쓴다. 한도를 넘기면 자동 재시도 없이 업데이트 실패로 표시된다(앱 사용은 막히지 않는다). 지금 규모로
  HanPage만으로는 닿기 어렵지만, 공개된 앱의 업데이트 경로를 바꾸는 일이다. 1.0.0의 이득(공개 자산을 지우지 않음, `untagged-` 함정 해소,
  `latest.json` 처리 전체 재시도)은 가드와 절차로 막고 있거나 작다. 0.x로 고칠 수 없는 결함이 나오거나 upstream이 주소를 고르게 해 주면 다시
  판단한다. Dependabot의 1.x 업데이트는 `@dependabot ignore this major version`으로 막아 뒀다. 올릴 때는 아래를 함께 다룬다.
  - `.app.tar.gz` 이름에 버전이 붙어 검증 단계의 digest 대조와 감사 스크립트가 실패한다. 이 대조는 태그에서만 돌아 PR CI와 수동 실행에는 보이지
    않으므로 같은 PR에서 고치고 §4.1로 확인한다. 감사의 URL 검사도 주소 형식 때문에 바꿔야 한다.
  - 지금 주소를 유지하려면 `uploadUpdaterJson: false`로 생성을 끄고 별도 job으로 `latest.json`을 만든다.
  - 주소가 자산 id에 묶이므로 공개 뒤 자산을 교체하면 그 주소가 무효가 된다. 교체하면 `latest.json`도 다시 만든다.
  - §4.1은 설치본 경로를 지나지 않는다. 다운로드·서명 검증·설치는 공개 자산을 바라보는 시험 빌드로 따로 확인한다.
- **게시 게이트**: 필수 번들·업데이트 서명·매니페스트·코드 서명·공증 검증이 실패하면 공개 게시를 완료하지 않는다. UI 로컬 검증은 실제 업데이트 설치와 공개 릴리스 검증을 대신하지 않는다.

## 8. 트러블슈팅

| 증상 | 원인·조치 |
|------|----------|
| 업데이트 안 뜸 | 기존 버전에 updater 없음(부트스트랩) / `latest.json` 미첨부 / 버전 비교 동일 |
| "signature 검증 실패" | config `pubkey` ↔ 서명 개인키 불일치(키 교체 후 pubkey 미반영) |
| CI 빌드가 "A public key has been found, but no private key"로 실패 | updater 시크릿 2개 미설정. config에 공개키가 있으면 개인키 없이 빌드할 수 없다 |
| macOS 공증 HTTP 403 | Apple Developer Program 약관 미동의(§3.1). 계정 소유자 동의 후 실패 job 재실행 |
| macOS job이 공증 `Accepted` 뒤 `bundle_dmg.sh` 실패로 끝남 | 어느 단계인지 아직 모른다. 세 번 있었고(run 40627099903 6초, 0.8.12 태그 빌드 첫 시도 11초, #170 첫 수동 실행 6초) 모두 `--verbose` 전이라 스크립트 출력이 없다. 0.8.12는 같은 commit, #170은 앱 source가 같은 다음 실행, run 40627099903은 두 commit 뒤 실행에서 통과했고, 복구는 모두 새 러너에서였다. 실패 시각은 Finder AppleScript(CI에서도 돈다) 전일 수 있고 job 끝에 `diskimages-help` 고아 프로세스가 남아, `hdiutil`도 후보다. tauri-action이 `tauri build`를 한 번 더 시도하고(`retryAttempts: 1`), macOS 번들은 `--verbose`로 돌아 출력이 로그에 남는다. 두 번 다 실패하면 그 출력을 기록하고 같은 run의 실패 job만 다시 실행한다(§4). `TAURI_BUNDLER_DMG_IGNORE_CI: false`는 창 배치 단계를 건너뛰어 DMG를 열었을 때 아이콘 배치가 사라지고 commit이 바뀌므로, 출력으로 AppleScript 실패를 확인한 뒤 다음 버전에서만 쓴다 |
| tauri-action 단계가 릴리스 조회·자산 목록·`latest.json` 처리에서 GitHub API 오류로 실패 | `retryAttempts`는 `tauri build`와 자산 업로드만 다시 시도한다. 그 밖의 일시 오류는 job 실패로 끝나므로 실패 job을 다시 실행한다(§4). `latest.json`은 지운 뒤 다시 올리므로 그 업로드가 실패하면 파일이 사라진다. 감사에서 플랫폼이 빠지면 빠진 플랫폼의 job도 다시 실행한다(`gh run rerun <run ID> --job <job ID> --repo paldyn/HanPage`) |
| tauri-action 단계가 자산 업로드에서 `already_exists`로 실패 | 앞선 업로드가 남긴 자산일 수 있다. 재시도는 자산을 지우지 않고 다시 올린다. 실패 job을 다시 실행하면 목록을 새로 읽어 같은 이름을 지운다 |
| 같은 태그의 초안이 둘이다 | 두 job이 찾기와 만들기를 같은 순간에 했다(tauri-action에 잠금이 없다). 자산이 갈려 감사의 자산 6개 검사가 실패한다. 공개하지 말고 초안을 모두 지운 뒤 run을 다시 실행한다 |
| 감사의 `stapled notarization ticket`이 `Error 68`로 실패 | 일시 오류로 보인다(2026-11-04 같은 자산이 바로 다음 실행에서 통과). 다시 감사한다. 계속 실패하면 네트워크와 티켓을 확인한다 |
| `Refuse to overwrite a published release`의 `공개 릴리스`·`공개 릴리스 확인` 오류 | 그 태그의 릴리스가 이미 공개돼 있다(공개 뒤 태그 workflow 재실행). 번들 전에 멈췄으므로 공개 자산은 그대로다. 다시 실행하지 않는다 / 릴리스 조회를 세 번 시도해 모두 실패했다. 일시 오류면 실패 job만 다시 실행한다 |
| 검증 단계의 `macOS 미서명` 오류(태그 빌드)·경고(수동 실행) | `APPLE_SIGNING_IDENTITY`가 비어 있다. 태그 빌드는 실패하고, 수동 실행은 서명·공증 검증 없이 성공으로 끝나므로 §4의 2단계로 확인한다 |
| 검증 단계의 `macOS Team ID` 오류 | `APPLE_SIGNING_IDENTITY` 괄호 안 Team ID가 없거나 배포 Team ID와 다르다(§3.1 표의 형식). 인증서를 바꿨다면 Team ID도 함께 확인한다 |
| 검증 단계의 `macOS 서명`·`macOS 공증`·`DMG`·`DMG 서명`·`DMG 구성`·`updater 아카이브 구조` 오류 | 해당 `.app`·DMG·아카이브가 §3.1의 조건(배포 Team의 Developer ID 서명, `.app` 공증·stapler, `HanPage.app`만 담은 구성)을 어긴다. 원인을 확인하기 전에는 공개하지 않는다 |
| 검증 단계의 `macOS 빌드 불일치`·`초안 자산 불일치` 오류 | 번들·아카이브·DMG의 `.app`이 서로 다른 빌드이거나, 초안에 올라간 자산이 검증한 파일과 다르다. 공개하지 않고 원인을 확인한다 |
| 검증 단계의 `초안 자산`·`초안 자산 조회` 오류 | tauri-action이 알려 준 릴리스에서 자산을 하나로 찾지 못했거나(릴리스 ID 없음·태그 불일치 포함), 그 릴리스가 초안이 아니거나(위 `공개 릴리스` 확인을 지나 공개된 경우 — 공개 자산이 다시 올라갔을 수 있으니 감사부터 다시 한다), API 조회를 세 번 시도해 모두 실패했다. 일시 오류면 실패 job만 다시 실행한다(§4) |
| 초안의 태그가 `untagged-…`로 바뀜 | REST API로 초안을 고칠 때 `tag_name`을 빠뜨림(§4). `tag_name=<태그>`를 넣어 다시 고치면 연결된다. Git 태그와 자산은 그대로다 |
| Windows 사용자 업데이트가 "signature 검증 실패" | 빌드 뒤 설치 파일을 다시 서명해 `.sig`와 내용이 달라짐. 서명은 `tauri build` 안에서만 한다 |
| Windows 설치 시 "Windows의 PC 보호" | 미서명 배포의 정상 동작(§3.2). 추가 정보 → 실행 |
| 매니페스트 404 | 엔드포인트 URL ↔ 릴리스에 `latest.json` 첨부 여부 확인 |
