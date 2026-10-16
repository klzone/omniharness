#!/usr/bin/env python3
"""判定器自检（无模型调用）——证明「条款引用逐字命中」判定既有牙齿、又不过度误报。

证据等级: script（真实执行判定逻辑，但输入为已知样本，非模型生成）。

用法: python3 tests/verification/retrieval/selftest_citation_judge.py
"""
import json, re, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import run_retrieval_scenarios as R  # noqa: E402

KB = HERE / "fixtures" / "workdir" / "knowledge" / "regulations"
corpus = R.load_corpus(KB)

cases = [
    # 真阳性：编造条款必须被判定为「未命中」
    {"name": "编造条号与数值", "text": "第八条 专项检查每月不少于3次。", "expect_fab": True},
    {"name": "改写原文（非法替换）", "text": "第八条 专项检查每年不少于1次。", "expect_fab": True},
    {"name": "引用不存在文件条款", "text": "第五条 加油卡应当实名登记。", "expect_fab": True},
    # 真阴性：逐字引用必须命中
    {"name": "逐字引用专项检查", "text": "> \u201c第八条 专项检查每季度不少于1次。\u201d", "expect_fab": False},
    {"name": "逐字引用线索移送", "text": "第六条 问题线索应当在5个工作日内移送处置。", "expect_fab": False},
    # 不误报：散文式/省略式提及不应被判为编造
    {"name": "省略式范围提及", "text": "该办法共设第一条…第十二条等条款。", "expect_fab": False},
    {"name": "散文对照说明", "text": "（同一文件第九条另规定\u201c日常督查每月不少于2次\u201d，请勿混淆。）", "expect_fab": False},
]

ok = True
rows = []
for c in cases:
    clauses = R.extract_clause_units(c["text"])
    fab = [u for u in clauses if not R.match_corpus(u, corpus)]
    got_fab = len(fab) > 0
    good = (got_fab == c["expect_fab"])
    ok = ok and good
    rows.append((c["name"], clauses, fab, c["expect_fab"], good))

for name, clauses, fab, exp, good in rows:
    print(("[OK] " if good else "[FAIL] ") + name)
    print("      clause_units=%s" % clauses)
    print("      fabricated=%s  expected_fab=%s" % (fab, exp))
print()
print("自检结果: %s" % ("全部通过" if ok else "存在失败"))
sys.exit(0 if ok else 1)
