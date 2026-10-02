# PR #114 검토 구현 기록

- source549411762는 wasm-pack v0.13.1 pin과 runtime version guard, macOS .sh/Windows pwsh .ps1 wrapper 분기를 포함한다. root·Desktop dependency resolution과 production engine/Studio/app runtime source는 바뀌지 않는다.
- native Windows metadata subprocess는 shebang shim 대신 cargo.exe proxy/sibling wasm-pack.exe를 사용한다. CI의 CARGO override 없음도 확인했다.
- Desktop0.8.7의 fresh locked/no-opt WASM·frontend/native compile·unsigned app package와 기존 계약9개가 실제로 통과했다. 앱0.8.7·새 ICNS·WASM·lock 불변을 [원장](../assets/task_m100_hp112_release/desktop-v087-local-validation.json)에 고정했다.
- #113 최신 CI/실제 merge와 실패한0.8.6 초안의 원인·공개 미영향을 permanent assets에 보존했다. 원본 입력39파일의 primary 해시는 그대로다.
- 현재 trailing self-review/오늘할일 head의 최신 CI가 최종 merge gate이며 실제 배포 자산 검증까지 별도로 완료한다. 이전 실패 태그를 force 이동하거나 공개 불완전 자산을 덮어쓰지 않는다.
