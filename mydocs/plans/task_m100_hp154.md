# Task #154 — upstream/devel 통합 수행계획 (2026-10-27)

- Issue: [paldyn/HanPage#154](https://github.com/paldyn/HanPage/issues/154)
- 사용자 승인: 2026-10-27 "응 고고" — 쌓인 upstream 변경 통합 진행
- 브랜치: `claude/upstream-sync-20261027`
- HanPage 기준: `9b4f20a69c5eec46ce8b3f2235b8ecc0b93375a0` (Desktop 0.8.11)
- 지난 upstream 고정점: `02530b9ed567a44663edb26c65fb565c4a79f00d` (0.8.6, #110)
- 신규 upstream 고정점: `faa885e8b54ffc7cdf4196cd43a3103f362499a2` (엔진·Studio 0.8.7)

## 통합 방식

#110 과 같다. Task #51 의 pdf-large 제거 이력을 보존하기 위해 upstream 이력을 merge 하지 않는다. `origin/devel` 을
부모로 둔 통합 브랜치에서 최신 upstream 파일 트리를 수용하고, 지난 고정점 대비 HanPage 델타만 3-way 로 이식한다.
일반 PR 로 devel 에 반영하며 force-push 하지 않는다. 다음 동기화의 기준이 되도록 upstream SHA 와 분류 manifest 를 남긴다.

분류 기준은 "지난 고정점(P) 과 HanPage devel(D) 의 차이만 HanPage 델타"다. P 기준이므로 upstream 이 지운 파일을 HanPage
델타로 오인하지 않는다(#51 함정). upstream 이 지운 475경로는 되살리지 않는다.

| 분류 | 경로 수 | 적용 |
| --- | --- | --- |
| HanPage 추가(upstream 없음) | 490 | D 그대로 |
| HanPage 수정, upstream 무변경 | 22 | D 그대로 |
| 양쪽 수정 | 33 | 3-way(base P, ours U, theirs D). 충돌 5건 수동 해결 |
| 같은 날짜 오늘할일을 양쪽이 추가 | 5 (+ 양쪽 수정 1) | upstream 파일 유지, HanPage 기록은 `mydocs/orders/archives/<날짜>_hanpage.md` 로 이전(#110 과 같은 방식) |

## 단계

1. 트리 구성·분류·충돌 해결(이 문서).
2. 의미 검토: 글자 충돌 없이 합쳐진 파일을 영역별로 독립 검토하고 반박 검증한다. 특히 upstream 의 Studio PDF 직접 열기와
   HanPage 데스크톱 native 열기·저장·파일 연결 큐의 상호작용, 폰트 timeout, 웹/데스크톱 빌드 분리, CI bootstrap.
3. CI 정책 bootstrap 재고정: base·upstream·head branch 를 이번 통합 값으로 바꾸고, upstream 과 다른 HanPage 테스트 입력은
   정확한 SHA-256 으로만 허용한다.
4. 검증: fresh WASM, Studio tsc·단위·E2E(desktop-update-ui·반응형), 웹/Desktop frontend 빌드, Desktop crate
   check·clippy·test, npm/editor, Rust lint 묶음·전체 nextest, 실브라우저 스모크(native 열기·저장·폰트 pending·업데이트),
   Desktop 번들 dispatch 시험.
5. 큰 push 분할(단일 push 2 GiB 상한), PR, 최신 CI 확인 후 병합. Desktop 앱 릴리스는 별도.
