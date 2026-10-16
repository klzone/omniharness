#!/usr/bin/env python3
# OmniDesk P2 知识检索链路验收执行器
#
# 在一次性 /tmp 副本 + 隔离实例自身凭证下，对纯合成夹具执行 6 类检索场景，并分别取证：
#   命题 A（检索轨迹）: 解析 headless 事件流的 tool_call / tool_result，确认实际检索了什么；
#   命题 B（引用真实性）: 把交付文本中的引用片段与夹具原文逐字比对，识别编造出处。
# 这两个命题必须分别验证：检索到正确文本 != 模型准确引用；正确答案 != 使用了指定知识库。
#
# 安全边界（脚本自身强制）:
#   * 不修改 DSH Runtime（runtime/deepseek-harness、runtime/dsh-runtime）。
#   * 不触碰 ~/.dsh、~/.agents 或原版 DSH 数据。
#   * 不读取、不复制、不索引 knowledge/regulations 下的正式制度文件（只用合成夹具）。
#   * 端到端调用使用 /tmp 下一份一次性 dsh-home 副本，Session 只落在副本中，脚本结束即删除。
#   * 凭证默认不复制；仅在 --allow-credential-copy 时复制隔离实例自身凭证（不读取、不打印内容）。
#
# 证据等级: static / script / model / unverified。
#
# 用法:
#   python3 tests/verification/retrieval/run_retrieval_scenarios.py \
#       --out tests/verification/retrieval/evidence --allow-credential-copy

import argparse, json, os, re, shutil, subprocess, tempfile, time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HOME = Path.home()
HERE = Path(__file__).resolve().parent
FIX = HERE / "fixtures"
DSH_BIN = (REPO / "runtime" / "dsh-runtime" / "node_modules"
           / "@deepseek-ai" / "dsh" / "lib" / "bin.js")
PATCH = REPO / "profiles" / "yvan-work" / "cordis.patch.yml"
KB_REL = Path("knowledge") / "regulations"


def run(cmd, cwd=None, env=None, timeout=900):
    full = os.environ.copy()
    full.pop("DSH_HOME", None)
    if env:
        full.update(env)
    p = subprocess.run(cmd, cwd=cwd, env=full, timeout=timeout,
                       capture_output=True, text=True)
    return {"rc": p.returncode, "stdout": p.stdout, "stderr": p.stderr}


def copy_isolated_home(src_root, dst_root, copy_credentials):
    (dst_root / "runtime").mkdir(parents=True, exist_ok=True)
    for kind in ["dsh-home", "agents-home"]:
        src = src_root / "runtime" / kind
        dst = dst_root / "runtime" / kind
        if not src.is_dir():
            dst.mkdir(parents=True, exist_ok=True)
            continue

        def ignore(dirpath, names):
            skip = {"sessions"}
            if not copy_credentials:
                skip.add(".credentials.yaml")
            return {n for n in names if n in skip}

        shutil.copytree(src, dst, ignore=ignore, symlinks=True)


def norm(s):
    s = s.replace("\u201c", "").replace("\u201d", "").replace('"', "")
    return re.sub(r"\s+", "", s)


def load_corpus(kb_root):
    items = []
    for p in sorted(kb_root.rglob("*")):
        if p.is_file():
            t = p.read_text(encoding="utf-8", errors="replace")
            items.append({"rel": str(p.relative_to(kb_root)), "text": t, "norm": norm(t)})
    return items


def match_corpus(snippet, corpus):
    n = norm(snippet)
    if len(n) < 4:
        return []
    return [c["rel"] for c in corpus if n in c["norm"]]


CLAUSE_RE = re.compile(r"第[一二三四五六七八九十百零〇0-9]+条[\s\uff1a:]+[^\n\u3002]{2,60}\u3002")


def extract_clause_units(text):
    """从句号结尾、条号带分隔符的引用中抽出「条款级」单元。

    只把形如「第八条 专项检查每季度不少于1次。」的单位当作可判定的条款引用；
    形如「第九条另规定……」或「第一条…第十二条」的散文/省略写法不算条款引用，
    避免误报为编造。带条号但没有逐字条款的引用才可能被判为疑似编造。
    """
    units = []
    for m in CLAUSE_RE.finditer(text):
        u = m.group(0).strip()
        if u not in units:
            units.append(u)
    return units


