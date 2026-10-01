# Task #110 — upstream/devel 통합 수행계획

- Issue: [paldyn/HanPage#110](https://github.com/paldyn/HanPage/issues/110)
- 사용자 승인: 2026-10-01 "합치자. 우리가 추가한 기능 하고 겹치는게 있으면 말해주고"
- 브랜치: `codex/upstream-sync-20261001`
- HanPage 기준: `cecaf1bbfec9a10778484d30ebb747280e7a02af`
- 지난 upstream 고정점: `cf5d462dcda1b5ab71160033e1d454b42198ad18`
- 신규 upstream 고정점: `02530b9ed567a44663edb26c65fb565c4a79f00d` (0.8.6)

## 통합 방식

기존 HanPage 이력은 Task #51에서 pdf-large를 제거하며 재작성됐다. 원본 upstream 이력을 직접 merge하면 그 과거 이력을 다시 수용한다. 따라서 origin/devel을 부모로 유지하는 통합 브랜치에서 최신 upstream 파일 트리를 가져오고, 원본 고정점 대비 HanPage 델타만 3-way 이식한다. 일반 PR로 devel에 반영하며 force-push하지 않는다. upstream SHA와 소유 델타 목록을 남겨 다음 동기화의 기준으로 쓴다.

## 단계

1. 계획 및 소유 델타 확정: 원본 고정점 대비 158경로 중 HanPage 추가 97, 양쪽 수정 24, upstream 무변경 14, upstream 삭제/로컬 수정 4, 의도 삭제 19를 분류한다.
2. 최신 트리 수용 및 기능 이식: 엔진·테스트는 upstream 그대로 유지하고 Desktop, 네이티브 파일 I/O·큐, 폰트 타임아웃, updater, 다운로드 버튼, PWA·브랜드·배포 설정을 이식한다. upstream 삭제 파일은 되살리지 않으며 HanPage 문서는 archive에 보존한다.
3. 검증: Studio TypeScript·테스트·웹/데스크톱 빌드, npm/editor 계약, Desktop Rust check, WASM fresh build, 실브라우저 파일 열기·폰트 영구 pending·저장/업데이트 스모크를 수행한다. 전체 CI 성격의 긴 로컬 회귀는 focused 결과를 공유한 뒤 별도 승인에 따른다.
4. 결과 보고 및 통합: 중복 기능/계약 변경·검증 결과·PR 본문을 완성한다. PR 생성은 저장소의 별도 승인 규칙을 적용하고 최신 CI 통과 뒤 승인된 merge를 수행한다. 앱 배포와 Dependabot close는 별도로 기록한다.

## 이미 확인한 겹침

- upstream `rhwp-desk`는 Windows 에이전트 명령/검증 도구다. HanPage의 Studio 편집기 Desktop shell을 대체하지 않는다.
- upstream host-font-provider API·새 폰트 registry는 수용하되 FontFace 영구 pending 보호는 계속 필요하다.
- upstream 저장은 내용 손실 보고/암호 상태/최근 문서 갱신 계약이 강화됐다. Desktop 저장도 같은 파이프라인에 연결한다.
- `.gitattributes`의 pdf-large LFS 제거는 upstream에도 반영됐다. 최신 파일을 그대로 수용한다.
- Dependabot 26개에는 upstream에 이미 반영된 범위가 있으나 desktop-release의 action 변경은 별도로 남을 수 있다.

## 절차 선택

base route: maintainer_general (HanPage admin, 통합 작업)

modifiers: local_validation, visual_fixture_evidence, multi_pr_update_branch, rework_and_exceptions, post_merge

loaded documents: pr_review_workflow.md, pr_review/README.md 및 위 기본·보조 문서, docs_and_git_workflow.md, dev_environment_guide.md, visual_verification_governance.md, visual_sweep_guide.md

current head: 위 기준 SHA는 이번 통합에 고정한 참고값이다. 최종 PR head의 CI와 merge 가능 여부는 별도 확인한다.
