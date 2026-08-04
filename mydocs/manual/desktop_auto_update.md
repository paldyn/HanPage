# HanPage Desktop 자동 업데이트 운영 가이드

Tauri v2 updater 기반. 새 릴리스 출시 시 앱이 **업데이트를 알리고 원클릭으로 받아 설치**한다.

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

- **시작 시 자동 확인**(전 desktop): 새 버전이면 `업데이트 있음 / [지금 설치][나중에]` 대화상자 → [지금 설치] → 다운로드·설치·자동 재시작.
- **수동 확인**: macOS는 `HanPage > 업데이트 확인` 메뉴. (Windows/Linux는 시작 시 자동 확인만 — 수동 메뉴는 후속 옵션 B에서 studio 연동 시 추가)

## 6. 주의 사항

- **부트스트랩**: updater가 없던 기존 버전(예: v0.7.13)은 자동 감지 불가. **첫 updater 포함 버전은 사용자가 수동 1회 설치**해야 이후 자동화된다. 릴리스 노트에 안내 권장.
- **macOS Gatekeeper**: 업데이트 전달·설치는 동작하나, 미공증(notarization) 상태에서는 새 버전 첫 실행 시 Gatekeeper 경고가 남는다 → **Task #4(코드 서명·공증)** 완료 시 해소.
- **graceful CI**: 서명 시크릿이 없는 환경(fork)에서는 updater 아티팩트만 생략되고 일반 dmg/nsis 번들은 정상 생성된다.

## 7. 트러블슈팅

| 증상 | 원인·조치 |
|------|----------|
| 업데이트 안 뜸 | 기존 버전에 updater 없음(부트스트랩) / `latest.json` 미첨부 / 버전 비교 동일 |
| "signature 검증 실패" | config `pubkey` ↔ 서명 개인키 불일치(키 교체 후 pubkey 미반영) |
| CI에 updater 아티팩트 없음 | 시크릿 2개 미설정 또는 `createUpdaterArtifacts:false` |
| 매니페스트 404 | 엔드포인트 URL ↔ 릴리스에 `latest.json` 첨부 여부 확인 |
