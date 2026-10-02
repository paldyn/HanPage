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

## 0.8.6 초안 실패 후 0.8.7 보정 계획

PR #113은 최신32checks(28SUCCESS·4정상skip)에서 검증됐고 d523c04493ddf8296bc5d56e2c90c38f4da4fe96으로 병합됐다. Desktop tag hanpage-desktop-v0.8.6 run36948944525는 양 플랫폼 WASM 도구가0.9.1로 설치돼 --no-opt를 거부하며 컴파일 전에 실패했다. 초안에는 자산이 없고 기존 공개0.8.3은 유지한다.

사용자의 앱 릴리스 승인 범위에서 실제 실패를 보정한다. 기존 태그는 옮기지 않고 Desktop0.8.7로 패치 버전을 높인다. 엔진·Studio·로고·런타임 기능은 PR #113과 동일하게 유지한다. wasm-pack-action에 공식 배포가 확인된 v0.13.1을 명시하고 실행 버전을 검사한다. Windows는 기존 native PowerShell locked wrapper, macOS는 shell locked wrapper를 사용한다.

변경은 최신 origin/devel d523c04493ddf8296bc5d56e2c90c38f4da4fe96 기준의 별도 준비 PR에 담는다. 로컬 fresh locked WASM·Desktop 빌드, 기존 workflow 계약 및 YAML을 확인한 후 번호를 발급받고 archive self-review·오늘할일을 trailing commit에 포함한다. 최신 head CI를 확인하고 정상 병합한 후 새 Desktop0.8.7 tag로 초안 두 플랫폼 빌드를 실행한다. 설치 파일·updater 서명·macOS 공증 검증 후에만 공개한다.

원 작업공간의 원본39파일을 stash/ff/drop하는 동기화는 자동 승인 검토에서 원본 유실 위험으로 거절됐다. 해당 명령은 실행되지 않았고 원 작업공간과39해시는 그대로 보존한다. 안전한 격리 작업트리에서 릴리스 작업을 계속하며 이 동기화를 우회하지 않는다. GitHub 댓글·main/Pages/CLI/npm/확장·secret/권한 변경은 범위에 포함하지 않는다.
