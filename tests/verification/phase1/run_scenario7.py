#!/usr/bin/env python3
"""
OmniDesk Phase 1 —— 场景 7（隔离 Skill 同步加载）离线验证器

目的：证明「修改 Skill 源 -> 显式同步 -> 隔离实例按 DSH 真实发现机制加载新版本」，
      全程只使用 /tmp 下的一次性可丢弃副本与一次性隔离根，绝不触碰：
        * 仓库真实 skills/（只读复制）
        * ~/OmniDesk 隔离根
        * ~/.dsh / ~/.agents（原版 DSH）

证据等级：脚本 / 隔离环境实测（DSH 的真实 FileSystemSkillProvider 发现与正文加载），
          不是模型输出，也不证明模型会遵守 Skill 内容。

用法：
  python3 tests/verification/phase1/run_scenario7.py --out tests/verification/phase1/evidence
"""
import argparse, json, os, shutil, subprocess, sys, tempfile, time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HOME = Path.home()

PROBE = Path(__file__).resolve().parent / "probe_provider.mjs"
MARKER_OLD = "<!-- SCENARIO7-MARKER: v0 -->"
MARKER_NEW = "<!-- SCENARIO7-MARKER: v1 -->"

def run(cmd, cwd=None, env=None, timeout=300):
    full = os.environ.copy()
    if env:
        full.update(env)
    p = subprocess.run(cmd, cwd=cwd, env=full, timeout=timeout,
                       capture_output=True, text=True)
    return {"cmd": cmd, "rc": p.returncode, "stdout": p.stdout, "stderr": p.stderr}

