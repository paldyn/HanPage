# PR #111 implementation — upstream snapshot 수용 단계

## 승인과 고정 기준

2026-10-01 사용자 “승인”으로 전체 검증·remote push·Open PR·최신 CI 뒤 merge를 승인받았다. primary의 별도 Desktop 변경은 사용자가 소유하며 통합 범위에서 제외했다. GitHub 댓글·수동 issue close·앱 릴리스는 별도 승인 대상이다. [리뷰](pr_111_review.md), [Task 계획](../../plans/task_m100_hp110.md), [보고서](../../report/task_m100_hp110_report.md)를 따른다.

base cecaf1bbfec9a10778484d30ebb747280e7a02af; pinned upstream 02530b9ed567a44663edb26c65fb565c4a79f00d. isolated worktree는 /Users/lwm/.codex/worktrees/upstream-sync-final-20261001/HanPage이며 owned clean worktree다. shared primary와 target/pr-review는 삭제 대상이 아니다.

## 실제 커밋·단계

| SHA | 내용·상태 |
| --- | --- |
| a55d362195b15a2d38e961b8974ce97e3a72d0cf | 계획·소유 델타 분류 |
| b6f4318dc8c8c50cdb0897887f72b3e59684dee8 | upstream source/fixture 수용 및 HanPage 기능 이식 |
| 3790cb0d7e35554a64e76af47b9c6e1be00d2d42 | upstream 기준 PDF 수용 |
| 48404b29594806b38a3a70a2b6abc3b697a83989 | upstream 문서 수용 |
| 33386f1c09f65e6adb9462e5177acf6bbce7d911 | upstream 추적 ignore 대상 보존 |
| 39a68841cce40c348cf95be77fc5d3fe876c5308 | CI 정책 exact old base bootstrap |
| 88317b1110432d37a61b08a1af3ed305dd56e9c0 | focused·browser 결과 및 승인 경계 기록 |
| c973306239ea6e93efac5f4d3cb0e28b3cf9cb71 | 사용자 승인 및 실제 편집 HWP 커밋 |
| b2034762f640bdf66721f52a7cda6c31cb8282e8 | genuine-hidden 세 입력 분류의 exact hash·whole finding 보호 |
| 27666e01a99cfbf186640bc9c21e83a14d3cd1e9 | 원본 XML·Native SVG·PNG 및 sample manifest |
| c38538745d7350efdf3ec9cd2ed64dda01848c66 | missing pinned commit filtered fetch 보정, 전체검증 source |
| dc3f47079120c64a89565b927b2d516c3a5ff30f | 완료한 full regression·Native 및 입력 근거 기록 |
| befaca4650dc3b1611b4b654a3554adfb3f69eeb | OPEN PR에서 Pages workflow mirror 계약 보정; focused41/41·관련200/200 PASS |
| 580a1e6f386052ad64bbfa3e8bef7887fb21a557 | 좁은 화면의 다운로드 버튼 보존·responsive 문서 준비 경합 보정; responsive2666/2666·단위22/22·tsc PASS |

1. 소스·PDF·문서·최종 기록을 pack당2GiB 미만의 네 번 순차 push로 전송했다. PR #111 번호는 생성 API 결과로 확정했다.
2. 번호 기반 review/impl 및 대표 시각 근거를 같은 PR의 docs tail로 반영한다. source와 입력 기존 blob 동일성이 유지되면 이미 완료한 Cargo 게이트를 반복하지 않는다.
3. 최신 exact head의 모든 생성된 Actions check를 확인한다. CI·CodeQL·Render Diff·Adapter·Proptest·skill 적용 check는 success 또는 조건에 따른 정상 skip이어야 한다. fail/cancel/미완료를 무시하지 않는다. Gym은 path-filter 상한과 default main 미등록으로 자동/dispatch 실행이 없어 동일 계약22모듈 로컬2124 PASS/1skip을 별도 기록한다. Render Diff는 최종 head를 지정해 기본 PR 범위로 dispatch하고 availability·readiness·actual artifacts를 확인한다.
4. PR head/base·mergeability를 재조회하고 --match-head-commit으로 merge한다. admin bypass를 사용하지 않는다.
5. 실제 merge SHA와 최신 origin/devel 포함을 확인한다. 병합 뒤 duration refresh만 관찰하고 전체 검증 CI를 다시 시작하지 않는다.
6. maintainer 직접 반영: archive review/impl·오늘할일·review assets 운영 기록만 한 commit으로 반영한다. source/test/workflow/golden/sample 수정은 이 단계에 넣지 않는다. issue 자동 close 상태를 확인하되 승인 없는 댓글·수동 close는 하지 않는다.
7. primary의 현재 HEAD·staged/unstaged hash·user Desktop bytes를 다시 확인한 뒤 안전한 ff-only가 가능한 경우 동기화한다. 사용자 변경과 shared target을 보존하고 clean owned worktree 및 unused owned output만 정리한다. 기본 작업공간이 dirty이면 PR 전용 remote/local branch는 규칙상 삭제하지 않고 이유를 남긴다.

## 보정과 rollback 범위

로컬 초기 전체 nextest 실패 한 건은 원본 corpus 분류와 실제 hidden-positive 세 입력이 달랐기 때문이다. detector 변경이나 corpus 전체 skip 대신 경로·hash·전체 finding 보호로 분류했다. fixture/원본 XML/실제 출력은 커밋한 자료에 고정했다. full suite10,276 및 focused6 통과했다.

CI가 실패하면 실제 실패 로그의 원인에 한정해 보정하고 최신 head 검사를 새로 확인한다. workflow/source 수정이면 관련 로컬 검증을 수행한다. 이미 merged한 tree를 되돌릴 필요가 생기면 최신 devel 기반 별도 branch/PR에서 실제 merge SHA에 git revert -m1을 적용하며 reset·force push·사용자 작업 삭제는 하지 않는다. rollback은 현재 통합 범위에 포함하지 않았다.

## 처리 완료

최종 PR head `f9f2e47bf43d87fff7e1faabcdac361c7091be12`는9299f56de의review/asset docs tail과f9f2e47bf의문구/영구증적경로보정을포함한다. 실제 merge SHA `b57d9ae5e77bf3f10f76ced17e40dc7c47673930`. 최신 CI/CodeQL/Render Diff gate를확인한뒤사용자승인범위에서병합했다. primary ff-only동기화·Desktop39변경보존·clean managedworktree apparchive는pinned task/workspace보호로거절돼유지했다. local/remote branch는primaryDesktopdirty때문에보존한다. [최종운영증적](../assets/task_m100_hp110_final_ci/artifact-map.json)과 [오늘할일](../../orders/20261001.md)을따른다. Duration갱신은근거부족보류를별도기록하고검증CI를재시작하지않았다. Linux SVG호환PDF문구누락/실제OSupdater/중복PR정리/main승격정책은남은별도범위다.
