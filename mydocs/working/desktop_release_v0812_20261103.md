# HanPage Desktop 0.8.12 배포 검증

- 사용자 지시: 2026-11-03 "0.8.12 릴리스 준비해" — 버전 준비, 태그 전 시험 빌드, 초안 검증까지 진행하고 **공개는 검증
  결과를 보고한 뒤 별도 승인**을 받는다. 릴리스 PR 병합과 태그 생성도 승인을 받아 진행한다.
- 포함 변경(0.8.11 이후 devel 병합):
  - [PR #155](https://github.com/paldyn/HanPage/pull/155) — upstream rhwp 0.8.7 통합과 HanPage 기능 보정(#154)
  - [PR #157](https://github.com/paldyn/HanPage/pull/157) — 문서마다 창 하나(#156)
  - [PR #168](https://github.com/paldyn/HanPage/pull/168) — Windows 혼합 배율 모니터의 새 창 위치(#159)
  - [PR #160](https://github.com/paldyn/HanPage/pull/160) — 데스크톱 크레이트 PR 검사(#158, CI 만 바뀜)
- 엔진·Studio·root 버전(upstream 0.8.7)은 그대로이고 Desktop 5파일의 6개 버전 필드만 0.8.12 로 맞춘다.

## 버전 필드

`HanPage-Desktop/package.json`, `package-lock.json`(2곳), `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`(rhwp-desktop),
`src-tauri/tauri.conf.json`. `cargo check --locked` 통과로 잠금 파일 일치를 확인했다(남은 0.8.11 은 의존 크레이트
`regex-syntax`). 변경 이력 `0.8.12 — 2026-11-03`.

## 공개 게이트

0.8.11 과 같다. 정상 병합 SHA 에 annotated 태그 `hanpage-desktop-v0.8.12` 를 만들고 desktop-release 초안을 다음으로 확인한다.

1. macOS·Windows job 성공, `Verify macOS signature and notarization` 성공(두 `.app` Notarized Developer ID·stapler),
   Windows 요약 `Authenticode NotSigned (미서명 배포, #122)`.
2. 자산 6개와 크기·digest, `latest.json` 플랫폼 4개·같은 태그 URL·버전 0.8.12.
3. updater 서명(minisign)이 실제 자산으로 검증되고 1바이트 변조를 거부한다.
4. 번들 메타데이터 버전이 0.8.12 이다(updater `.app` 의 `CFBundleShortVersionString`, setup.exe 버전 문자열, `latest.json`).
5. 릴리스 노트에 Windows 설치 안내("Windows의 PC 보호" → 추가 정보 → 실행)가 있고, 실기에서 확인하지 않은 플랫폼 동작을
   확인된 것처럼 쓰지 않는다.

태그 전에 `claude/desktop-release-0.8.12` 로 desktop-release dispatch 를 실행해 양 플랫폼 빌드를 확인한다. 태그는 옮길 수 없으므로
빌드 실패를 태그 전에 걸러낸다. 공개 전에는 0.8.11 이 latest 로 남는다. 실패하면 태그를 옮기거나 secret 을 바꾸지 않고 같은
source 의 실패 job 만 복구한다.

## 공개 전 실기 확인(초안 설치본)

이번 변경 중 실기에서 확인하지 못한 동작이다(#156·#154·#159 보고서). 초안 설치본으로 작업지시자가 확인한 뒤 공개를 결정한다.

| 항목 | 플랫폼 |
| --- | --- |
| 다른 한글 파일은 새 창, 같은 파일은 그 창으로, 빈 창 재사용(Finder·탐색기 더블클릭·파일 > 열기·macOS 최근 문서 메뉴) | Windows·macOS(macOS 는 파일 연결 경로만 실기 확인) |
| 끌어다 놓은 파일은 빈 창이면 그 창, 아니면 새 창(같은 파일 판단 없음) | Windows·macOS |
| 창 제목에 문서 이름 | Windows·macOS |
| 저장하지 않은 창 닫기 확인, Cmd+Q 차례 확인 | macOS·Windows(창 닫기) |
| 배율이 다른 모니터를 함께 쓸 때 새 창 위치 | Windows(해당 환경이 있을 때) |
| 파일 > 열기의 PDF·HML, HWPX 저장 확장자, macOS 편집 메뉴와 AI 입력창 | macOS·Windows |
| AI 도우미: 도구 메뉴, AI 설정의 키 등록, 문서 내용 전송 동의, 질문·답변, 자동 적용 끄고 켜기 | macOS·Windows |
| 0.8.11 에서 0.8.12 로 자동 업데이트 | macOS·Windows |

## 알려진 한계

- 0.8.11 → 0.8.12 업데이트는 0.8.11 의 단일 창 업데이트 흐름으로 진행된다. 여러 창의 업데이트 사전 확인·알림 1회는 0.8.12 를
  설치한 다음 업데이트부터 적용된다.
- 다중 창: 끌어다 놓은 파일은 경로가 없어 같은 파일 판단을 하지 않는다(이미 열린 파일도 새 창). 업데이트를 적용할 때는
  그 창의 AI 도우미만 먼저 멈추고, 다른 창은 적용 중 카드와 입력 보호만 띄운다. macOS Dock 종료·로그아웃과 Windows 로그오프는 창에 묻지 않는다. 두 번째 이후 창이 비정상 종료되면 그 창에서는 자동 저장
  복구를 제안하지 않는다. 설정은 마지막으로 바꾼 창의 값이 남는다.
- macOS 에서 다른 앱 뒤에 가려진 채 시작한 창은 WebKit 이 웹 프로세스를 일시 정지해, 앞으로 가져올 때까지 문서를 열지 않을 수 있다
  (터미널 실행 실기 관측, #156 보고서 3.3). 사용자가 직접 연 경우에는 앱이 앞으로 나온다.
- AI 도우미는 rhwp 0.8.7 통합으로 0.8.12 에 처음 들어오는 기능이다. 데스크톱 실기 확인은 위 표로 한다.
- 앱 정보 화면의 `Version` 은 Studio `package.json` 을 표시한다. 기존과 같은 동작이다.

## 결과

| 단계 | 결과 |
| --- | --- |
| 공개 문구 독립 대조 | 변경 이력·릴리스 노트·게이트 기록을 출시 코드·보고서와 대조(에이전트 28, 발견마다 검증자 2명). 확인 3건 반영: 끌어다 놓기에는 같은 파일 규칙이 적용되지 않음(문구 범위 정정·README 한계 추가), AI 도우미가 새 기능인데 소개 없이 결함 수정처럼 씀(새 기능 항목 추가 — 작업지시자 결정 '포함하고 소개'), 실기 확인 표 누락(macOS 열기 경로·창 제목·AI 도우미 추가). 절반 인정된 'AI 정지 범위' 도 문구를 그 창으로 좁힘 |
| 태그 전 시험 | desktop-release dispatch [run 41558815960](https://github.com/paldyn/HanPage/actions/runs/41558815960) (`73ff09f63`): 양 플랫폼 번들 성공, `Verify macOS signature and notarization` 성공, Windows 서명 상태 보고 실행. 이후 커밋은 문서(변경 이력·README·이 기록)만 바꾼다 |
| PR #169 | CI 33 성공·5 건너뜀(Desktop check macOS·Windows 포함, 건너뜀은 경로 조건), 작업지시자 승인("응" — CI 가 모두 통과하면 병합·태그) 후 squash 병합 `1b1d49c58b73726d5089e19b3b62b1a3b033852c`. 병합 커밋 tree `fa2a9bae` 가 PR head `f2f8736c4` 와 동일 |
| 태그 | annotated `hanpage-desktop-v0.8.12` → `1b1d49c58` |
| 태그 빌드 | [run 41574281039](https://github.com/paldyn/HanPage/actions/runs/41574281039). 첫 시도: Windows 성공(04:13Z), macOS 는 공증 `Accepted` 뒤 DMG 를 만드는 `bundle_dmg.sh` 가 11초 만에 실패했다. 같은 러너 이미지(`macos-14-arm64` 20260831.0302.1)·같은 source(문서만 다름)의 태그 전 시험에서는 `bundle_dmg.sh` 가 26초 만에 성공해 DMG 를 만들었으므로 러너의 일시 실패로 보고, 태그·secret 은 그대로 둔 채 실패한 macOS job 만 다시 실행했다(attempt 2, 04:18Z). 실패한 시도는 초안에 macOS 자산을 올리지 않았다. attempt 2: macOS 성공 — 공증 `Accepted`(04:45:01Z)·스테이플, `bundle_dmg.sh` 26초, 자산 업로드, `Verify macOS signature and notarization` 성공(04:45:40–43Z). Windows 는 attempt 1 의 결과(04:12Z 업로드)를 그대로 쓴다. Windows `Report Windows installer signature status` 단계는 성공했으나 요약 본문은 GitHub 로그인 상태에서만 보여 읽지 못했다 — 설치 파일에 Authenticode 가 없음은 아래 감사가 PE 보안 디렉터리로 따로 확인했다. 실행 결론 success, 초안 생성 |
| 감사 스크립트 대조군 | 0.8.11 의 [감사 스크립트](../pr/assets/desktop_v0811_release/audit-draft.mjs)·[macOS 로컬 스크립트](../pr/assets/desktop_v0811_release/macos-local.sh)를 그대로 쓴다. 공개된 0.8.11 자산으로 먼저 실행해 감사 18/18 통과, macOS 로컬 결과가 기록된 [0.8.11 증적](../pr/assets/desktop_v0811_release/macos-local.txt)과 같음(임시 경로 표기만 다름)을 확인했다(node 22.22.1, macOS 26.6.2) |
| 게이트 2~4 | [감사 결과](../pr/assets/desktop_v0812_release/draft-audit.json) 18/18 통과 — 자산 6개, [`latest.json`](../pr/assets/desktop_v0812_release/latest.json) 버전 0.8.12·플랫폼 4개·같은 태그 URL·`.sig` 일치·`notes` 빈 값, minisign 서명(키 `835d6b3831e133aa`, 0.8.11 과 같음) 검증과 1바이트 변조 거부, updater `.app` 0.8.12, setup.exe 버전 문자열, setup.exe Authenticode 없음. Windows 자산은 macOS 재실행 중에 먼저 받아 같은 검사(11/11)를 했고, 최종 초안의 setup.exe 와 바이트 단위로 같다 |
| macOS 로컬 | [확인 결과](../pr/assets/desktop_v0812_release/macos-local.txt)(0.8.11 [스크립트](../pr/assets/desktop_v0811_release/macos-local.sh)) — updater `.app`·DMG 안 `.app`: codesign strict, `Developer ID Application: Wonmo Lee (8L78W6D8XF)`, Gatekeeper `accepted / Notarized Developer ID`, stapler 정상, 0.8.12·arm64·`com.paldyn.hanpage`. DMG checksum VALID. 결과가 0.8.11 증적과 버전 외에 같다 |
| 자산 digest | 6개 모두 GitHub API `digest` 와 로컬 SHA-256 일치. 크기는 0.8.11 보다 세 설치본 모두 약 8.2MB 커졌다(DMG 43,729,414→51,978,477, updater 아카이브 42,965,888→51,207,310, setup.exe 40,150,833→48,405,242). 플랫폼 공통으로 같은 폭이라 앱에 내장되는 웹 자산(엔진 0.8.7 통합)의 증가와 맞고, 태그 전 시험의 Windows 아티팩트(zip 48,406,023바이트)도 같은 규모였다 |
| 독립 검증 | 공개 판단 전 반박 검증(에이전트 3, 메인터너 스크립트 재사용 금지, 읽기 전용) — 세 영역 모두 통과, 차단 우려 0. ① CI·출처: 태그·PR #169 병합·devel·실행 head 가 같은 SHA, attempt 2 의 검증 단계가 태그 시점 workflow 와 같은 스크립트로 두 `.app` 을 확인, macOS 자산·`latest.json` 은 attempt 2 가, Windows 자산은 attempt 1 이 올렸고 그 뒤 수정 없음, 재실행 전후 secret 변경 없음. ② 자산·서명·버전: OpenSSL(Ed25519·BLAKE2b-512)로 두 updater 서명 재검증, 여러 위치 변조·trusted comment 변조 거부, 0.8.11·0.8.12 실행 파일에 내장된 공개키가 태그 공개키와 같음(기존 0.8.11 설치본이 이 업데이트를 받아들임), setup.exe 와 내부 `HanPage.exe` 의 VS_VERSIONINFO 0.8.12.0, DMG 안 앱과 updater 앱의 CDHash·파일이 같음. ③ 릴리스 노트: 아래 게이트 5. 참고(차단 아님): Developer ID 인증서 만료 2027-02-01(이번 서명은 보안 타임스탬프로 유효, 그 뒤 릴리스는 갱신 필요), DMG 컨테이너 자체는 서명만 되고 공증·스테이플은 안 됨(0.8.11 과 같음 — 공증은 안의 `.app`), CI 검증 단계가 업로드 뒤 디스크의 `.app` 을 확인하는 구조(이번에는 CDHash·CRC 대조로 메움), 감사 스크립트의 setup.exe 버전 검사는 문자열 포함 여부만 봄(독립 파싱으로 보완) |
| 게이트 5 | [릴리스 노트](../pr/assets/desktop_v0812_release/release-notes.md)를 초안 본문에 반영(문서마다 창 하나·AI 도우미 새 기능·그 밖의 변경, Windows 설치 안내, 확인 범위 문장). 독립 검증이 짚은 문장 하나를 코드 범위에 맞게 고쳤다 — 이미 열린 창을 앞으로 가져오는 최근 문서 경로는 macOS 의 `파일 > 최근 문서` 뿐이고(네이티브 메뉴가 macOS 전용, Windows 최근 목록은 경로 없이 열어 파일 선택으로 돌아감), 끌어다 놓아 연 문서는 경로가 없어 나중에 같은 파일을 열면 새 창이 생긴다(README 한계와 같음). 같은 수정을 devel 의 변경 이력에도 넣었다(태그 시점 변경 이력은 이전 문구, 앱에 포함되지 않음). AI 자동 적용의 "(Ctrl+Z로 되돌릴 수 있음)" 은 새 문서 만들기가 되돌리기 대상이 아니지만 저장하지 않은 문서는 확인을 거치고 앱 설정 문구와 같아 유지했다. 본문을 API 로 고칠 때 `tag_name` 없이 보낸 첫 PATCH 가 초안의 태그를 `untagged-…` 로 풀었다 — 곧바로 `tag_name=hanpage-desktop-v0.8.12` 를 넣어 다시 연결했고, Git 태그(`242cb0ca`→`1b1d49c5`)·대상 커밋·자산 6개(digest·업로드 시각)·본문이 그대로임을 확인했다. 초안을 고치거나 공개할 때는 `tag_name` 을 함께 보내고 공개 직전에 다시 확인한다 |
| 공개 | 작업지시자 승인 "공개해"(2026-11-03) 후 2026-11-03 05:32:53Z [공개](https://github.com/paldyn/HanPage/releases/tag/hanpage-desktop-v0.8.12). 공개 직전 `tag_name`·대상 커밋·본문·자산 6개 digest 를 다시 확인했고 `tag_name` 을 함께 보내 공개했다. GitHub latest·API latest 0.8.12, 태그 그대로(`untagged` 태그 생성 없음). updater 엔드포인트(`releases/latest/download/latest.json`)는 공개 14초 뒤(05:33:07Z)부터 0.8.12 로 redirect 됐고 05:33:48Z 세 번 모두 같았으며, 응답이 감사한 [`latest.json`](../pr/assets/desktop_v0812_release/latest.json)과 SHA-256 동일. 공개 자산 6개 익명 요청 정상(206, 첫 바이트), 공개 뒤 digest 그대로. 홈페이지 다운로드 버튼(API latest 조회)이 고르는 파일도 0.8.12 DMG·setup.exe 다. 0.8.11 이하 릴리스는 유지. 위 "공개 전 실기 확인" 표의 결과는 받지 않았다 — 미확인으로 남기며, 0.8.11 → 0.8.12 자동 업데이트는 공개 뒤부터 시험할 수 있다 |
