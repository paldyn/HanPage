# Task #112 — HanPage Desktop 0.8.6 새 로고·통합 기능 배포

- 이슈: [HanPage #112](https://github.com/paldyn/HanPage/issues/112)
- 사용자 승인: 2026-10-02 “응 하자” — 새 로고와 통합 기능을 포함한 앱 버전 상승 및 실제 릴리스.
- 기준: origin/devel `c5b6022532371fd07a2f5e978fb21e4c3cc37758`; 통합 엔진/Studio 0.8.6, 공개 Desktop 0.8.3.
- 역할/라우팅: ADMIN 실행자의 본인 PR 준비이므로 collaborator self-merge 절차(관리자 우회 없음).
- base route: collaborator_self_merge.md
- modifiers: intake_and_review.md, review_template.md, local_validation.md, post_merge.md
- loaded documents: CLAUDE.md, mydocs/README.md, manual/README.md, docs_and_git_workflow.md, pr_review_workflow.md, pr_review/README.md 및 위 자식 문서, dev_environment_guide.md, publish_guide.md, github_operations.md.

## 범위와 단계

1. 현재 사용자 로고 39파일을 hash로 고정하고 청결한 기존 검토 작업트리에 복사한다. 원 작업공간의 파일은 변경하지 않는다.
2. Desktop package/tauri/Cargo 및 독립 lock 버전을 0.8.6으로 맞춘다. 엔진과 Studio source는 이미 #111에서 검증됐고 이 작업에서 변경하지 않는다.
3. 실제 아이콘 포맷/크기/연결, locked Desktop 컴파일, fresh Desktop frontend bundle을 검증한다. 릴리스 workflow에 새 구조 호환 수정이 필요하면 그 명령·계약도 검증한다.
4. devel PR에 후보·검증·self-review를 포함하고 최신 head CI 통과 뒤 병합한다.
5. 검증된 병합 commit에 `hanpage-desktop-v0.8.6` 태그를 push하여 Desktop Release만 게시한다. macOS aarch64/Windows x64와 updater 서명·macOS 공증을 확인한다.
6. 공개 latest.json, 자산 다운로드/서명, 실제 macOS 번들 버전·아이콘·공증을 검증하고 최종 보고한다.

## 배포 경계와 복구

O4 승인 대상은 HanPage Desktop 릴리스이다. main 승격/Pages·엔진 CLI·npm·확장 배포는 이번 목표에 필요하지 않으므로 Desktop 전용 tag가 devel의 검증된 commit을 가리킨다. 실제 workflow는 이 태그를 독립 트리거로 사용한다.

기존 공개 0.8.3 릴리스를 보존한다. 실패 시 불완전한 후보를 완료로 보고하지 않고 원인과 run을 보존해 준비 PR에서 보정한다. 게시 후 결함이 확인되면 설치 사용자를 버전 역행시킬 수 없으므로 같은 태그/자산 덮어쓰기보다 더 높은 패치 버전으로 수정 배포한다. secret/권한 변경은 이 범위에 포함하지 않는다.

## 완료 전까지 남은 검증

로컬 및 GitHub 빌드/릴리스는 아직 실행 전이다. 알려진 Linux native SVG 호환 PDF 글꼴 문구 누락(#111)은 별도 문제이며 Desktop 인쇄가 같은 경로라고 주장하지 않는다. OS별 자동 업데이트 설치 실측 여부는 최종 기록에서 별도로 명시한다.
