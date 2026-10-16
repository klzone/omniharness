#!/usr/bin/env python3
"""
OmniDesk P1 —— 引擎接口审计的隔离副作用核对

目的：在跑完一次完整通道探测前后，对**原版 DSH 与用户数据**做元数据快照比对，
      证明本审计过程不写原版数据。

快照对象（只读元数据：路径、类型、大小、mtime，不读取文件内容）：
  * ~/.dsh（原版 DSH 数据根）
  * ~/.agents（原版 agents 根）
  * ~/OmniDesk（隔离运行根）
  * ~/YvanAI（外部数据中枢）
  * 仓库工作区（git status 层面的文件清单）

不做的事：不读凭证内容、不读 Session 内容、不比较文件字节。

用法：
  python3 tests/verification/engine-interface/probes/probe_isolation_sideeffects.py \
      --out tests/verification/engine-interface/evidence/isolation-sideeffects.json
"""
import argparse, json, os, subprocess, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _common import REPO, write_json  # noqa: E402

HOME = Path.home()
TARGETS = {
    "dsh_home": HOME / ".dsh",
    "agents_home": HOME / ".agents",
    "omnidesk_root": HOME / "OmniDesk",
    "yvanai_root": HOME / "YvanAI",
}


def snapshot(root: Path, limit=20000):
    if not root.exists():
        return {"exists": False, "entries": {}}
    entries = {}
    count = 0
    for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
        for name in list(dirnames) + list(filenames):
            p = Path(dirpath) / name
            rel = str(p.relative_to(root))
            try:
                st = p.lstat()
            except OSError:
                continue
            entries[rel] = [st.st_size, int(st.st_mtime)]
            count += 1
            if count >= limit:
                return {"exists": True, "entries": entries, "truncated": True}
    return {"exists": True, "entries": entries}


def diff(before, after):
    b, a = before.get("entries", {}), after.get("entries", {})
    added = sorted(set(a) - set(b))
    removed = sorted(set(b) - set(a))
    changed = sorted(k for k in (set(a) & set(b)) if a[k] != b[k])
    return {
        "added": added[:200], "added_count": len(added),
        "removed": removed[:200], "removed_count": len(removed),
        "changed": changed[:200], "changed_count": len(changed),
    }


def git_status(repo: Path):
    p = subprocess.run(["git", "status", "--porcelain=v1"], cwd=str(repo),
                       capture_output=True, text=True)
    return sorted(l for l in p.stdout.splitlines() if l.strip())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(REPO / "tests/verification/engine-interface/evidence/isolation-sideeffects.json"))
    ap.add_argument("--skip-probe", action="store_true")
    a = ap.parse_args()

    rep = {"evidence_level": "script（元数据快照比对）", "targets": {}}
    before = {k: snapshot(v) for k, v in TARGETS.items()}
    git_before = git_status(REPO)

    if not a.skip_probe:
        probe = REPO / "tests/verification/engine-interface/probes/probe_engine_channels.py"
        p = subprocess.run([sys.executable, str(probe)], cwd=str(REPO),
                           capture_output=True, text=True, timeout=600)
        rep["probe_rc"] = p.returncode

    after = {k: snapshot(v) for k, v in TARGETS.items()}
    git_after = git_status(REPO)

    for k in TARGETS:
        rep["targets"][k] = {"path": str(TARGETS[k]), **diff(before[k], after[k])}
    rep["repo_git_status_changed"] = git_before != git_after
    rep["repo_git_status_delta"] = sorted(set(git_after) ^ set(git_before))[:50]
    rep["conclusion"] = (
        "全部目标差异为 0 时，说明本审计的引擎探测对本机原版 DSH、隔离根与用户数据中枢无写入副作用。"
    )
    write_json(a.out, rep)
    print(json.dumps({k: {kk: vv for kk, vv in v.items() if kk.endswith('_count')}
                      for k, v in rep["targets"].items()}, ensure_ascii=False, indent=1))
    print("git_changed:", rep["repo_git_status_changed"])
    print("wrote", a.out)


if __name__ == "__main__":
    main()
