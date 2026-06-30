# Task M100 #1686 구현 계획서

## 접근

선행 RowBreak 표가 같은 문단 안에서 continuation을 만든 뒤, 후행 co-anchored 표를 같은 문단의 잔여
control로 즉시 배치하는 경로를 좁게 제어한다.

## 구현 후보

1. `typeset_block_table`이 표 분할 중 page advance를 발생시켰는지 호출자에게 알릴 수 있게 한다.
2. `typeset_table_paragraph`는 한 문단의 선행 비-TAC RowBreak 표가 분할 continuation을 만들면,
   같은 문단의 후행 비-TAC 표를 현재 control loop에서 계속 배치하지 않는다.
3. 단, 후행 표를 영구 누락하면 안 되므로 별도 pending control 모델이 필요하다.
4. pending 모델이 커지면 위험하므로, 우선 #1686 구조에 맞는 보수적 가드와 회귀 테스트를 통해
   최소 변경 범위를 찾는다.

## 주의할 회귀

- #1510: visible host co-anchored float tables의 문서/control 순서 유지
- #1535: visible host 후행 float가 선행 float 점유 영역을 침범하지 않아야 함
- #1639: 빈 host + 음수 offset 표의 문서 순서 유지
- #1658: RowBreak 분할 표의 page count/clip gate

## 초기 RED 기준

`samples/hwpx/pr-1674.hwpx`와 `samples/pr-1674.hwp`의 page 3 render tree에서
`다. 우대요건 등 [원서접수 마감일 기준]`가 보이지 않는 현재 동작을 실패로 고정한다.

