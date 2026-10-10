# HanPage Desktop 0.8.11 배포 검증

- 사용자 지시: 2026-10-10 "응" — #126 이슈·PR 병합 뒤 0.8.11 버전 준비, 태그 전 시험 빌드, 초안 검증까지 진행하고
  **공개는 검증 결과를 보고한 뒤 별도 승인**을 받는다.
- 포함 변경: [PR #127](https://github.com/paldyn/HanPage/pull/127)(#126 업데이트 준비 완료 작은 알림 1회).
- 엔진·Studio·root 버전과 Native Rust·WASM source 는 그대로이고 Desktop 5파일의 6개 버전 필드만 0.8.11 로 맞춘다.

## 버전 필드

`HanPage-Desktop/package.json`, `package-lock.json`(2곳), `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`(rhwp-desktop),
`src-tauri/tauri.conf.json`. `cargo check --locked` 통과로 잠금 파일 일치를 확인했다. 변경 이력 `0.8.11 — 2026-10-10`.

## 공개 게이트

0.8.10 과 같다. 정상 병합 SHA 에 annotated 태그 `hanpage-desktop-v0.8.11` 을 만들고 desktop-release 초안을 다음으로 확인한다.

1. macOS·Windows job 성공, `Verify macOS signature and notarization` 성공(두 `.app` Notarized Developer ID·stapler),
   Windows 요약 `Authenticode NotSigned (미서명 배포, #122)`.
2. 자산 6개와 크기·digest, `latest.json` 플랫폼 4개·같은 태그 URL·버전 0.8.11.
3. updater 서명(minisign)이 실제 자산으로 검증되고 1바이트 변조를 거부한다.
4. 번들 메타데이터 버전이 0.8.11 이다(updater `.app` 의 `CFBundleShortVersionString`, setup.exe 버전 문자열, `latest.json`).
5. 릴리스 노트에 Windows 설치 안내("Windows의 PC 보호" → 추가 정보 → 실행)가 있다.

태그 전에 `claude/desktop-release-0.8.11` 로 desktop-release dispatch 를 실행해 양 플랫폼 빌드를 확인한다. 이 브랜치는 PR #127
최종 head(`5c5dc79bc`) 위의 버전 커밋으로 시작해 #127 CI 와 병행하며, #127 병합 뒤 devel 위로 옮긴다. 옮긴 뒤의 tree 가 시험
source 와 다르면 그 차이(병합 후 문서 등)를 기록한다. 태그는 옮길 수 없으므로 빌드 실패를 태그 전에 걸러낸다.

공개 전에는 0.8.10 이 latest 로 남는다. 실패하면 태그를 옮기거나 secret 을 바꾸지 않고 같은 source 의 실패 job 만 복구한다.

## 알려진 한계

- 준비 완료 알림은 앱 안의 기능이라 0.8.11 을 설치한 다음 업데이트부터 보인다. 0.8.10 → 0.8.11 은 기존처럼 하단 상태 버튼으로 안내된다.
- 알림의 실제 Tauri 업데이트 경로와 macOS WebKit·Windows WebView2 의 창 전환·최소화 동작은 미검증이다(#127 결과 보고 4절).
- 앱 정보 화면의 `Version` 은 Studio `package.json` 을 표시한다. 기존과 같은 동작이다.

## 결과

| 단계 | 결과 |
| --- | --- |
| PR #127 | CI 30 성공·4 건너뜀, squash 병합 `6dbff3c92cef39c000d5f5c40404012fbc7a81d7`, 병합 기록 `ec3b1dcff` |
| 태그 전 시험 | desktop-release dispatch [run 38047022225](https://github.com/paldyn/HanPage/actions/runs/38047022225) (`75f57aca4`, #127 최종 head 위 버전 커밋): 양 플랫폼 번들 성공, `Verify macOS signature and notarization` 성공, Windows 서명 상태 보고 실행. devel 위로 옮긴 버전 커밋과 시험 source 의 차이는 `mydocs/orders/20261010.md`·`mydocs/pr/archives/pr_127_review.md` 문서 2개뿐(비문서 차이 0) |
| PR #128 | CI 27 성공·3 건너뜀, squash 병합 `a431608afdf79059c6f47b2b98a795a98286156b`. 병합 커밋 tree `72cb18ac` 가 PR head 와 동일 |
| 태그 | annotated `hanpage-desktop-v0.8.11` → `a431608af` |
| 태그 빌드 | [run 38049768297](https://github.com/paldyn/HanPage/actions/runs/38049768297) 성공. macOS `Verify macOS signature and notarization`(태그 필수 경로) 성공, Windows 서명 상태 보고 실행. 초안 생성 |
| 게이트 2~4 | [감사 결과](../pr/assets/desktop_v0811_release/draft-audit.json) 18/18 통과 — 자산 6개, `latest.json` 버전·플랫폼 4개·같은 태그 URL·`.sig` 일치, minisign 서명(키 `835d6b3831e133aa`, 0.8.10 과 동일) 검증과 1바이트 변조 거부, updater `.app` 0.8.11, setup.exe 버전 문자열, setup.exe Authenticode 없음. 같은 스크립트가 공개된 0.8.10 자산에서도 통과(대조군) |
| macOS 로컬 | [확인 결과](../pr/assets/desktop_v0811_release/macos-local.txt)([스크립트](../pr/assets/desktop_v0811_release/macos-local.sh)) — updater `.app`·DMG 안 `.app`: codesign strict, `Developer ID Application: Wonmo Lee (8L78W6D8XF)`, Gatekeeper `accepted / Notarized Developer ID`, stapler 정상, 0.8.11·arm64·`com.paldyn.hanpage`. DMG checksum VALID. 공개된 0.8.10 자산으로 먼저 대조 실행 |
| 자산 digest | 6개 모두 GitHub API `digest` 와 로컬 SHA-256 일치 |
| 게이트 5 | [릴리스 노트](../pr/assets/desktop_v0811_release/release-notes.md)를 초안에 반영(작은 알림 설명·적용 시점, Windows 설치 안내) |
| 공개 | 작업지시자 승인 "응"(2026-10-10) 후 2026-10-10 12:13:05Z [공개](https://github.com/paldyn/HanPage/releases/tag/hanpage-desktop-v0.8.11). GitHub latest·API latest 0.8.11. updater 엔드포인트(`releases/latest/download/latest.json`)는 공개 직후 약 1분간 0.8.10 으로 redirect 되다가 12:14Z 무렵 0.8.11 로 바뀌었고, 응답이 감사한 [`latest.json`](../pr/assets/desktop_v0811_release/latest.json)과 SHA-256 동일. DMG·setup.exe·updater 아카이브 공개 URL 응답 200. 0.8.10·0.8.9 릴리스는 유지 |
