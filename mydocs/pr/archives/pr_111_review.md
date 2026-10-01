# PR #111 리뷰 — 최신 upstream 통합과 HanPage 기능 보존

## 최종 판정

**승인 — 최신 head CI·CodeQL·Render Diff 통과 후 병합 완료.** final head `f9f2e47bf43d87fff7e1faabcdac361c7091be12`, merge SHA `b57d9ae5e77bf3f10f76ced17e40dc7c47673930`. 로컬 Rust10,276·responsive2,666 통과를 유지하며 actual GitHub 검사와 artifact를 별도 확인했다. [최종 CI/병합 증적](../assets/task_m100_hp110_final_ci/final-ci-audit.json).

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#111](https://github.com/paldyn/HanPage/pull/111) / enxec / devel |
| route·reviewer | maintainer_general; 작성자 enxec의 self-review와 독립 subagent 점검. 본인에게 GitHub reviewer assign은 적용하지 않았다. |
| 기존 base | cecaf1bbfec9a10778484d30ebb747280e7a02af |
| 수용한 upstream | 02530b9ed567a44663edb26c65fb565c4a79f00d (0.8.6) |
| 최종 source head | 580a1e6f386052ad64bbfa3e8bef7887fb21a557; 이후 review 문서 commit은 source 변경 없는 tail이다. |
| 검증 source | Rust: c38538745d7350efdf3ec9cd2ed64dda01848c66; 보안 분류 보정 b2034762f640bdf66721f52a7cda6c31cb8282e8; Pages mirror befaca4650dc3b1611b4b654a3554adfb3f69eeb; Studio responsive 580a1e6f386052ad64bbfa3e8bef7887fb21a557 |
| 규모·상태 | 초기 제출 dc3f47079의 local Git diff: 28,065 files / +6,875,409 / -161,971; API 초기값은 0으로 계산 중. 초기 head: Open / MERGEABLE / UNSTABLE(최초 CI 실패); 과거 참고값이며 merge 전 최신 head와 상태를 재확인한다. |
| 이슈·승인 | [#110](https://github.com/paldyn/HanPage/issues/110), Closes #110. 2026-10-01 사용자 “승인”: 전체 Rust 검증·push·Open PR·최신 CI 통과 뒤 merge. 댓글·issue 수동 close·앱 릴리스는 범위 밖. |

## 변경과 검토 범위

구체적인 이식·겹침은 [Task #110 보고서](../../report/task_m100_hp110_report.md)와 [158개 소유 델타](../../working/task_m100_hp110_ownership.json)에 기록했다. 기존 fork의 pdf-large 제거 이력을 유지하기 위해 old origin/devel 위에 최신 upstream 파일 트리를 수용했다. 기존 upstream Git ancestry를 merge하는 방식 대신 snapshot을 수용했으며 force push는 없다.

Rust src/crates/Cargo.lock과 renderer/layout은 pinned upstream과 동일하다. HanPage가 별도로 고친 엔진 geometry는 없고 tests는 genuine-hidden 입력 세 개의 분류를 보호한 한 파일만 다르다. Studio에서는 최신 font registry·reported export·문서 제목 API에 HanPage timeout·native I/O·파일 큐·updater·브랜드를 연결했다. Desktop 0.8.3 설정은 유지했다. 기본 작업공간의 별도 사용자 Desktop 아이콘·설정 변경은 이 PR에 포함하지 않았다.

Dependabot 29개 중 26개에 upstream이 수용한 범위가 있다. #52/#55/#58은 Desktop workflow 범위가 남고 #56 Tauri/#30 cross-env는 Desktop 별도 범위다. 중복 PR은 자동 close하거나 일괄 merge하지 않았다.

## 검증 입력과 커밋 확인

판정: **충족**. 사용한 원본 HWP/HWPX/PDF는 이미 upstream tree에서 수용하거나 이 branch에 커밋한 파일이며 임시 다운로드만을 입력 증거로 세지 않았다. [1,154개 sample 전체 해시 및 신규/수정 479개 manifest](../assets/task_m100_hp110_input_manifest.json)에 경로·Git blob·SHA-256·출처 commit을 고정했다. 실제 사용 bytes와 committed bytes를 대조했다. 기준 PDF는 imported commit 3790cb0d7e35554a64e76af47b9c6e1be00d2d42에 포함된다.

- samples/para001.hwp: SHA-256 bab4561ceb02cdfa184a1689be9619c08e18d6021cdbc423486b848bc14d267e; upstream source, HWP 열기·native 저장·재열기 및 제목 검증에 사용했다.
- [폰트 pending 상태에서 실제 편집·저장한 HWP](../assets/task_m100_hp110_font-pending-edited.hwp): SHA-256 6dc2a8a953d85b693d385ea0e6b06539b21f0249ba31e8e24463a13806a8e4fd, c973306239ea6e93efac5f4d3cb0e28b3cf9cb71에 커밋. committed 경로에서 다시 열어 text·3쪽·편집 가능·dirty=false를 확인했다.
- issue4690/30098_indent_over_stored_cs.hwp 및 issue6086/30098_resident_registration_reform.hwp: SHA-256 de4d89bd8803bd3c8cc84b7e51ed36846af7cc938a97baf6fb458fc0b0eeb474. 기존 genuine-hidden issue6524/30098_float_host_split_lineseg.hwp와 byte 동일하다. p14의 흰 “장”과 전체 finding을 검증했다.
- issue5723/coanchored_square_pair_center_slack.hwpx: SHA-256 a15ef3aa092febb9540e68f9bae206a8a2b7f7a613b8b13eca65e3fa67126aea. p1의 흰 “기준”과 전체 finding을 XML·Native SVG·PNG에서 직접 확인했다. [근거](../assets/task_m100_hp110_security_genuine/genuine-hidden-evidence.json).

clipping 원장의 외부 참조 92건은 파일이 없어 미검증이며 통과로 세지 않는다. 일반 코퍼스 parser fail skip의 범위도 테스트의 기존 계약으로 남았다. 초기 실행의 hidden/injection/unicode 출력은 각각 479개 검사를 확인했다.

## 실행한 검증

실제 명령과 시간·대상은 [검증 JSON](../assets/task_m100_hp110_validation.json)에 고정했다. source와 fixture blob은 검증 primary와 isolated 후보가 같았다. Cargo 명령은 shared target/pr-review에서 순차 실행했다.

| 명령·검사 | 실제 결과 |
| --- | --- |
| cargo fmt --all / cargo fmt --all --check | 통과 |
| cargo clippy --locked --target-dir target/pr-review -- -D warnings, WASM clippy, workspace/all-target clippy | native·WASM·workspace 통과; 보안 test 보정 뒤 workspace/all-target 검사도 다시 통과 |
| cargo build --locked --target-dir target/pr-review --workspace | 통과 |
| cargo nextest run --locked --cargo-profile release-test --target-dir target/pr-review --tests --test-threads 10 --no-fail-fast | 10,276/10,276 PASS, 50 skip, 0 fail; test 261.762초 |
| 보안 corpus focused (prepare 후) | 6/6 PASS. 첫 전체 실행 10,275 PASS/1 FAIL은 genuine-hidden 세 입력의 분류 오류였고 원본 detector는 바꾸지 않았다. prepare 누락 0건 실행 시도는 실패로 기록하고 통과로 세지 않았다. |
| cargo test --locked --profile release-test --target-dir target/pr-review --features native-skia --lib -- --test-threads 10 | rhwp 3,930 PASS/13 ignored, dependency lib 182 PASS |
| native-skia issue_2225_missing_picture_placeholder / render_p37_direct_pdf_export | 2/2 PASS / 4/4 PASS |
| Studio 전체 / npm editor | 1,830 PASS/2 skip/0 fail; 32 PASS 및 type·pack PASS |
| fresh locked WASM / 웹·Desktop build / Desktop cargo check | PASS. WASM SHA-256 5c7a9a16c22cc6e0086d14d38df0386c5e63a1c75fe194a49848111c95011d12, pkg/public/upstream 동일 |
| CI bootstrap/manifest Node 계약 | 31 PASS. 정확한 base/repo/branch/source tree 및 보안 test original/candidate SHA-256 쌍을 검사한다. |
| pinned baseline sparse 정책 실제 실행 | missing upstream commit 상황의 filtered fetch, identity, suite prepare, manifest/unit-tier 검사가 PASS. 추가 94,093,234B. [근거](../assets/task_m100_hp110_filtered_bootstrap.json) |
| 문서 링크 | 실제 추가/이동 경로를 선택 검사했다. 일반 Markdown 수정만을 이유로 전체 CI를 로컬 재실행하지 않았다. |

보안 test 보정은 경로·exact SHA-256·전체 예상 finding(종류/배경/개수/위치/텍스트)을 모두 검증한다. hidden positive 세 건의 injection/unicode 검사와 음성 코퍼스 assertion을 유지한다. 다른 Rust/test/policy 변경은 bootstrap이 거부하며 canonical threshold를 완화하지 않았다.

## 최초 CI 실패와 원 PR head 보정

초기 head dc3f47079120c64a89565b927b2d516c3a5ff30f의 [CI run](https://github.com/paldyn/HanPage/actions/runs/36832803599)에서 Lint의 Pages trigger mirror 계약 한 건이 실패했다. 이미 보존한 HanPage deploy-pages.yml의 paths-ignore와 upstream mirror 상수의 목록 차이였다. scripts/ci-impact-policy.cjs의 상수 목록만 실제 workflow와 맞췄다(12 insertions/1 deletion). 해당 상수는 export·mirror test에서만 참조되며 배포 workflow·assertion·enforcement·skip 조건은 바꾸지 않았다.

동일 focused suite는 보정 전 40/41 PASS + mirror 1 FAIL, 보정 뒤 41/41 PASS였다. classifier/policy/controller/evidence/report/duration Node bundle 200/200 PASS, diff/check PASS. 파일 SHA-256 aff4e0109508f1c6ad7d07db14cc8bcf736bec18a5ed9fb221e61fbf84d126ac → 40b33b3c398f18d7c41de2da685f8fa8f312ce4616a3e0c3543103dc5e6b914b. bundle 첫 시도는 파일 확장자 오기로 실행 전 실패했으며 완료 결과로 세지 않았다. [CI 보정 기록](../assets/task_m100_hp110_ci_contracts.json). 원 PR이 OPEN인 병합 전 보정이며 병합 후 직접 source 반영은 아니다.

## Frontend CI에서 확인한 HanPage 다운로드 버튼 겹침

초기 CI의 responsive gate는2,657PASS/9FAIL이었다. HanPage full-label 다운로드 CTA가 upstream 반응형 메뉴의 경계 폭에 더해지면서 영어600px의6skin/theme 조합과 한국어520/500/480px에서 가로폭이 넘쳤다. 기존 menu8개·토글·479px scroll 계약과 assertion은 유지하고, <=767px에서는 기존 버튼을24px↓아이콘으로 표시해 다운로드 접근을 유지한다. `content: '↓' / ''`로 장식 glyph의 접근성 대체문자열을 비우며 원래 textContent/title/접근 이름/click을 바꾸지 않는다. >=768px의 기존 full CTA는 그대로다.

actual Chrome의12개 locale/skin/theme/width 조건에서 outer/menuoverflow0·메뉴8개/1행·narrow24px visible·exact 접근 이름과 title/text 보존·768px 이상 full label 복귀를 확인했다. 600px 실제 클릭은 GitHub Releases fetch 대역을 통해 예상asset popup URL/target/features를 반환했다. 실제 download는 실행하지 않았다. 최종 `npm run e2e:responsive`는 **2,666 PASS/0 FAIL**, TypeScript와 관련 단위22/22도 PASS였다. [정확한 명령·결과·source 해시](../assets/task_m100_hp110_responsive/responsive-download-final-results.json), [12조건 측정·접근성·클릭](../assets/task_m100_hp110_responsive/responsive-download-icon-ui-probe.json), [영어600px](../assets/task_m100_hp110_responsive/responsive-download-icon-header-en-600.png), [한국어480px](../assets/task_m100_hp110_responsive/responsive-download-icon-header-ko-480.png), [한국어1280px](../assets/task_m100_hp110_responsive/responsive-download-icon-header-ko-1280.png)를 보존했고 root가 실제 PNG를 직접 열었다.

콜드 첫 locale에서 문서 이벤트가 async 초기화 완료를 기다리지 않는 setup 경합도 재현했다. loadApp 반환6106ms에 pageCount1이어도 root.rhwp-busy/inputActive=false였고,6159ms keyboard trigger의 실제 focus는 BODY였다.9503ms init 완료가 TEXTAREA focus를 적용하고9505ms busy 종료/inputActive=true로 바뀌었다. 기존 keyboard assertion/timeout/viewport를 완화하지 않고 responsive localsetup이 busy시작→종료·busyDepth0·활성 input/doc/toolbar를 기다리게 했다. 기존 productionJS는 바꾸지 않았다. [보정 전 실제 trace](../assets/task_m100_hp110_responsive/responsive-first-focus-probe.json)와 [보정 후 실제 trace](../assets/task_m100_hp110_responsive/responsive-first-focus-ready-probe.json)를 보존했다. 2개 CDN 요청을 각각6초 지연한 통제 양성에서도 loadApp6292ms 시점 busy/inputActive=false, helper18955ms 종료 뒤 기존 keyboard30초 assertion PASS를 확인했다. [초기 busy 경로 양성](../assets/task_m100_hp110_responsive/responsive-first-focus-ready-delayed-probe.json). 처음 pkg symlink의 Vite403 환경 시도, hide 대조와 readiness 보정 전 icon gate의 첫30초 timeout은 실패 시도로 남기고 최종 PASS로 세지 않는다.

## Gym 계약의 동등 로컬 검증

대형 PR의 GitHub path-filter diff 상한으로 Gym 자동 실행이 생성되지 않았고 default main에 해당 workflow가 없어 API dispatch도 404였다. main/base를 먼저 바꾸지 않았다. 후보 befaca4650dc3b1611b4b654a3554adfb3f69eeb에서 workflow의 Validate Gym contracts 명령(22개 unittest 모듈)을 그대로 실행해 2,125건 중 2,124 PASS/1 skip, exit0을 확인했다. 실행 전후 2,358개 검증 파일 blob은 모두 같았다. 기존 self-diff report가 없어 한 건 skip했다. full benchmark 또는 GitHub Gym CI 통과로 세지 않는다. 정확한 명령·결과는 [CI contracts JSON](../assets/task_m100_hp110_ci_contracts.json)에 보존했다.

## 제품 진입점과 동작 기반 회귀

실제 Chrome 154/fresh WASM에서 파일 연결 큐 선행, HWP 열기·native 저장(16,896B)·취소 dirty 유지·재열기·준비된 업데이트 버튼, HWPX 저장·최근 문서·제목·새 문서를 실행했고 pageerror는 0이었다. Tauri invoke/event는 대역이다. 실제 OS 대화상자·파일 연결·업데이트 설치·서명·공증은 미실행이다.

FontFace.load 영구 pending 반례에서 foreground 8,015.7ms 반환·background 배치 지속·foreground progress 중단을 확인했다. 실제 키보드 편집·Chromium 다운로드·committed HWP 재열기에서 “font pending smoke”가 남았다. timeout focused 두 개와 native queue race는 수정 전 실패·수정 후 통과였다. native picker catch 제거 음성대조는 unhandled rejection 1개, native 분기 제거는 3개가 실패했다. 원본 source hash는 바뀌지 않았다. [브라우저 결과](../assets/task_m100_hp110_browser_checks.json).

## 렌더 영향·조판 원칙·시각 증적

upstream source 수용으로 old fork 대비 renderer 변경은 있으므로 시각 검증 경로를 적용했다. 이 PR은 개별 layout 개선이나 전체 한컴 PDF fidelity 통과를 주장하지 않는다. imported src/crates/Cargo.lock 및 baseline/golden은 pinned upstream과 동일하고, HanPage 추가분은 geometry 규칙을 바꾸지 않는다. 후보 Native SVG와 fresh WASM Chrome 화면을 직접 열었다. [native startup](../assets/task_m100_hp110_native-start.png), [committed HWP 재열기](../assets/task_m100_hp110_font-pending-hwp-reopened.png), [숨김 색/영역 판독](../assets/task_m100_hp110_security_genuine/genuine-hidden-evidence.json).

| 공통 조판 검토 항목 | 판정과 실제 근거 |
| --- | --- |
| 구현 근거·일반성 | HanPage 추가 엔진 분기는 비해당: renderer/typeset delta 0. imported upstream 전체 개별 규칙에 대해 새 독립 개선 주장은 하지 않는다. |
| 측정·배치 일관성 | fork-specific 수정은 비해당: 동일 engine tree·동일 WASM hash. imported source의 전체 경로 재입증을 주장하지 않는다. |
| 분할·이어받기 계약 | fork-specific 수정은 비해당. 기존 전체 release-test 및 Native 직접 출력 회귀를 실제 실행했다. |
| 줄 소속·점유 높이 | fork-specific 수정은 비해당. 대표 원본의 실제 출력 확인 범위만 기록한다. |
| 사례·증거 독립성 | 충족: actual original HWP/HWPX·기준 PDF와 synthetic Tauri 동작을 구분하며 음성대조·색 판독 및 남은 OS 미검증을 명시했다. |
| 기준값 변경 | 별도 보정은 비해당: imported upstream baseline/golden을 유지하며 threshold나 goldens를 green 결과에 맞춰 갱신하지 않았다. |
| 주장·검증 범위 | 충족: source SHA, command 결과, committed hash, 실제 화면과 미검증을 구분했다. 대표 PDF 비교 결과는 아래에 추가한다. |

대표 두 문서의 p1을 같은 physical96dpi에서 scripts/visual_sweep.py로 직접 비교했다. 입력·기준 PDF 네 파일은 source/upstream/primary/submitted commit과 byte 동일하다. [출처·입력 해시](../assets/task_m100_hp110_visual/committed-provenance.json), [실제 명령·결과](../assets/task_m100_hp110_visual/validation-summary.json), [execution→permanent 경로·SHA 매핑](../assets/task_m100_hp110_visual/artifact-map.json)을 보존했다.

| 실제 Native SVG/한컴 비교 | 자동 후보·gate | 실제 수치·직접 판독 |
| --- | --- | --- |
| samples/re-03-latin-only-hancom.hwp / pdf/re-03-latin-only-hancom-2022.pdf p1 | flagged0/1, passed, exit0 | 2px 실루엣99.81464%, pixel99.23985%, strict ink proxy21.47822%; 세 줄 시작·끝·줄바꿈 보존. [review](../assets/task_m100_hp110_visual/official-sweep-latin/hp110-latin-p1/review/review_001.png), [overlay](../assets/task_m100_hp110_visual/official-sweep-latin/hp110-latin-p1/overlay/overlay_001.png) |
| samples/biz_plan.hwp / pdf/biz_plan-2022.pdf p1 | flagged0/1, font_mismatch_exception, exit0; 정상90% 통과가 아님 | 2px86.04917%, pixel98.12743%, strict ink proxy16.89727%. 제목 굵기·glyph 폭과 괘선 stroke 차이가 남는다. [review](../assets/task_m100_hp110_visual/official-sweep-final/hp110-business-p1/review/review_001.png), [overlay](../assets/task_m100_hp110_visual/official-sweep-final/hp110-business-p1/overlay/overlay_001.png) |

사업계획서 예외는 [font-mismatch-evidence-official.json](../assets/task_m100_hp110_visual/business/font-mismatch-evidence-official.json)의 exact SHA-256 2f7ffe102dbebfc7b38fe7af157800e481e0f6fafd3565271fc2cd9a8870e366로 실제 --font-mismatch-evidence에 지정했다. PDF의 사용 face HCRBatang/H2hdrM/HCRBatang-Bold와 공식 래스터 경로의 실제 NotoSerifKRExtraLight-Regular/HCRDotum은 교집합0이다. CDP45개 node와 실제 공급 font program/name/hash를 대조했고, 동일 HTML 재캡처 PNG가 공식 출력과 byte 동일함을 확인했다. earlier bareSVG의 Nanum/Apple 결과를 공식 예외로 쓰지 않았다. [실제 font program](../assets/task_m100_hp110_visual/business/official-actual-font-programs.json).

예외 지정 전에 같은 좌표계에서 괘선4개의 endpoint 최대0.910px, 문단 baseline/anchor 최대0.932px 차이를 확인했다. 제목 첫 glyph origin의9.625px 차이는 숨기지 않았고 실제 CID width/Tz와 SVG em/scale로 계산한 center anchor는0.212px 차이였다. 그림은 p1에0개라 해당 경계는 비해당이다. 괘선 stroke0.04..0.239px 차이와 다른 실제 글꼴의 outline/weight는 residual로 남겼다. 예외는 business p1과 이 입력·출력에만 적용되며 다른 쪽의 배치 결함 면제가 아니다.

대표 두 official review/overlay와 Latin SVG 호환 PDF overlay를 root와 독립 agent가 직접 열었다. 도구 라벨과 본문을 구분했고 한글/수치가 읽혔다. Latin 호환 PDF 실제 출력의2px 점수는100%, strict proxy44.13694%였으며 [PDF](../assets/task_m100_hp110_visual/latin/native-compatibility.pdf)와 [overlay](../assets/task_m100_hp110_visual/latin/native-pdf-overlay-p001-96dpi.png)를 보존했다. 이 기존 CLI 바이너리에는 native-skia가 없어 대표 direct-PDF 시도는exit1이었고 통과로 세지 않는다. 앞 절의 별도 feature-enabled Native Skia focused4/4와 혼동하지 않는다. 처음 mismatchDPI의 r000/g000 비교는 제외했다. Business의 bareSVG/PDFium88.83287%와 official/Poppler86.04917%는 서로 다른 실제 래스터 경로로 구분한다. 전체 문서 쪽 또는 전체 upstream fidelity를 주장하지 않는다.


GitHub Render Diff의 최신 head 결과와 actual artifact를 확인한 뒤 병합했다. 아래 최종 증적과 남은 차이를 따른다. PR 본문 대표 이미지 URL은 최종 PR head의 raw SHA에 고정하고 API에서 본문 존재를 재확인한다. UI/색 판독 증거를 전체 Hancom fidelity로 승격하지 않는다.

## Merge 후 contributor PR comment 계획

GitHub 댓글 게시 승인은 받지 않았으므로 게시하지 않는다. 추후 별도 승인 시 [Visual Sweep 정본](../../manual/verification/visual_sweep_guide.md#github-merge-comment), 실제 CI URL·merge SHA·검증 범위·남은 차이를 한국어 존댓말 본문으로 작성한다. 영구 asset이 devel에 있는지 확인하고 raw.githubusercontent.com/paldyn/HanPage/<merge-sha>/mydocs/pr/assets/ 경로로 이미지를 고정하며 --body-file 게시 뒤 API로 줄바꿈·이미지 표시를 재확인한다. 실제 수치만 쓰고 UI 스모크를 문서 픽셀 일치율로 쓰지 않는다.

## 처리 계획과 남은 범위

[implementation 단계](pr_111_review_impl.md)대로 최신 head의 CI/CodeQL/RenderDiff PASS·mergeability·base/head를 확인하고 --match-head-commit으로 병합했다. merge SHA/devel 포함·duration refresh 갱신 보류·issue CLOSED를 확인해 archive review/오늘할일 운영 기록으로 남겼다. 사용자 Desktop 작업공간과 shared target/pr-review를 보존했으며 clean managed worktree는 앱의 pinned task/workspace 보호로 archive가 거절돼 유지했다. 앱 릴리스, Dependabot close, main 승격 CI 정책, 실제 OS updater 설치는 별도 범위다.

## 최종 head CI·실제 Render Diff와 남은 범위

final head f9f2e47bf43d87fff7e1faabcdac361c7091be12의 생성된 CI·CodeQL/GHAS·Adapter·Proptest·Skill 검사와 [manual Render Diff](https://github.com/paldyn/HanPage/actions/runs/36836467524)를 확인했다. [API/actual 단계 근거](../assets/task_m100_hp110_final_ci/final-ci-audit.json), [root artifact 수치/직접판독](../assets/task_m100_hp110_final_ci/render-artifact-root-verification.json). 최초 dc3 실패나 이전 head 결과를 최신 결과로 세지 않았다. 최종 Git diff는28,104파일/+6,895,197/-161,972, tracked37,475경로이며 exhaustive rename detection은 Git에서 생략됐다.

Canvas legacy/layered p1비교3/3 PASS(KTX166pixel=0.0186169%, 다른2개0); Native Skia direct(print)/Native SVG 호환PDF p1비교3/3 PASS(사업계획서1.15816%, tac0.34394%, kps0.81741%, canonical2%); CanvasKit readiness8/8 evaluated·PASS·0failed/0missing·16pairedcapture였다. readiness의Nativecapture는선정browser-only경로여서비해당이다. PDF의browser/compatibility report는4경고/0error·report-only이며72dpi/96dpi사이즈차이가남는다. [actual JSON](../assets/task_m100_hp110_final_ci/render/rhwp-studio/e2e/screenshots/render-diff/pdf-results.json), [readiness](../assets/task_m100_hp110_final_ci/render/output/renderer-baseline/canvaskit-readiness/baseline-report.json).

root가businesscompat/direct/diff·KTXlayer·CanvasKit표PNG를직접열었다. Linux SVG 호환PDF의사업계획서에는제목·괘선만남고상단문구/날짜/회사문구가누락되는차이가있다. Native Skia직접PDF에는문구가남는다. 1.15816%whole-page gate통과가의미상누락해결을뜻하지않으며source/threshold/golden을그결과에맞춰바꾸지않았다. 입력과upstream엔진동일성은유지한다. [호환 raster](../assets/task_m100_hp110_final_ci/render/rhwp-studio/e2e/screenshots/render-diff/biz_plan.hwp-618f6a38-p01-pdf-raster.png), [direct raster](../assets/task_m100_hp110_final_ci/render/rhwp-studio/e2e/screenshots/render-diff/biz_plan.hwp-618f6a38-p01-direct-raster.png). [PDF 원본/text/font·호출 경로 진단](../assets/task_m100_hp110_final_ci/compatibility-pdf-font-gap.json)을 보존했다. Linux generic serif 초기fontdb선택실패가유력하지만당시fontdb상태가없어강한정적추론이다. usvg초기선택None은span을건너뛰어per-character fallback이구제하지못한다. HanPage저장은WASM PrintSVG→browserwindow.print경로로native변환기를직접쓰지않으며실제frontend인쇄PDF는이진단에서미검증이다. 이CLI/Linux글꼴경로의후속이필요하며web/Desktop저장전체에서동일누락이발생한다고확대하지않는다. 대표HancomPDF2쪽의font_mismatch_exception과이CIbackend비교는별개다.

## 병합 뒤 처리와 정리

PR #111 실제 MERGED, merge SHA b57d9ae5e77bf3f10f76ced17e40dc7c47673930가origin/devel에포함됨을확인했다. 직접후속기록은archive review/impl·오늘할일·mydocs/pr/assets만한commit으로반영하며code/test/workflow/golden/sample은수정하지않는다. 병합후검증CI를다시시작하지않았다. Duration refresh와issue상태는 [duration](../assets/task_m100_hp110_final_ci/duration-refresh-result.json), [issue](../assets/task_m100_hp110_final_ci/issue-final.json)에고정했다. 사용자승인없는GitHub댓글/수동issueclose/Dependabotclose/앱release는실행하지않았다.

primary는현재branch를유지한채merged origin/devel tree로ff-only동기화했다. 사용자Desktop변경39개/3,999,393B를직전/직후byte로검사해보존했고이번owned보안test/HWP는committed target과동일하다. [동기화/해시](../assets/task_m100_hp110_final_ci/primary-sync-result.json). 기본작업공간의Desktopdirty와다른작업소유변경을보존하므로local/remote codex/upstream-sync-20261001 branch는유지한다. 작업소유clean managed worktree의app archive는보호오류(This worktree is protected by a pinned task or workspace.)로거절돼그경로를유지했다. 직접filesystem삭제로보호를우회하지않았고 [실제정리](../assets/task_m100_hp110_final_ci/worktree-cleanup-result.json)에증거를남겼다. 공유target/pr-review(13GiB)·기본workspace·다른도구산출물은보존한다. 영구증적확인뒤unused owned output/pr-review/hp110만정리한다.
