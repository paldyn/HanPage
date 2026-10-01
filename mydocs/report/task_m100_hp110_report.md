# Task #110 — upstream 통합 후보 검증 기록

- 이슈: [HanPage #110](https://github.com/paldyn/HanPage/issues/110)
- 계획: [task_m100_hp110](../plans/task_m100_hp110.md)
- 브랜치: `codex/upstream-sync-20261001`
- 기존 HanPage: `cecaf1bbfec9a10778484d30ebb747280e7a02af`
- 가져온 upstream/devel: `02530b9ed567a44663edb26c65fb565c4a79f00d`
- 상태: 로컬 통합 후보 및 focused·브라우저 검증 완료. 2026-10-01 사용자의 "승인"에 따라 전체 Rust 게이트 실행 중이며, 통과 뒤 remote push·Open PR·최신 CI 확인·merge를 진행한다.

## 통합과 기능 겹침

Task #51의 pdf-large 제거 이력을 보존하기 위해 원본 Git history 대신 최신 upstream tree를 수용했다. 기존 origin/devel을 부모로 두고 HanPage 델타만 이식했다. 비교 원본과 158개 소유 델타는 [ownership JSON](../working/task_m100_hp110_ownership.json), 적용 분류는 [sync manifest](../working/task_m100_hp110_sync_manifest.json)에 보존했다.

| 겹치는 범위 | 통합 결과 |
| --- | --- |
| upstream `rhwp-desk`와 HanPage Desktop | upstream 도구는 Windows 에이전트 CLI/검증 workbench다. Studio 편집기인 HanPage-Desktop과 용도가 달라 둘 다 보존했다. |
| host-font-provider, OS 폰트 탐지, CJK registry | 최신 upstream API·폰트 목록을 수용했다. 개별 5초·foreground 8초 timeout은 upstream에 없어 이식했다. 예산 이후 나머지 배치를 background로 계속 처리한다. |
| 파일 저장·내용 손실 안내·암호·최근 문서 | native 저장을 upstream reported-export 파이프라인에 연결했다. native 성공 뒤에만 파일명·dirty·최근 문서·손실 안내를 갱신한다. 취소는 dirty를 유지하고 실패는 오류로 반환한다. |
| 초기 새 문서와 native 파일 연결 큐 | 초기 native 큐 처리가 끝난 뒤 새 문서 fallback을 실행한다. 구독 중 발생한 이벤트가 먼저 큐를 가져가는 race를 재현하고 drain을 직렬화했다. |
| 창 제목·About 국제화 | 최신 문서 제목 상태 관리와 locale 처리를 사용하면서 HanPage 브랜드를 유지했다. |
| native I/O·updater·웹 다운로드 버튼 | upstream에 대응하는 Studio 구현이 없어 HanPage 기능을 이식했다. 웹/desktop 빌드의 PWA 분리, CNAME, 아이콘, 서명·업데이트 설정도 보존했다. |
| pdf-large LFS 제거 | upstream 최신 tree도 제거되어 최신 .gitattributes를 사용했다. 과거 대형 파일 이력은 되살리지 않았다. |

Rust 엔진 `src/`, `tests/`, `crates/`, `Cargo.lock`의 upstream 대비 diff는 0이다. 앱 버전은 HanPage Desktop 0.8.3을 유지하고 엔진·Studio는 upstream 0.8.6으로 갱신했다. 이 통합은 devel 대상이며 앱 배포·릴리스 태그 생성은 포함하지 않는다.

## 실행한 검증

명령 로그는 ignored `output/pr-review/hp110/logs/`에 보존한다. 아래 결과는 실제 실행한 범위만 뜻한다.

| 검사 | 결과 / 증거 |
| --- | --- |
| Studio 전체 단위 테스트 | 최종 오류 처리 보정 뒤 1,830 통과, 2 skip, 실패 0. `studio-tests.log`. |
| 폰트 focused | 관련 8개 파일 41 통과. 새 timeout 테스트 2개는 이전 구현에서 실패·수정 후 통과. `font-focused.log`. |
| native queue race | 수정 전 새 테스트 1 실패, 수정 후 3/3 통과. `desktop-bridge-before.log`, `desktop-bridge-after.log`. |
| native save / 암호 / 내용 손실 / updater focused | 31 통과. `desktop-focused.log`. |
| npm/editor | 32 통과, type check·pack dry-run 성공. `editor-tests.log`, `editor-types.log`, `editor-pack.json`. |
| fresh WASM | locked wrapper 빌드 성공. `wasm-build.log`. pkg와 Studio public의 SHA-256 모두 `5c7a9a16c22cc6e0086d14d38df0386c5e63a1c75fe194a49848111c95011d12`; 최신 upstream public bundle과도 diff 0. |
| Studio TypeScript / WASM binding contract | TypeScript 성공, binding 및 bridge 6/6 통과. `studio-tsc.log`, `frontend-bindings.log`. |
| 웹 / Desktop frontend 빌드 | 모두 성공. 웹 PWA 생성, Desktop SW 미생성 확인. `studio-web-build.log`, `studio-desktop-build.log`. |
| Desktop Rust | `cargo check --locked --target-dir target/pr-review` 성공. `desktop-cargo-check.log`. |
| Pages workflow 계약 | 28/28 통과. `pages-workflow-tests.log`. |
| 문서 이동 | 보존한 HanPage archive 및 신규 계획·오늘할일 링크 5개 파일 검사 통과. |
| 실제 Chrome / fresh WASM | 웹 시작, native 큐 선행 race, HWP 3쪽 열기, 16,896바이트 native 저장, 취소 시 dirty 유지, native 다시 열기, 준비된 업데이트 버튼 모두 성공; pageerror 0. `browser-smoke.log`, `browser-smoke.json`, `web-start.png`, `native-start.png`. |
| 실제 Chrome / 문서 제목 | 손상 파일 실패 시 기존 문서·제목 보존, 보조 WASM bridge의 제목 불변, HWP 열기·HWPX 다른 이름 저장·host 저장 통지·새 문서에서 HanPage 제목 갱신 성공. `document-title-browser.log`. |
| native 열기 오류 | native reject(Error/Tauri string), 취소, dirty guard, 성공 payload와 save·queue 관련 focused 12/12 통과. ko/en 오류 안내를 기존 picker 처리에 연결했다. |
| native 열기 음성대조 | 격리 사본에서 try/catch 제거 시 읽기 거부 test가 미처리 rejection으로 실패, native 분기 제거 시 오류·취소·성공 전달 3개 실패. 현재 helper 6/6 통과, 원본 hash 불변. `native-open-negative-*.log`, `native-open-negative-summary.json`. |
| 실제 Chrome / 폰트 영구 pending | 다중 배치 foreground 8,015.7ms 반환, 나머지 배치 background 실행·foreground progress 중단. 기본 문서 준비 13.8초(초기화·새 문서 각각 약 5초), HWP 업로드·키보드 편집 5.904초. 실제 Chromium HWP 다운로드와 재열기에서 입력 문구 보존, 3쪽·편집 가능·dirty=false·pageerror 0. `font-pending-browser.log`. |

Chrome에서 Tauri 명령·이벤트는 대역을 사용했다. 실제 WASM·Canvas·메뉴·파일 저장 흐름은 실행했지만 OS 파일 연결, native 대화상자, 실제 업데이트 설치·서명·공증은 실행하지 않았다. 위 캡처는 통합 UI 스모크이며 전체 renderer의 한컴 PDF fidelity 검증을 뜻하지 않는다.

원격에서도 열어 볼 수 있는 대표 근거는 [브라우저 결과 JSON](../pr/assets/task_m100_hp110_browser_checks.json), [native 파일 연결 화면](../pr/assets/task_m100_hp110_native-start.png), [폰트 pending 상태에서 저장 후 재열기 화면](../pr/assets/task_m100_hp110_font-pending-hwp-reopened.png)에 보존했다. 실제 다운로드 입력은 [저장 후 편집 문구가 포함된 HWP](../pr/assets/task_m100_hp110_font-pending-edited.hwp)에 커밋하여 재현 입력으로 보존했다. 다운로드 HWP의 SHA-256은 `6dc2a8a953d85b693d385ea0e6b06539b21f0249ba31e8e24463a13806a8e4fd`이며 저장·재열기 파일이 같다.

## CI 및 전송 준비

기존 origin/devel에는 최신 suite/unit-tier 정책이 없어 그대로 비교하면 CI가 실패한다. 이번 정확한 old base·동일 HanPage 저장소·통합 branch에만 적용되는 bootstrap을 추가했다. pinned upstream SHA와 Rust/test/policy 입력 tree 동일성을 검증한 경우에만, 임시 sparse worktree에서 upstream 원래 prepare·manifest·unit-tier 검사를 실행한다. current HEAD로 비교 기준을 바꾸거나 정책 threshold를 완화하지 않는다.

실제 Git identity·tree 동일성 검사와 pinned sparse worktree의 prepare·두 policy check가 통과했다. `policy-bootstrap-identity.log`, `policy-bootstrap-worktree.log`. focused Node 30/30도 통과했다(`policy-bootstrap-focused.log`). 임시 worktree는 정리했다. 정책 보정은 `ci.yml`, 신규 bootstrap helper·Node test 및 기존 manifest test의 호출 경로 assertion에 한정되며 canonical policy와 JSON threshold는 upstream 그대로다.

기존 main에는 CI Controller가 아직 없어 상태 부재 시 consumer가 전체 검사로 fallback한다. 향후 devel→main 릴리스에는 upstream 전용 promotion repository 제한을 HanPage에 맞추는 별도 작업이 필요하다.

전체 snapshot의 thin pack 추정은 2,895,192,237바이트(2.696 GiB)로 단일 push 상한 2 GiB를 넘는다. 소스·fixture, mydocs, PDF의 중간 커밋으로 나누고 각 pack을 측정한 뒤 같은 통합 branch를 순차 push한다. 최종 tree는 하나의 통합 후보로 PR에 제출한다.

첫 소스 커밋 `b6f4318dc8c8c50cdb0897887f72b3e59684dee8`의 실제 thin pack은 337,339,571바이트(0.314 GiB)로 상한 이하다. HanPage 이식 파일의 upstream 대비 `git diff --check`는 통과했다. 전체 imported tree에는 upstream의 CRLF fixture 등 기존 whitespace 경고가 있으며, 이 통합에서 원본 fixture를 일괄 수정하지 않았다.

PDF 커밋 `3790cb0d7e35554a64e76af47b9c6e1be00d2d42`의 앞 커밋 대비 thin pack도 688,631,403바이트(0.641 GiB)로 상한 이하다.

문서 커밋 `48404b29594806b38a3a70a2b6abc3b697a83989`의 pack은 1,110,233,641바이트(1.034 GiB), upstream 추적 기록 보완 `33386f1c09f65e6adb9462e5177acf6bbce7d911`은 982,379바이트, CI 커밋 `39a68841cce40c348cf95be77fc5d3fe876c5308`은 1,375,812바이트다. 모두 단일 push 상한 이하다. 사용자 승인을 받았으며 전체 로컬 게이트가 끝나면 각 커밋을 순서대로 push한다.

최종 경로 대조: upstream 37,300경로 전부 후보에 존재하고, HanPage 추가 113경로를 포함해 37,413경로다. 원본에서 이미 추적하던 ignore 대상 `.log` 143경로는 정확한 upstream 목록만 명시 staging하여 보존했다. 이번 로컬 검증 로그는 ignored output에 있다.

## 승인 및 남은 후속

- [Internal Task PR Approval](../manual/codex/docs_and_git_workflow.md#internal-task-pr-approval)에 따라 focused·browser 결과 공유 뒤 2026-10-01 사용자 "승인"으로 전체 Rust lint/회귀와 remote push·Open PR 생성·최신 CI 통과 뒤 merge를 승인받았다.
- 최종 head에서 필수 로컬 게이트 및 GitHub CI를 확인한 뒤 사용자가 요청한 merge를 수행한다.
- Dependabot 29개 중 26개에 upstream에서 이미 수용한 범위가 있다. #52/#55/#58은 Desktop workflow 변경도 있어 전부 대체된 것은 아니다. #56 Tauri·#30 cross-env는 Desktop 별도 범위다. 중복 PR의 close는 이 작업에서 실행하지 않았다.

승인 뒤 primary 작업공간에 별도 Desktop 아이콘·설정 변경이 생겼으므로 최종 통합 커밋은 `88317b1110432d37a61b08a1af3ed305dd56e9c0` 기준의 격리 worktree에서 기록한다. primary의 사용자 변경은 staging·되돌림 없이 보존한다. 검증 source와 격리 후보의 엔진·Studio·workflow blob 동일성을 최종 제출 전에 확인한다.
