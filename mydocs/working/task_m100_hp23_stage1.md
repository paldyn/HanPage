# Task #23 Stage 1 완료 보고서 — 기준 고정 + 재베이스 브랜치 + triage 확정

- **이슈**: [paldyn/HanPage#23](https://github.com/paldyn/HanPage/issues/23) (M100)
- **단계**: Stage 1 / 5
- **계획서**: `mydocs/plans/task_m100_hp23_impl.md` §3 Stage 1
- **일자**: 2026-06-02

## 1. 단계 목표 (구현 계획서 §3)

> **Stage 1** — 기준 고정(upstream/devel SHA 기록) + `local/task23-rebase` 생성 + triage 확정(§1-1)
> 산출: 고정 SHA·triage 표·기반 빌드 sanity(`cargo build`)

## 2. 고정 기준 (SHA 기록)

재베이스 작업 전 기간 동안 **불변으로 고정**할 기준점이다. upstream/devel은 작업 중에도 계속 전진하므로(세션 시작 시 `f83c43b5` → 현재 `f6ffe9d6`), 본 SHA를 고정 기준으로 못박는다.

| 역할 | 브랜치·커밋 | SHA | 비고 |
|------|------------|-----|------|
| **새 기준 (base)** | upstream/devel | `f6ffe9d6` | "Merge task 1237 paragraph line segment save contract" — 재베이스의 새 뿌리 |
| **paldyn 레이어 권위 원본** | origin/main | `0156d8ef` | HanPage-Desktop·브랜딩·CI/Pages·mydocs 최종 상태 출처 |
| **분기점 (구 기준)** | merge-base | `854515f5` | "Task #76" (2026-04-08) — fork가 갈라진 옛 뿌리 |
| **계획서 커밋** | local/task23 | `f7e55fbf` | 수행+구현 계획서 + 오늘할일 |

## 3. 재베이스 브랜치 생성

- `local/task23-rebase` ← `f6ffe9d6` (pinned upstream/devel) 생성 완료.
- **LFS skip-smudge 적용**: `git lfs install --local --skip-smudge`. upstream은 `pdf-large/`의 대용량 PDF를 Git LFS로 추적하나 객체가 로컬에 없어 일반 체크아웃 시 smudge 필터가 실패한다. skip-smudge로 LFS 포인터만 기록하도록 전환 — 권위 PDF는 엔진 빌드/테스트에 불필요하므로 영향 없음.
- **기반 트리 검증**:

| 항목 | 상태 | 해석 |
|------|------|------|
| `HanPage-Desktop/` | **부재** | 데스크톱 앱은 전적으로 paldyn 추가물 → Stage 2에서 이식 |
| `rhwp-desktop/` | **부재** | upstream에는 데스크톱 디렉터리 자체가 없음(순수 엔진) |
| `src/` | **존재** (`.rs` 427개) | 순수 upstream 엔진 |

> 참고: fork의 devel은 stale한 `rhwp-desktop/`을 가졌으나, upstream/devel에는 데스크톱 디렉터리가 아예 없다. 재베이스는 이 비대칭을 자연 해소한다 — 데스크톱 전체를 Stage 2에서 main의 `HanPage-Desktop/` 최종 상태로 단일 이식.

## 4. 엔진fix 10건 triage 확정 (전부 DROP)

fork main 고유 56커밋 중 "엔진fix" 성격 10건을 전수 검증한 결과, **모두 upstream/devel에 동일 논리가 이미 반영(리플레이/대체)**되어 있어 **전부 DROP**한다. 따라서 paldyn 레이어는 **순수 비-엔진**이며, 재베이스 시 엔진 라인을 단 한 줄도 이식하지 않는다.

| # | fork 커밋 | 내용 | upstream 대응 | 판정 |
|---|-----------|------|--------------|------|
| 1 | `ec9ce12f` | Task #741 | `4bfcee05` (리플레이) | DROP |
| 2 | `e7ee056b` | textbox 인라인 렌더 | `layout.rs` 로직+샘플 존재 (`has_real_text`·`get_inline_shape_position` 확인) | DROP |
| 3 | `96214c03` | Task #993 | `a6bd1cb5` (리플레이) | DROP |
| 4 | `f9b5b584` | image 샘플 | upstream `hy-002` 샘플 | DROP |
| 5 | `a5511168` | Task #1052 | `a0391ee8` (리플레이) | DROP |
| 6 | `9de60328` | Task #1061 | `9fbf518b` (대체, EQEDIT 정오표 포함) | DROP |
| 7 | `acba85af` | Task #1064 | `ee7b1e2f` (리플레이) | DROP |
| 8 | `df34df9c` | Task #1067 | `22318bb2` (리플레이) | DROP |
| 9 | `66a49bb0` | PR #1101 | `68fecab4` (리플레이) | DROP |
| 10 | `55c4bb0a` | LFS pdf-large 추가 | 인프라(LFS 추적) — 엔진 무관 | DROP |

**결론**: 재베이스 후 엔진 `src/`는 upstream/devel과 **byte-identical** 목표(Stage 4에서 diff=0 검증). paldyn 레이어 = 리브랜딩 + HanPage-Desktop + CI/Pages + mydocs(전부 비-엔진).

## 5. 기반 빌드 sanity

```
cargo build  (네이티브, dev 프로파일)
→ Compiling rhwp v0.7.13 (...)
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.95s
```

- 빌드 **성공**. upstream/devel 기반이 로컬에서 정상 컴파일됨을 확인.
- 엔진 크레이트명 `rhwp` 보존 확인(보존 불변식 부합).

## 6. 보존 불변식 점검 (현 단계 해당분)

| 불변식 | 현 단계 상태 |
|--------|-------------|
| rhwp 엔진 식별자(`rhwp`/`@rhwp/*`/edwardkim) | 기반이 upstream이므로 **자연 보존** ✅ |
| 시크릿 금지 | 신규 시크릿 없음 ✅ |
| GitHub Pages 무영향 | 현 단계 변경 없음 ✅ |
| paldyn 브랜딩·HanPage-Desktop | Stage 2~3 재적용 예정 (미해당) |

## 7. 브랜치 구조 (작업 모델)

- **`local/task23`**: 이슈 #23 명명 브랜치 — 계획서·단계별 보고서·최종 보고서 등 mydocs 보관(planning).
- **`local/task23-rebase`**: 재베이스 실행 브랜치 — 엔진(upstream) + 데스크톱 + 브랜딩 + CI/Pages 누적(product). Stage 5에서 mydocs를 일괄 반영한 뒤 devel로 force-push.

> 단계별 보고서는 본 planning 브랜치(`local/task23`)에 커밋하여 Task #23 문서를 한곳에 모은다. Stage 5에서 전체 `mydocs/`가 product 브랜치로 함께 이전된다.

## 8. 다음 단계

- **Stage 2** — HanPage-Desktop 이식: `local/task23-rebase`에서 origin/main(`0156d8ef`)의 `HanPage-Desktop/` 최종 상태를 복사(self-contained, 엔진 충돌 0).
- **승인 대기** — 본 보고서 승인 후 Stage 2 착수.
