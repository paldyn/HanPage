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

(초안 검증 후 기록)
