#!/usr/bin/env python3
"""
OmniDesk Phase 1 验证收口 —— 只读副作用 + 隔离 Skill 加载 离线验证器

设计原则：
  * 只读：本脚本对仓库、~/OmniDesk、~/YvanAI、~/.dsh、~/.agents 只做快照比对，
          绝不写入其中任何一个；所有会写入的动作都发生在 /tmp 下的一次性沙箱。
  * 不触碰原版 DSH：~/.dsh 与 ~/.agents 仅做元数据快照（大小/mtime），
          且排除会持续变化的 sessions/ 子树；不读取凭证、不读取会话内容。
  * 证据分级：本脚本产生的是「脚本 / 隔离环境实测」级证据，不是模型输出。

用法：
  python3 tests/verification/phase1/run_verification.py --out tests/verification/phase1/evidence
"""
import argparse, hashlib, json, os, shutil, subprocess, sys, time, tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HOME = Path.home()

SNAPSHOT_ROOTS = {
    "repo": REPO,
    "omnidesk_root": HOME / "OmniDesk",
    "yvanai": HOME / "YvanAI",
    "stock_dsh": HOME / ".dsh",
    "stock_agents": HOME / ".agents",
}

def tree_meta(root: Path, skip_sessions: bool) -> dict:
    """返回 {relpath: [size, mtime]} 元数据；不读取文件内容。"""
    out = {}
    if not root.exists():
        return {"__missing__": True}
    for dirpath, dirnames, filenames in os.walk(root):
        if skip_sessions and (os.sep + "sessions") in dirpath:
            continue
        if dirpath.endswith(os.sep + ".git") or (os.sep + ".git" + os.sep) in dirpath:
            dirnames[:] = []
            continue
        for fn in filenames:
            p = os.path.join(dirpath, fn)
            rel = os.path.relpath(p, root)
            try:
                st = os.lstat(p)
                out[rel] = [st.st_size, round(st.st_mtime, 3)]
            except OSError:
                out[rel] = ["ERR"]
    return out

def snapshot() -> dict:
    snap = {}
    for name, root in SNAPSHOT_ROOTS.items():
        snap[name] = tree_meta(root, skip_sessions=(name == "stock_dsh"))
    return snap

def diff_snap(before: dict, after: dict) -> dict:
    report = {}
    for name in before:
        b, a = before[name], after[name]
        if b.get("__missing__") or a.get("__missing__"):
            report[name] = {"missing_before": bool(b.get("__missing__")),
                            "missing_after": bool(a.get("__missing__")),
                            "changed": 0, "added": 0, "removed": 0}
            continue
        bs, as_ = set(b), set(a)
        changed = sorted(k for k in (bs & as_) if b[k] != a[k])
        added = sorted(as_ - bs)
        removed = sorted(bs - as_)
        report[name] = {"changed": len(changed), "added": len(added),
                        "removed": len(removed),
                        "changed_items": changed[:20],
                        "added_items": added[:20],
                        "removed_items": removed[:20]}
    return report

def run(cmd, cwd=None, env=None, timeout=300):
    full_env = os.environ.copy()
    if env:
        full_env.update(env)
    p = subprocess.run(cmd, cwd=cwd, env=full_env, timeout=timeout,
                       capture_output=True, text=True)
    return {"cmd": cmd, "cwd": cwd, "rc": p.returncode,
            "stdout": p.stdout, "stderr": p.stderr}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    outdir = Path(args.out).resolve()
    outdir.mkdir(parents=True, exist_ok=True)

    evidence = {"generated_at": time.strftime("%Y-%m-%d %H:%M:%S%z"),
                "repo": str(REPO), "steps": {}}

    # ---------------- 快照（before） ----------------
    ev = {}
    before = snapshot()
    ev["snapshot_before"] = {k: ({"missing": True} if v.get("__missing__") else len(v))
                             for k, v in before.items()}

    # ---------------- 正常路径：真实环境 setup.sh --check ----------------
    ev["normal_check"] = run(["bash", "scripts/setup.sh", "--check"], cwd=str(REPO))
    ev["bootstrap_check"] = run(["bash", "scripts/bootstrap-runtime.sh", "--check"], cwd=str(REPO))
    ev["sync_check"] = run(["bash", "scripts/sync-skills.sh", "--check"], cwd=str(REPO))

    # ---------------- 失败路径：一次性沙箱 ----------------
    sandbox_root = Path(tempfile.mkdtemp(prefix="omni_calib_"))
    sandbox_repo = sandbox_root / "sandbox"
    isolated_root = sandbox_root / "isolated_root"   # 故意不存在
    try:
        # 只复制脚本 + Skills，不复制 runtime（制造「底座未安装」）
        (sandbox_repo / "scripts").mkdir(parents=True)
        for f in ["setup.sh", "bootstrap-runtime.sh", "sync-skills.sh"]:
            shutil.copy2(REPO / "scripts" / f, sandbox_repo / "scripts" / f)
        shutil.copytree(REPO / "skills", sandbox_repo / "skills")
        ev["fail_case"] = run(["bash", "scripts/setup.sh", "--check"],
                              cwd=str(sandbox_repo),
                              env={"OMNIDESK_ROOT": str(isolated_root)})
        ev["fail_case"]["isolated_root_created"] = isolated_root.exists()
        ev["fail_case"]["isolated_root"] = str(isolated_root)
        ev["fail_case"]["reported_runtime_fail"] = "运行底座 检查失败" in ev["fail_case"]["stderr"]
        ev["fail_case"]["reported_sync_fail"] = "Skill 同步状态 检查失败" in ev["fail_case"]["stderr"]
    finally:
        shutil.rmtree(sandbox_root, ignore_errors=True)

    # ---------------- 快照（after）+ 差异 ----------------
    after = snapshot()
    ev["snapshot_after"] = {k: ({"missing": True} if v.get("__missing__") else len(v))
                            for k, v in after.items()}
    ev["readonly_diff"] = diff_snap(before, after)

    evidence["steps"]["readonly_check"] = ev

    (outdir / "readonly-check-evidence.json").write_text(
        json.dumps(evidence, ensure_ascii=False, indent=2))

    # 人类可读摘要
    lines = []
    lines.append("OmniDesk Phase 1 只读副作用验证（脚本/隔离环境实测级证据）")
    lines.append("生成时间: " + evidence["generated_at"])
    lines.append("")
    lines.append("[正常路径] bash scripts/setup.sh --check")
    lines.append("  退出码: %d" % ev["normal_check"]["rc"])
    lines.append("  尾部输出: " + ev["normal_check"]["stdout"].strip().splitlines()[-1])
    lines.append("")
    lines.append("[失败路径] 一次性沙箱（无底座 + 无隔离根）")
    lines.append("  退出码: %d" % ev["fail_case"]["rc"])
    lines.append("  报告底座失败: %s" % ev["fail_case"]["reported_runtime_fail"])
    lines.append("  报告 Skill 失败: %s" % ev["fail_case"]["reported_sync_fail"])
    lines.append("  缺失的隔离根被创建: %s" % ev["fail_case"]["isolated_root_created"])
    lines.append("")
    lines.append("[只读副作用] 快照前后差异（元数据级；~/.dsh/sessions 子树已排除）")
    for name, d in ev["readonly_diff"].items():
        lines.append("  %-14s changed=%d added=%d removed=%d" %
                     (name, d["changed"], d["added"], d["removed"]))
    (outdir / "readonly-check-summary.txt").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))

if __name__ == "__main__":
    main()

