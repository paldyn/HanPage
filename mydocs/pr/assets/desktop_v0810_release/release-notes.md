HanPage Desktop 0.8.10

- Windows에서 앱이 꺼져 있을 때 한글 파일(.hwp/.hwpx)을 더블클릭하거나 `연결 프로그램`으로 열면 빈 문서만 뜨던 문제를 수정했습니다. 앱이 시작되면 그 문서를 엽니다.
- 파일 연결이나 최근 문서로 연 파일을 읽지 못하면 아무 반응 없이 끝나지 않고, 열 수 없다는 안내와 사유를 표시합니다.
- 이미 실행 중인 앱에 명령줄 등에서 다른 폴더 기준 상대 경로로 넘긴 파일도 올바른 위치에서 엽니다.

**Windows 설치 안내** — 설치 파일은 코드 서명 없이 배포합니다. 브라우저가 "일반적으로 다운로드되지 않는 파일"로 표시하면 `⋯` 메뉴에서 `유지`를 선택하세요. 처음 실행할 때 "Windows의 PC 보호" 화면이 뜨면 `추가 정보 → 실행`을 누르세요. 공식 배포처는 이 GitHub Releases와 hanpage.paldyn.com의 다운로드 버튼뿐입니다.

macOS Apple Silicon은 DMG, Windows x64는 설치 EXE를 사용할 수 있습니다. 기존 앱에서는 자동 업데이트 또는 업데이트 확인 메뉴를 이용할 수 있습니다.

양 플랫폼 빌드와 실제 배포 자산, 자동 업데이트 서명, macOS 코드 서명·공증을 확인했습니다. Windows 파일 연결 수정은 같은 실행 경로를 macOS에서 재현해 확인했으며, 문제가 있으면 [#121](https://github.com/paldyn/HanPage/issues/121)에 알려 주세요. 변경 PR: [#123](https://github.com/paldyn/HanPage/pull/123), [#124](https://github.com/paldyn/HanPage/pull/124), [#125](https://github.com/paldyn/HanPage/pull/125).