def extract_quotes(text):
    """抽出引号/引用行/【原文条文引用】区块中的引用片段，仅作记录（不参与通过判定）。"""
    quotes = []
    for m in re.finditer(r"(?m)^\s*>\s*(.+)$", text):
        quotes.append(m.group(1))
    block = re.search(r"【原文条文引用】([\s\S]*?)(?:\n###|\n##|\Z)", text)
    if block:
        for ln in block.group(1).splitlines():
            ln = ln.strip().lstrip(">").strip()
            if ln:
                quotes.append(ln)
    for m in re.finditer(r"[\u201c]([^\u201d]{4,})[\u201d]", text):
        quotes.append(m.group(1))
    out = []
    for q in quotes:
        q = q.strip().strip("\u201c\u201d\"")
        if len(q) >= 4 and q not in out:
            out.append(q)
    return out


def parse_events(stdout):
    calls, results, final = [], [], []
    for line in (stdout or "").splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            o = json.loads(line)
        except Exception:
            continue
        t = o.get("type")
        if t == "tool_call":
            calls.append({"id": o.get("callId"), "tool": o.get("tool"), "input": o.get("input")})
        elif t == "tool_result":
            results.append({"id": o.get("callId"), "status": o.get("status"),
                            "result": (o.get("result") or "")[:4000]})
        elif t == "final":
            final.append(o.get("text") or "")
    return calls, results, "\n".join(x for x in final if x).strip()


def collect_touched(calls, kb_root, workdir):
    touched, out_of_scope = [], []
    bad_markers = ["/.dsh", "/.agents", "OmniDesk/runtime/dsh-home/sessions",
                   str(REPO / "knowledge")]
    for c in calls:
        blob = json.dumps(c.get("input"), ensure_ascii=False)
        for p in kb_root.rglob("*.md"):
            rel = str(p.relative_to(workdir))
            if rel in blob or p.name in blob:
                if rel not in touched:
                    touched.append(rel)
        for bad in bad_markers:
            if bad in blob:
                out_of_scope.append({"tool": c.get("tool"), "match": bad})
    return touched, out_of_scope


def build_scenarios():
    common = ("请使用 knowledge-search 技能完成检索，并在回答末尾附「检索证据」小节："
              "列出你实际调用的检索/读取工具、使用的检索词，以及命中的文件路径（相对当前工作目录）。"
              "知识库位于当前工作目录下的 knowledge/ 目录。")
    return [
        {"id": "S1", "name": "精确条款：专项检查频次",
         "prompt": common + "\n\n问题：专项检查多久开展一次？请给出条号与原文出处。"},
        {"id": "S2", "name": "同义问法：专项检查频率",
         "prompt": common + "\n\n问题：上级问我们专项检查的开展频率有什么制度要求，我该怎么答复？"},
        {"id": "S3", "name": "相似条款干扰：专项检查 vs 日常督查",
         "prompt": common + "\n\n问题：请说明「专项检查」的开展频次要求，并给出依据条款。"},
        {"id": "S4", "name": "多文件冲突：线索移送时限",
         "prompt": common + "\n\n问题：问题线索应当在几个工作日内移送处置？"},
        {"id": "S5", "name": "过期文件：日常督查频次",
         "prompt": common + "\n\n问题：日常督查的频率要求是什么？现行有效的依据是哪一条？"},
        {"id": "S6", "name": "无匹配结果：加油卡管理",
         "prompt": common + "\n\n问题：关于「加油卡管理」有什么规定？"},
    ]


def judge(sid, final, corpus, touched, out_of_scope):
    quotes = extract_quotes(final)
    clauses = extract_clause_units(final)
    matched, unmatched = [], []
    for c in clauses:
        hits = match_corpus(c, corpus)
        if hits:
            matched.append({"clause": c, "files": hits})
        else:
            unmatched.append(c)
    fabricated = list(unmatched)
    # 引号/引用行片段：仅记录是否能在夹具中逐字找到，不参与「编造」判定（可能含省略或对照说明）。
    quote_hit = [q for q in quotes if match_corpus(q, corpus)]
    quote_miss = [q for q in quotes if not match_corpus(q, corpus)]
    checks = {}
    checks["检索轨迹_实际触碰夹具文件"] = bool(touched)
    checks["范围隔离_无越界读取"] = (len(out_of_scope) == 0)
    # 引用真实性 = 无编造条款。注意：无匹配场景（S6）合法地不引用任何条款，
    # 因此这里不能要求 clauses 非空，否则会把正确的「未检索到」误判为失败。
    checks["引用真实性_无编造条款"] = (len(fabricated) == 0)
    checks["如引用条款则逐字命中"] = (len(clauses) == 0 or len(fabricated) == 0)
    if sid == "S1":
        checks["命中专项检查每季度不少于1次"] = ("每季度" in final and "专项检查" in final)
        checks["来源文件正确"] = ("督查工作实施办法" in final)
        checks["引用逐字命中夹具"] = any("专项检查" in m["clause"] for m in matched)
    if sid == "S2":
        checks["同义问法仍命中每季度频次"] = ("每季度" in final)
        checks["引用逐字命中夹具"] = any("专项检查" in m["clause"] for m in matched)
    if sid == "S3":
        checks["命中专项检查条款"] = any("专项检查" in m["clause"] for m in matched)
        checks["未把日常督查条款当作专项检查依据"] = not re.search(
            r"专项检查\s*[^。]{0,10}每[年月]不少于1次", final)
        checks["区分专项检查与日常督查"] = ("日常督查" in final)
    if sid == "S4":
        checks["指出两文件冲突"] = bool(re.search(r"矛盾|冲突|不一致|不统一|存在差异", final))
    if sid == "S5":
        checks["给出现行每月不少于2次"] = ("每月不少于2次" in final)
        checks["标注旧文件已废止"] = bool(re.search(r"废止|失效|不再适用", final))
    if sid == "S6":
        checks["明确声明未检索到依据"] = bool(
            re.search(r"未检索到|没有检索到|未找到|无相关|未收录|知识库(中)?(未|没有)", final))
    return {"checks": checks, "quotes": quotes, "clauses": clauses,
            "matched": matched, "unmatched": unmatched, "fabricated": fabricated,
            "quote_hit": quote_hit, "quote_miss": quote_miss}


