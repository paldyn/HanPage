# PR #185 리뷰 — WMF 이미지 보존 검사의 글꼴 의존 제거 (#184)

## 판정

**병합 완료.** 검토 head `1291db85cc` 의 GitHub CI 28 성공·6 건너뜀·실패 0(MERGEABLE·CLEAN)을 확인하고 2026-11-10 squash 병합했다(merge `ee549d9d65269c0a468dffc271c2e6904d7a9cb7`). #184 는 devel push 워크플로가 닫는다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#185](https://github.com/paldyn/HanPage/pull/185) / enxec / paldyn/HanPage devel |
| 고정 base | `0615710804` |
| 커밋 | `1291db85c` upstream rhwp `8c97771760` 의 테스트 보정 |
| 관련 이슈 | [#184](https://github.com/paldyn/HanPage/issues/184) |
| 경로 | maintainer 본인 작업. PR #174 CI 실패의 후속 → 작업지시자 "새세션작업 추천해준것도 해야하는거면 진행해"(2026-11-10) |

## 구현 주장과 검증

| 주장 | 근거 | 판정 |
| --- | --- | --- |
| 실패 원인은 새 러너 이미지의 Noto Color Emoji 다 | 진단 run 42729517482(ubuntu-26.04 20261101.186.1): `SubsetError` face 가 `/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf`(`fonts-noto-color-emoji` 2.051). 20261025.179.1 에서는 같은 코드가 통과 | 충족 |
| 수정한 테스트가 그 이미지에서 통과한다 | 진단 run 42730713904(같은 이미지): 2 passed, `SubsetError` 없음. PR CI 의 `test-archive-b-shard-1`(같은 이미지)에서 `issue_6612` 2개 PASS | 충족 |
| 테스트의 기대값과 제품 동작은 그대로다 | 바뀐 파일은 `tests/cases/issue_6612_pdf_keeps_sub_svg_images.rs` 하나, upstream 수정본과 같음. 기대값 21 유지 | 충족 |
| Rust lint 묶음 | fmt check, clippy native·wasm32·workspace all-targets, workspace build, manifest `--check --base-ref 0615710804` 모두 통과 | 충족 |

upstream rhwp 도 같은 원인을 진단해(늘어난 80개는 글자 비트맵 40개와 soft mask 40개) 같은 방식으로 보정했다. 같은 파일을 쓰므로 다음 upstream 동기화에서 충돌하지 않는다.

## 조판 원칙·시각 증적

비해당 — 테스트의 입력 글꼴만 고정했다. 렌더링 코드·기준값·쪽수·좌표는 바꾸지 않았다.

## 병합 후

- 같은 조건(문서 글꼴이 없고 Color Emoji 가 먼저 걸리는 환경)에서 제품 PDF 가 그 페이지 텍스트를 path 로 내보내는 동작은 #184 에 적어 둔 별도 판단 사항이다.
- 원격 브랜치 `claude/ci-6612-test-fonts` 는 지운다. 일회용 진단 브랜치 `claude/ci-6612-probe` 는 이미 지웠다(run 기록은 남는다).
- 본인 PR 이며 별도 댓글 지시가 없어 PR 댓글은 게시하지 않았다.
