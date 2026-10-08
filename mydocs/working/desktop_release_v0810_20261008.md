# HanPage Desktop 0.8.10 배포 검증

- 사용자 지시: 2026-10-08 "응 0.8.10 릴리스 준비해". 버전 PR·병합·태그·초안 릴리스 검증까지 진행하고,
  **공개는 검증 결과를 보고한 뒤 별도 승인**을 받는다.
- 포함 변경: [PR #123](https://github.com/paldyn/HanPage/pull/123)(#121 Windows 파일 연결 첫 실행 문서 열기·읽기 실패 안내),
  [PR #124](https://github.com/paldyn/HanPage/pull/124)(#122 macOS 서명·공증 CI 검증, Windows 미서명 배포 결정·설치 안내, CLI 2.11.5).
- base `origin/devel` = `5795d01ed`. 엔진·Studio·root 버전과 Native Rust·WASM source 는 그대로이고 Desktop 5파일의 6개 버전 필드만 0.8.10 으로 맞춘다.

## 버전 필드

`HanPage-Desktop/package.json`, `package-lock.json`(2곳), `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`(rhwp-desktop),
`src-tauri/tauri.conf.json`. `cargo check --locked` 통과로 잠금 파일 일치를 확인했다. 변경 이력 `0.8.10 — 2026-10-08`.

## 공개 게이트

정상 병합 SHA 에 annotated 태그 `hanpage-desktop-v0.8.10` 을 만들고 desktop-release 초안을 다음으로 확인한다.

1. macOS·Windows job 성공, `Verify macOS signature and notarization` 성공(두 `.app` Notarized Developer ID·stapler),
   Windows 요약 `Authenticode NotSigned (미서명 배포, #122)`.
2. 자산 6개와 크기·digest, `latest.json` 플랫폼 4개·같은 태그 URL·버전 0.8.10.
3. updater 서명(minisign)이 실제 자산으로 검증되고 1바이트 변조를 거부한다.
4. 번들 메타데이터 버전이 0.8.10 이다(updater `.app` 의 `CFBundleShortVersionString`, setup.exe 버전 문자열, `latest.json`).
5. 릴리스 노트에 Windows 설치 안내("Windows의 PC 보호" → 추가 정보 → 실행)가 있다.

태그 전에 `claude/desktop-release-0.8.10` 으로 desktop-release dispatch 를 한 번 더 실행해 릴리스할 source 의 양 플랫폼 빌드를
확인한다(앞선 #124 시험 run 37724295849 는 #123 의 `lib.rs` 변경 전 커밋이었다). 태그는 옮길 수 없으므로 빌드 실패를 태그 전에 걸러낸다.

공개 전에는 0.8.9 가 latest 로 남는다. 실패하면 태그를 옮기거나 secret 을 바꾸지 않고 같은 source 의 실패 job 만 복구한다.

## 알려진 한계

- #121 의 Windows 실기기 동작(파일 연결 첫 실행·NSIS 업데이트 재실행·Explorer 다중 선택)은 미검증이다. macOS 동일 경로 재현과
  소스 대조로 대신했으며 #121 은 Windows 확인 전까지 연다. 릴리스 노트도 이 한계를 밝힌다.
- 앱 정보 화면의 `Version` 은 Studio `package.json`(0.8.6)을 표시한다. 0.8.9 와 같은 기존 동작이다.
- Windows 0.8.9 를 파일 더블클릭으로 실행한 상태에서 0.8.10 으로 업데이트하면, NSIS 가 이전 실행 인자를 넘겨 재시작 직후
  그 파일을 한 번 다시 열거나(경로에 공백이 있으면) 열 수 없다는 안내가 한 번 뜰 수 있다. 0.8.9 는 재실행 표식을 남기지 않기 때문이며 0.8.10 부터는 생기지 않는다.

## 결과

| 단계 | 결과 |
| --- | --- |
| PR #125 | CI 28 성공·6 건너뜀, squash 병합 `3c598548c3c3560156a9ec8e7ec26f5a169438fa` |
| 태그 전 시험 | desktop-release dispatch [run 37729208261](https://github.com/paldyn/HanPage/actions/runs/37729208261) (`ed82f5def`): 양 플랫폼 번들 성공, macOS 공증 검증 통과. 병합 커밋 tree `4aa147fc` 가 시험 source 와 동일 |
| 태그 | annotated `hanpage-desktop-v0.8.10` → `3c598548c` |
| 태그 빌드 | [run 37731061888](https://github.com/paldyn/HanPage/actions/runs/37731061888) 성공. macOS `Verify macOS signature and notarization`(태그 필수 경로) 성공, Windows 서명 상태 보고 실행. 초안 생성 |
| 게이트 2~4 | [감사 결과](../pr/assets/desktop_v0810_release/draft-audit.json) 18/18 통과 — 자산 6개, `latest.json` 버전·플랫폼 4개·같은 태그 URL·`.sig` 일치, minisign 서명(키 `835d6b3831e133aa`, 0.8.9 와 동일) 검증과 1바이트 변조 거부, updater `.app` 0.8.10, setup.exe 버전 문자열, setup.exe Authenticode 없음. 감사 스크립트는 공개된 0.8.9 자산에서도 18/18 통과(대조군) |
| macOS 로컬 | updater `.app`·DMG 안 `.app`: codesign strict, `Developer ID Application: Wonmo Lee (8L78W6D8XF)`, Gatekeeper `accepted / Notarized Developer ID`, stapler 정상, 0.8.10·arm64·`com.paldyn.hanpage`. DMG checksum VALID |
| 자산 digest | 6개 모두 GitHub API `digest` 와 로컬 SHA-256 일치 |
| 게이트 5 | [릴리스 노트](../pr/assets/desktop_v0810_release/release-notes.md)를 초안에 반영(Windows 설치 안내·#121 한계 공개) |
| 공개 | 작업지시자 승인 "공개해"(2026-10-08) 후 2026-10-08 05:36:42Z [공개](https://github.com/paldyn/HanPage/releases/tag/hanpage-desktop-v0.8.10). GitHub latest·API latest·updater 엔드포인트 모두 0.8.10, 공개 `latest.json` 이 감사한 파일과 SHA-256 동일, DMG·setup.exe·updater 아카이브 공개 URL 응답 정상. 0.8.9·0.8.8 릴리스는 유지 |

#121 은 Windows 실기기에서 0.8.10 파일 연결 첫 실행을 확인한 뒤 닫는다.