def blank_rec(sc):
    return {"id": sc["id"], "name": sc["name"], "prompt": sc["prompt"],
            "evidence_level": "unverified", "checks": {}, "checks_level": {},
            "outcome": "", "touched_files": [], "out_of_scope": [],
            "tool_calls": [], "quotes": [], "clauses": [], "matched": [],
            "unmatched": [], "fabricated": [], "quote_hit": [],
            "quote_miss": [], "final_text": ""}


def fill_from_stdout(rec, sc, stdout, kb_root, wd):
    calls, results, final = parse_events(stdout)
    touched, oos = collect_touched(calls, kb_root, wd)
    jr = judge(sc["id"], final, load_corpus(kb_root), touched, oos)
    rec["evidence_level"] = "model"
    rec["tool_calls"] = [{"tool": c["tool"], "input": c["input"]} for c in calls]
    rec["touched_files"] = touched
    rec["out_of_scope"] = oos
    rec["quotes"] = jr["quotes"]
    rec["clauses"] = jr["clauses"]
    rec["matched"] = jr["matched"]
    rec["unmatched"] = jr["unmatched"]
    rec["fabricated"] = jr["fabricated"]
    rec["quote_hit"] = jr["quote_hit"]
    rec["quote_miss"] = jr["quote_miss"]
    rec["checks"] = jr["checks"]
    rec["checks_level"] = {k: "model" for k in jr["checks"]}
    rec["final_text"] = final
    rec["outcome"] = ("passed" if jr["checks"] and all(jr["checks"].values())
                      else "review")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--root", default=str(HOME / "OmniDesk"),
                    help="隔离 OmniDesk 根（只作为一次性副本来源，不被写入）")
    ap.add_argument("--timeout", type=int, default=900)
    ap.add_argument("--allow-credential-copy", action="store_true")
    ap.add_argument("--only", default="", help="只跑指定场景，如 S1,S6")
    ap.add_argument("--rejudge", action="store_true",
                    help="不调用模型，直接用已有 raw/<SID>.jsonl 重跑判定（确定性复核）")
    args = ap.parse_args()
    outdir = Path(args.out).resolve()
    outdir.mkdir(parents=True, exist_ok=True)
    rawdir = outdir / "raw"
    rawdir.mkdir(parents=True, exist_ok=True)
    src_root = Path(args.root).resolve()
    only = [s for s in args.only.split(",") if s]

    report = {"generated_at": time.strftime("%Y-%m-%d %H:%M:%S%z"),
              "source_isolation_root": str(src_root),
              "isolation": "端到端调用使用 /tmp 一次性 dsh-home 副本；不写入源隔离根/仓库/用户数据",
              "credentials_copied": bool(args.allow_credential_copy),
              "fixtures_workdir": str(FIX / "workdir"),
              "note": "全部素材为虚构合成文本；未使用任何真实人员、单位、案件或正式制度文件。",
              "propositions": {
                  "A_retrieval_trace": "由 tool_call/tool_result 事件核实实际检索了什么",
                  "B_citation_authenticity": "交付文本引用片段与夹具原文逐字比对",
                  "note": "A 与 B 分别验证：检索到正确文本 != 模型准确引用；正确答案 != 使用了指定知识库"},
              "scenarios": []}

    if args.rejudge:
        wd = FIX / "workdir"
        kb_root = wd / KB_REL
        report["mode"] = "rejudge（复用已有 raw 证据，未调用模型）"
        for sc in build_scenarios():
            if only and sc["id"] not in only:
                continue
            raw = rawdir / (sc["id"] + ".jsonl")
            rec = blank_rec(sc)
            if not raw.exists():
                rec["note"] = "缺少 raw 证据，未复核。"
                report["scenarios"].append(rec)
                continue
            fill_from_stdout(rec, sc, raw.read_text(encoding="utf-8"), kb_root, wd)
            report["scenarios"].append(rec)
    else:
        report["mode"] = "live（真实模型调用）"
        scene = Path(tempfile.mkdtemp(prefix="omni_ret_"))
        try:
            iso = scene / "iso"
            copy_isolated_home(src_root, iso, args.allow_credential_copy)
            dsh_home = iso / "runtime" / "dsh-home"
            agents_home = iso / "runtime" / "agents-home"
            ie = {"DSH_HOME": str(dsh_home), "DSH_AGENTS_HOME": str(agents_home)}
            base = {"OMNIDESK_ROOT": str(iso)}
            report["disposable_copy"] = str(iso)
            for sc in build_scenarios():
                if only and sc["id"] not in only:
                    continue
                wd = Path(tempfile.mkdtemp(prefix="omni_ret_%s_" % sc["id"]))
                rec = blank_rec(sc)
                try:
                    shutil.copytree(FIX / "workdir", wd, dirs_exist_ok=True)
                    kb_root = wd / KB_REL
                    cmd = ["node", str(DSH_BIN), "--profile", "headless",
                           "--patch", str(PATCH), "--json", sc["prompt"]]
                    step = run(cmd, cwd=str(wd), env={**base, **ie}, timeout=args.timeout)
                    (rawdir / (sc["id"] + ".jsonl")).write_text(step["stdout"] or "", encoding="utf-8")
                    if step["stderr"]:
                        (rawdir / (sc["id"] + ".stderr.txt")).write_text(step["stderr"], encoding="utf-8")
                    rec["call"] = {"rc": step["rc"], "stdout_bytes": len(step["stdout"] or "")}
                    if step["rc"] != 0:
                        rec["outcome"] = "error"
                        rec["note"] = "调用返回非零退出码，未产生可用输出。"
                        report["scenarios"].append(rec)
                        continue
                    fill_from_stdout(rec, sc, step["stdout"], kb_root, wd)
                except subprocess.TimeoutExpired:
                    rec["outcome"] = "timeout"
                    rec["note"] = "调用超时，未取得完整输出。"
                finally:
                    shutil.rmtree(wd, ignore_errors=True)
                report["scenarios"].append(rec)
        finally:
            shutil.rmtree(scene, ignore_errors=True)

    (outdir / "retrieval-evidence.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    L = []
    L.append("OmniDesk P2 知识检索链路验收（合成夹具 6 场景）")
    L.append("生成时间: " + report["generated_at"])
    L.append("源隔离根: " + report["source_isolation_root"] + "（只读；调用在一次性副本）")
    L.append("凭证复制: " + str(report["credentials_copied"]))
    L.append("素材    : 全部为虚构合成文本（见 fixtures/）")
    L.append("")
    for rec in report["scenarios"]:
        L.append("场景 %s: %s" % (rec["id"], rec["name"]))
        L.append("  证据等级: %s  结果: %s" % (rec.get("evidence_level"), rec["outcome"]))
        if rec.get("note"):
            L.append("  说明: " + rec["note"])
        L.append("  实际触碰夹具文件: %s" % (rec["touched_files"] or "（无）"))
        L.append("  越界读取: %s" % (rec["out_of_scope"] or "（无）"))
        L.append("  工具调用数: %d" % len(rec["tool_calls"]))
        L.append("  条款引用片段: 逐字命中 %d / 未命中 %d（疑似编造）"
                 % (len(rec["matched"]), len(rec["fabricated"])))
        L.append("  引号/引用行片段: 命中 %d / 未命中 %d（仅记录，不参与判定）"
                 % (len(rec.get("quote_hit", [])), len(rec.get("quote_miss", []))))
        for k, v in rec["checks"].items():
            L.append("    - %s = %s" % (k, v))
        L.append("")
    passed = sum(1 for r in report["scenarios"] if r["outcome"] == "passed")
    L.append("小结: 通过 %d；需复核/其他 %d；共 %d 个场景。"
             % (passed, len(report["scenarios"]) - passed, len(report["scenarios"])))
    (outdir / "retrieval-summary.txt").write_text("\n".join(L) + "\n", encoding="utf-8")
    print("\n".join(L))


if __name__ == "__main__":
    main()
