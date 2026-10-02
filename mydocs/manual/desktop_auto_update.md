---
kind: guide
status: active
canonical: mydocs/manual/desktop_auto_update.md
last_verified: 2026-10-02
---

# HanPage Desktop 자동 업데이트 운영 가이드

Tauri v2 updater 기반. 새 릴리스 출시 시 앱이 **백그라운드로 받아두고, 사용자가 업데이트를 선택하면 적용**한다.

## 1. 구성 요약

| 요소 | 위치 |
|------|------|
| 플러그인 | `tauri-plugin-updater`(Rust, desktop 전용) — `HanPage-Desktop/src-tauri/Cargo.toml`·`lib.rs` |
| 설정 | `tauri.conf.json`: `bundle.createUpdaterArtifacts: true` + `plugins.updater {endpoints, pubkey}` |
| 배포원 | GitHub Releases + `latest.json` 매니페스트 |
| 엔드포인트 | `https://github.com/paldyn/HanPage/releases/latest/download/latest.json` |
| 서명 | ed25519/minisign. 공개키=config, **개인키=GitHub 시크릿** |
| CI | `desktop-release.yml`의 `tauri-action`이 서명·`latest.json`·첨부 자동 |

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

## 3. 릴리스 절차

```bash
# 1) 버전 올림: HanPage-Desktop/src-tauri/tauri.conf.json "version" + package.json
# 2) 태그 push → desktop-release.yml 트리거
git tag hanpage-desktop-v0.7.14
git push origin hanpage-desktop-v0.7.14
```
- CI(`tauri-action`)가 macOS(dmg)·Windows(nsis) 번들 + **서명된 updater 아티팩트** + `latest.json` 생성 → GitHub Release에 첨부.
- 다음 실행부터 기존 사용자 앱이 `latest.json`을 확인 → 새 버전 감지.

## 4. latest.json 형식 (tauri-action 자동 생성, 참고)

```json
{
  "version": "0.7.14",
  "notes": "릴리스 노트",
  "pub_date": "2026-06-11T00:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "<minisign 서명>",
      "url": "https://github.com/paldyn/HanPage/releases/download/hanpage-desktop-v0.7.14/HanPage_0.7.14_aarch64.app.tar.gz"
    },
    "windows-x86_64": {
      "signature": "<minisign 서명>",
      "url": "https://github.com/paldyn/HanPage/releases/download/hanpage-desktop-v0.7.14/HanPage_0.7.14_x64-setup.nsis.zip"
    }
  }
}
```

## 5. 사용자 경험

- 시작할 때 조용히 새 버전을 확인하고 백그라운드로 다운로드한다. 최신이면 알림을 띄우지 않는다.
- 준비되면 앱 테마에 맞는 업데이트 카드가 한 번 표시된다. **업데이트**를 누르면 기존 저장 확인 절차를 거친 뒤 적용하고 다시 시작한다. Windows에서는 설치 프로그램이 열린다.
- **나중에** 또는 닫기를 선택해도 받아둔 파일은 앱이 실행 중인 동안 유지된다. 상태 표시줄의 **업데이트 확인 / 업데이트 준비됨** 버튼으로 언제든 다시 열 수 있다. macOS의 `HanPage > 업데이트 확인` 메뉴도 같은 카드를 연다.
- 앱을 종료했다가 다시 열면 버전을 다시 확인한다. 다운로드한 파일은 메모리에 보관하므로 필요한 경우 다시 받는다.
- 확인 중·다운로드·파일 검증·적용을 구분해서 표시한다. 다운로드는 실제 누적 용량과 전체 용량으로 진행률을 표시한다. 전체 크기를 모르면 받은 용량과 움직이는 진행 막대만 표시하며 임의의 퍼센트를 만들지 않는다.
- 적용 중에는 진행 막대와 다시 시작 안내를 표시하고 중복 클릭을 막는다. 파일 교체는 화면 스레드 밖에서 실행한다. 적용 전 저장 여부를 확인하며, 적용 도중에는 편집·단축키·네이티브 메뉴를 잠그고 실패하면 복구한다.
- 적용에 실패하면 받아둔 파일을 유지하며 **다시 업데이트**로 재시도한다. 네트워크 확인·다운로드 실패는 **다시 확인**으로 시작한다.
- 웹 빌드는 이 카드와 진입 버튼을 생성하지 않는다.

## 6. 주의 사항

- **부트스트랩**: updater가 없던 기존 버전(예: v0.7.13)은 자동 감지 불가. **첫 updater 포함 버전은 사용자가 수동 1회 설치**해야 이후 자동화된다. 릴리스 노트에 안내 권장.
- **macOS 배포**: 코드 서명·Apple 공증·stapler와 Gatekeeper 검증이 완료된 산출물을 게시한다. 0.8.7 실제 배포 검증은 [PR 114 기록](../pr/archives/pr_114_review.md)을 참고한다.
- **게시 게이트**: 필수 번들·업데이트 서명·매니페스트·공증 검증이 실패하면 공개 게시를 완료하지 않는다. UI 로컬 검증은 실제 업데이트 설치와 공개 릴리스 검증을 대신하지 않는다.

## 7. 트러블슈팅

| 증상 | 원인·조치 |
|------|----------|
| 업데이트 안 뜸 | 기존 버전에 updater 없음(부트스트랩) / `latest.json` 미첨부 / 버전 비교 동일 |
| "signature 검증 실패" | config `pubkey` ↔ 서명 개인키 불일치(키 교체 후 pubkey 미반영) |
| CI에 updater 아티팩트 없음 | 시크릿 2개 미설정 또는 `createUpdaterArtifacts:false` |
| 매니페스트 404 | 엔드포인트 URL ↔ 릴리스에 `latest.json` 첨부 여부 확인 |
