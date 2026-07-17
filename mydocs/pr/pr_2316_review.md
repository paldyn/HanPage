# PR #2316 검토 — legacy /web 개발 앱과 current tooling 결합 제거

- PR: https://github.com/edwardkim/rhwp/pull/2316 (postmelee, collaborator)
- 이슈: #2313 (assignee postmelee, OPEN) / 관련 #2022 #2125 #2124
- base=devel, head=postmelee:task2313-legacy-web-removal (3커밋)
- 상태: **CONFLICTING** (orders 문서 1건, add/add) / maintainerCanModify=true
- CI (head, same-base): CodeQL·Frontend package gates·Native Skia 전 pass

## 변경 본질

repository 가 소비하지 않는 tracked `web/` 18 entries (legacy HTML/JS 앱,
Python HTTPS 서버, cert/key, 생성물 glue) 제거 + CI detector·metrics·font
contract 의 `/web` 결합 정리 + 한/영 local server 매뉴얼 Studio 현행화.
Git history 를 archive 로 사용 (stub/복사본 없음). Rust/WASM·에디터
공개 계약·폰트 바이너리는 불변.

## 구조 검토

- `.github/workflows/ci.yml`: frontend prefix 목록에서 `web/` 1줄 제거 — 정합
- `scripts/frontend-metrics.mjs`: `legacy-web` include group + web 전용
  exclude 5건 제거 — 정합
- `scripts/frontend-font-assets.test.mjs`: 심링크 계약을 Studio canonical
  단독으로 축소 (`web/fonts` 링크 항목 제거) — 정합
- 잔존 참조 스캔: workflows/scripts/package.json 에 `web/` 참조 0건,
  `git ls-files web/` 0건. studio 소스의 "web-substitute" 등은 무관 문자열
- 컨트리뷰터 자체 하이퍼-워터폴 문서(plans/working/report, stage 1~5) 동봉

## 로컬 재실증 (merged tree = devel 6d57ed81 + head)

| 게이트 | 결과 |
|--------|------|
| rust 영향 | diff 0 (src/Cargo 불변 — devel 최신 rust 그대로) |
| font asset contract | **4/4** |
| frontend-metrics | 정상 실행 (214 files, legacy group 소멸) |
| studio | npm ci + 단위 **307/307** + production build PASS |
| @rhwp/editor | npm ci + **15/15** |

## 충돌 해소안

`mydocs/orders/20260717.md` add/add — 컨트리뷰터가 자기 작업 기록으로 만든
오늘할일과 메인테이너 파일의 충돌. **union 해소**: 메인테이너 본문 유지 +
컨트리뷰터 #2313 행을 "컨트리뷰터 진행 (PR 연동)" 절로 편입.
maintainerCanModify=true → 해소 커밋을 head 브랜치에 직접 push
(fork LFS locksverify false 필요 시 적용) 후 merge.

## 판단

**merge 권고.** 소비되지 않는 legacy 결합 제거로 frontend 표면이 단순해지고
(Total CC -828, 함수 -149 — 전량 legacy 그룹), 계약·게이트가 Studio 단독
기준으로 명료해진다. 위험 표면은 문서/스크립트/CI 한정이며 재실증 전부 green.
