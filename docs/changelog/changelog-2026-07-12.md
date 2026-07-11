# 2026-07-12

## HML signature detection CodeQL fix

- Root cause: the leading XML-comment expression combined a lazy comment-body repetition with an
  outer repeated-comment group. Repeated `--><!--` boundaries therefore admitted exponentially many
  backtracking paths; a 172-byte prefix already took hundreds of milliseconds.
- Decision: scan optional XML declaration and leading comments with monotonically advancing offsets
  and `indexOf`, then apply the existing root capture as a sticky expression at the resulting offset.
  This keeps XML/HML/HTML classification and the root-end offset contract while making preamble work
  linear in the bounded prefix size.
- Regression proof: the original implementation failed the malicious-prefix timing test; the scanner
  classifies the same input within the 100 ms budget and preserves XML declaration plus consecutive
  comment handling.
- Alternatives rejected: changing regex flags or lazy modifiers leaves the ambiguous nested
  repetition, and relying only on the 64 KiB prefix bound still permits expensive attacks from very
  small inputs.
