# -*- coding: utf-8 -*-
"""#2237/#2279 — 86712 pi=172 정량 게이트: cut/mt 행높이 vs 한글 오라클.

한글 실측(tools/task2195/rows_by_pi.py, 2026-07-15):
col0 세그 [23.3, 23.3, 23.3, 279.3, 23.3, 508.2, 1401.9] sum=2282.6,
r27(근거설명)=1401.9. rhwp 목표: 행높이 합 ≈ 2282.6, r27 ≈ 1402, 쪽수 65 유지.

usage: python tools/task2279/t172_gate.py [--exe target/debug/rhwp.exe]
"""
import argparse
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")

ap = argparse.ArgumentParser()
ap.add_argument("--exe", default=r"target\debug\rhwp.exe")
ap.add_argument("--doc", default=r"samples\86712_regulatory_analysis.hwp")
a = ap.parse_args()

env = dict(os.environ)
env["RHWP_TABLE_DRIFT"] = "1"
out = subprocess.run([a.exe, "dump-pages", a.doc], capture_output=True, text=True,
                     encoding="utf-8", errors="replace", timeout=900, env=env)
pages = len(re.findall(r"=== 페이지 ", out.stdout))
print(f"pages={pages} (한글 65)")
for ln in out.stderr.splitlines():
    if "TABLE_CUT_DRIFT: pi=172 " in ln:
        m = re.search(
            r"cut_sum=([\d.]+) mt_sum=([\d.]+) diff=([+\-\d.]+) "
            r"cut_rows=\[([^\]]*)\] mt_rows=\[([^\]]*)\]", ln)
        if m:
            cut = [float(x) for x in m.group(4).split(",") if x.strip()]
            mt = [float(x) for x in m.group(5).split(",") if x.strip()]
            print(f"pi=172 cut_sum={m.group(1)} mt_sum={m.group(2)} diff={m.group(3)}")
            print(f"  cut r16-27: {cut[16:28]}")
            print(f"  mt  r16-27: {mt[16:28]}")
            print("  (한글 목표: 합 2282.6, r27=1401.9)")
        break
