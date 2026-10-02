# PR #113 검토 구현 기록

- source 변경은 커밋0ac1f1bd7이며 이후 commit은 증거·검토 기록이다.
- Desktop npm lock의 오래된0.8.2를0.8.6으로 맞추고 Cargo lock은 rhwp-desktop 항목만 갱신했다.
- 공급 PNG/ICNS byte 동일, ICO 프레임 payload/픽셀 동일·256px 우선, 실제 app ICNS 동일 검증.
- Root engine/Studio source는 PR #111 결과와 동일. 새 Render Diff/Hancom 출력 개선은 주장하지 않는다.
- 실제 로컬 native build와 .app packaging 성공, 기존 workflow 계약9개 성공, YAML/diff 통과. 초기 환경 실패는 [원장](../assets/task_m100_hp112_release/local-validation.json)에 성공 재실행과 함께 보존했다.
- source 변경 후 PR #113 최신 CI 성공을 병합 직전에 확인한다. 초안 릴리스 두 플랫폼 자산·서명·공증 검증 후 공개한다.