def probe_skills(dsh_home: Path, agents_home: Path, cwd: Path, marker: str):
    cfg = {"dshHome": str(dsh_home), "agentsHome": str(agents_home),
           "cwd": str(cwd), "marker": marker, "includeDefaultRoots": True}
    entry = (REPO / "runtime" / "dsh-runtime" / "node_modules" / "@deepseek-ai"
             / "dsh-skill-filesystem" / "lib" / "index.js")
    env = {"OMNI_SKILL_FS_ENTRY": str(entry)}
    return run(["node", str(PROBE), json.dumps(cfg)], env=env)

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    outdir = Path(args.out).resolve()
    outdir.mkdir(parents=True, exist_ok=True)

    result = {"generated_at": time.strftime("%Y-%m-%d %H:%M:%S%z"),
              "isolation": "一次性 /tmp 副本 + 一次性隔离根；仓库与 ~/OmniDesk 只读",
              "steps": {}}
    scene = Path(tempfile.mkdtemp(prefix="omni_scen7_"))
    try:
        repo = scene / "repo"
        iso = scene / "iso"
        (iso / "runtime" / "dsh-home").mkdir(parents=True)
        (iso / "runtime" / "agents-home").mkdir(parents=True)
        # 与维护者实机隔离根一致的工作态：skills/ 目录已存在。
        # 注意：scripts/sync-skills.sh 不会创建该目录（见验证报告中的健壮性缺口）。
        (iso / "runtime" / "dsh-home" / "skills").mkdir(parents=True, exist_ok=True)
        # 可丢弃副本：只复制脚本/技能/资产/Profile
        for sub in ["skills", "templates", "knowledge", "scripts", "profiles"]:
            shutil.copytree(REPO / sub, repo / sub)
        result["steps"]["disposable_copy"] = {"repo": str(repo), "iso": str(iso),
                                              "note": "仓库与 ~/OmniDesk 未被写入"}

        dsh_home = iso / "runtime" / "dsh-home"
        agents_home = iso / "runtime" / "agents-home"
        env = {"OMNIDESK_ROOT": str(iso)}

        # 1) 初始状态：写入旧标记，同步到一次性隔离根
        mw_src = repo / "skills" / "material-writing" / "SKILL.md"
        body = mw_src.read_text()
        if MARKER_NEW in body:
            body = body.replace(MARKER_NEW, MARKER_OLD)
        if MARKER_OLD not in body:
            body = body.rstrip() + "\n\n" + MARKER_OLD + "\n"
        mw_src.write_text(body)
        result["steps"]["sync_v0"] = run(["bash", "scripts/sync-skills.sh"],
                                         cwd=str(repo), env=env)

        # 2) 用真实 DSH 发现机制读取：应看到 v0
        result["steps"]["discover_v0"] = probe_skills(dsh_home, agents_home, repo, MARKER_OLD)

        # 3) 修改源标记为 v1，再次显式同步
        mw_src.write_text(mw_src.read_text().replace(MARKER_OLD, MARKER_NEW))
        result["steps"]["sync_v1"] = run(["bash", "scripts/sync-skills.sh"],
                                         cwd=str(repo), env=env)

        # 4) 再次发现：应看到 v1（证明隔离实例加载了新版本）。
        #    同时对旧标记做一次探测，确认正文确实被替换而非两份并存。
        result["steps"]["discover_v1"] = probe_skills(dsh_home, agents_home, repo, MARKER_NEW)
        result["steps"]["discover_v1_oldmarker"] = probe_skills(
            dsh_home, agents_home, repo, MARKER_OLD)

        # 5) --check 应报告无漂移（源与隔离副本一致）
        result["steps"]["check_after_sync"] = run(["bash", "scripts/sync-skills.sh", "--check"],
                                                  cwd=str(repo), env=env)

        # 汇总判定
        def parse(step):
            try:
                return json.loads(step["stdout"])
            except Exception:
                return None
        d0 = parse(result["steps"]["discover_v0"])
        d1 = parse(result["steps"]["discover_v1"])
        d1o = parse(result["steps"]["discover_v1_oldmarker"])
        mw0 = next((s for s in (d0 or {}).get("skills", []) if s["name"] == "material-writing"), None)
        mw1 = next((s for s in (d1 or {}).get("skills", []) if s["name"] == "material-writing"), None)
        mw1o = next((s for s in (d1o or {}).get("skills", []) if s["name"] == "material-writing"), None)
        result["verdict"] = {
            "discovered_count_v0": (d0 or {}).get("count"),
            "discovered_count_v1": (d1 or {}).get("count"),
            "v0_body_has_marker": bool(mw0 and mw0.get("bodyHasMarker")),
            "v1_body_has_new_marker": bool(mw1 and mw1.get("bodyHasMarker")),
            "v1_body_still_has_old_marker": bool(mw1o and mw1o.get("bodyHasMarker")),
            "v1_skill_source": (mw1 or {}).get("source"),
            "v1_skill_path": (mw1 or {}).get("path"),
            "check_rc": result["steps"]["check_after_sync"]["rc"],
        }
    finally:
        shutil.rmtree(scene, ignore_errors=True)

    (outdir / "scenario7-evidence.json").write_text(
        json.dumps(result, ensure_ascii=False, indent=2))

    v = result["verdict"]
    lines = [
        "OmniDesk Phase 1 场景 7（隔离 Skill 同步加载）离线验证",
        "生成时间: " + result["generated_at"],
        "隔离方式: " + result["isolation"],
        "",
        "旧标记发现结果: 发现 %s 个 Skill；material-writing 正文含旧标记 = %s" %
            (v["discovered_count_v0"], v["v0_body_has_marker"]),
        "同步后再发现:   发现 %s 个 Skill；material-writing 正文含新标记 = %s" %
            (v["discovered_count_v1"], v["v1_body_has_new_marker"]),
        "旧标记是否仍在: %s（False = 已被新版本替换）" % v["v1_body_still_has_old_marker"],
        "发现来源/路径:  source=%s path=%s" % (v["v1_skill_source"], v["v1_skill_path"]),
        "同步后 --check 退出码: %s（0=无漂移）" % v["check_rc"],
        "",
        "判定: 隔离实例是否加载新版本 Skill = %s" %
            ("是（DSH 真实发现机制读到新正文）" if v["v1_body_has_new_marker"] else "否/未证明"),
    ]
    (outdir / "scenario7-summary.txt").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))

if __name__ == "__main__":
    main()
