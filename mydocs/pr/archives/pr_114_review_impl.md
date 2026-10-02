# PR #114 검토 구현 기록

- source549411762는 wasm-pack v0.13.1 pin과 runtime version guard, macOS .sh/Windows pwsh .ps1 wrapper 분기를 포함한다. root·Desktop dependency resolution과 production engine/Studio/app runtime source는 바뀌지 않는다.
- native Windows metadata subprocess는 shebang shim 대신 cargo.exe proxy/sibling wasm-pack.exe를 사용한다. CI의 CARGO override 없음도 확인했다.
- Desktop0.8.7의 fresh locked/no-opt WASM·frontend/native compile·unsigned app package와 기존 계약9개가 실제로 통과했다. 앱0.8.7·새 ICNS·WASM·lock 불변을 [원장](../assets/task_m100_hp112_release/desktop-v087-local-validation.json)에 고정했다.
- #113 최신 CI/실제 merge와 실패한0.8.6 초안의 원인·공개 미영향을 permanent assets에 보존했다. 원본 입력39파일의 primary 해시는 그대로다.
- 현재 trailing self-review/오늘할일 head의 최신 CI가 최종 merge gate이며 실제 배포 자산 검증까지 별도로 완료한다. 이전 실패 태그를 force 이동하거나 공개 불완전 자산을 덮어쓰지 않는다.


## 실제 공개 검증 완료

- 최종 trailing head440d57b0616adc8a10ce4415b0383a9372064aa5의32checks를 확인해b762cbdbbe0d57b379bb8cf29393599e078c9eaa으로 정상 merge했다. 실제 test counts와 duration 증거 부족에 따른 갱신 보류는 [최종 CI 원장](../assets/task_m100_hp112_release/pr114-final-ci-summary.json)에 보존했다.
- Apple 계약403 실패는 사용자 동의 뒤 같은 source/tag의 Mac job만 재시도해 Accepted로 해소했다. Windows의 기존 성공 설치 파일/서명은 그대로이며 추가 source 변경은 없다.
- 실제6자산 inventory·4platform manifest·기존 공개키 signature/변조 대조군·Mac0.8.7/arm64/ICNS/codesign/Gatekeeper/stapler·Windows 외부7frame icon·DMG checksum을 통과했다. [공개 원장](../assets/task_m100_hp112_release/desktop-v087-public-verification.json)의 공개 시각2026-10-02T02:39:24Z과 익명 앱 endpoint0.8.7을 확인했다.
- 설치/재시작·Windows GUI는 미실행이다. 원본39파일은 unchanged이며, 운영 commit push 뒤 local devel CAS·Issue112 종료·소유 임시 대상 정리를 진행한다. 보호 worktree와 공유 cache/fork branch는 소유·앱 archive 실제 결과를 따른다.
