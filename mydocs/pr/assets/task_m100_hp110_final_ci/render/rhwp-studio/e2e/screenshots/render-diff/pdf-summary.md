# PDF Visual Diff Report

- browser/compatibility status: report-only
- fixtures: basic/KTX.hwp, biz_plan.hwp, tac-case-001.hwp, kps-ai.hwp
- scale: 1
- raster DPI: 72
- max pages: 1
- channel tolerance: 8
- command timeout: 120000ms
- report threshold: disabled
- compared pages: 4
- warning pages: 4
- error pages: 0
- direct PDF gate: enabled
- direct PDF fixtures: biz_plan.hwp, kps-ai.hwp, tac-case-001.hwp
- direct PDF fallback raster DPI: 144
- direct/compatibility max ratio: 0.02
- direct PDF compared pages: 3
- direct PDF failed pages: 0

| Status | Fixture | Page | Size | Diff pixels | Diff ratio | Max channel delta | Artifacts |
| --- | --- | ---: | --- | ---: | ---: | ---: | --- |
| warn | basic/KTX.hwp | 1 | 1123x794 vs 842x596 | 520097 | 0.58328941 | 255 | e2e/screenshots/render-diff/basic_KTX.hwp-ce76d1d6-p01-pdf-reference.png<br>e2e/screenshots/render-diff/basic_KTX.hwp-ce76d1d6-p01.pdf<br>e2e/screenshots/render-diff/basic_KTX.hwp-ce76d1d6-p01-pdf-raster.png<br>e2e/screenshots/render-diff/basic_KTX.hwp-ce76d1d6-p01-pdf-diff.png |
| warn | biz_plan.hwp | 1 | 794x1123 vs 596x842 | 19366 | 0.02171899 | 255 | e2e/screenshots/render-diff/biz_plan.hwp-618f6a38-p01-pdf-reference.png<br>e2e/screenshots/render-diff/biz_plan.hwp-618f6a38-p01.pdf<br>e2e/screenshots/render-diff/biz_plan.hwp-618f6a38-p01-pdf-raster.png<br>e2e/screenshots/render-diff/biz_plan.hwp-618f6a38-p01-pdf-diff.png |
| warn | tac-case-001.hwp | 1 | 794x1123 vs 596x842 | 3566 | 0.00399927 | 255 | e2e/screenshots/render-diff/tac-case-001.hwp-097ee39c-p01-pdf-reference.png<br>e2e/screenshots/render-diff/tac-case-001.hwp-097ee39c-p01.pdf<br>e2e/screenshots/render-diff/tac-case-001.hwp-097ee39c-p01-pdf-raster.png<br>e2e/screenshots/render-diff/tac-case-001.hwp-097ee39c-p01-pdf-diff.png |
| warn | kps-ai.hwp | 1 | 794x1123 vs 596x842 | 52017 | 0.05833713 | 255 | e2e/screenshots/render-diff/kps-ai.hwp-1f4dbe57-p01-pdf-reference.png<br>e2e/screenshots/render-diff/kps-ai.hwp-1f4dbe57-p01.pdf<br>e2e/screenshots/render-diff/kps-ai.hwp-1f4dbe57-p01-pdf-raster.png<br>e2e/screenshots/render-diff/kps-ai.hwp-1f4dbe57-p01-pdf-diff.png |

## Warnings

- basic/KTX.hwp page 1: 58.32894% differs
- biz_plan.hwp page 1: 2.17190% differs
- tac-case-001.hwp page 1: 0.39993% differs
- kps-ai.hwp page 1: 5.83371% differs

## Direct PDF Compatibility Gate

| Status | Fixture | Page | Size | Diff pixels | Diff ratio | Max channel delta | Artifacts |
| --- | --- | ---: | --- | ---: | ---: | ---: | --- |
| pass | biz_plan.hwp | 1 | 596x842 | 5812 | 0.01158157 | 255 | e2e/screenshots/render-diff/biz_plan.hwp-618f6a38-p01.pdf<br>e2e/screenshots/render-diff/biz_plan.hwp-618f6a38-p01-pdf-raster.png<br>e2e/screenshots/render-diff/biz_plan.hwp-618f6a38-p01-direct.pdf<br>e2e/screenshots/render-diff/biz_plan.hwp-618f6a38-p01-direct-raster.png<br>e2e/screenshots/render-diff/biz_plan.hwp-618f6a38-p01-direct-vs-compat-diff.png |
| pass | tac-case-001.hwp | 1 | 596x842 | 1726 | 0.00343940 | 253 | e2e/screenshots/render-diff/tac-case-001.hwp-097ee39c-p01.pdf<br>e2e/screenshots/render-diff/tac-case-001.hwp-097ee39c-p01-pdf-raster.png<br>e2e/screenshots/render-diff/tac-case-001.hwp-097ee39c-p01-direct.pdf<br>e2e/screenshots/render-diff/tac-case-001.hwp-097ee39c-p01-direct-raster.png<br>e2e/screenshots/render-diff/tac-case-001.hwp-097ee39c-p01-direct-vs-compat-diff.png |
| pass | kps-ai.hwp | 1 | 596x842 | 4102 | 0.00817405 | 255 | e2e/screenshots/render-diff/kps-ai.hwp-1f4dbe57-p01.pdf<br>e2e/screenshots/render-diff/kps-ai.hwp-1f4dbe57-p01-pdf-raster.png<br>e2e/screenshots/render-diff/kps-ai.hwp-1f4dbe57-p01-direct.pdf<br>e2e/screenshots/render-diff/kps-ai.hwp-1f4dbe57-p01-direct-raster.png<br>e2e/screenshots/render-diff/kps-ai.hwp-1f4dbe57-p01-direct-vs-compat-diff.png |
