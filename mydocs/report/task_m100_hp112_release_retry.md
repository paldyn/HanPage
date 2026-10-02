# Task #112 — Desktop0.8.7 빌드 보정 검증

## 문제와 변경

Desktop0.8.6 초안 run36948944525는 양 플랫폼 모두 wasm-pack0.9.1이 --no-opt를 거부해 실패했다. 공개0.8.3은 유지됐고 초안 자산은0개다. source549411762fecb5a833de3e30d3e82d3aab8319f2에서 v0.13.1을 명시하고 실행 버전을 검사한다. Windows는 기존 PowerShell cargo.exe proxy wrapper, macOS는 기존 shell wrapper로 metadata/build 잠금을 유지한다. 기존 태그를 옮기지 않고 Desktop0.8.7로 빌드한다.

Desktop6버전 필드만0.8.6→0.8.7로 갱신했고 앱 의존성·root엔진/Studio source·37개 로고 바이너리/원본팩/백업은 그대로다. README의 사용자 원문도 보존한다. 관련 [계획](../plans/task_m100_hp112.md), [첫 배포 실패](../pr/assets/task_m100_hp112_release/desktop-v086-failed-release.json), [Windows wrapper 계약](../pr/assets/task_m100_hp112_release/desktop-v087-windows-wasm-contract.json).

## 실제 로컬 검증

[원장](../pr/assets/task_m100_hp112_release/desktop-v087-local-validation.json)에 source/base·명령 환경·실제 산출물 SHA와 raw 로그 해시를 보존했다. 기존 계약9/9·YAML/버전/의존성 검사 통과.

- CARGO_TARGET_DIR=target/pr-review, wasm-pack0.13.1: scripts/wasm-pack-locked.sh --target web --release --no-opt --out-dir <owned output/pkg-v087> 성공. 실제 WASM compile2m42s.
- Node22.22.1, npm run build -- --no-bundle --ci -- --locked 성공. frontend tsc/Vite와 native rhwp-desktop0.8.7 release compile44.29s.
- tauri bundle --bundles app --no-sign --ci 및 createUpdaterArtifacts=false override 성공. 로컬 .app Info.plist0.8.7/com.paldyn.hanpage, 실제 ICNS가 source byte 동일.
- fresh WASM이 Desktop dist에 byte 동일하게 포함됐고 sw.js는 없다. 빌드 전후 root/독립 Cargo.lock이 바뀌지 않았다.

Cargo 검증은 같은 shared target에서 순서대로 실행했다. 로컬 unsigned 번들이므로 실제 updater 설치·코드서명·공증·Windows GUI 성공을 주장하지 않는다. 원 작업공간39파일은 hash 그대로 보존됐고 자동 승인 검토가 거절한 stash/ff/drop을 우회하지 않았다.

## PR 및 공개 조건

사용자 “응 하자” 승인 범위에서 후속 준비 PR을 생성해 self-review/오늘할일을 trailing head에 포함한다. 최신 head CI와 exact head/CLEAN/MERGEABLE을 확인한 뒤 정상 merge한다. 엔진/renderer 변경이 없어 #111의 알려진 한계를 유지하며 별도 시각 출력 개선을 주장하지 않는다. 앱0.8.7 초안의 macOS·Windows 실제 빌드, 4플랫폼 latest.json과 기존 공개키 updater payload 서명, macOS codesign/Gatekeeper/stapler 및 실제 새 아이콘을 확인한 뒤에만 공개한다. main/Pages/CLI/npm/확장·secret/권한·댓글 변경 없음.
