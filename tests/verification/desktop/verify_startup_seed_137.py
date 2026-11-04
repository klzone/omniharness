#!/usr/bin/env python3
"""OD-ARCH-137 —— 首次启动先播种运行时 Profile（静态 + 本地 DSH 转储实测）。

0.2.8 在全新 `~/OmniDesk` 上打开「设置模型」会报：
`profile "omnidesk-base" does not exist`。根因是启动入口只装配资产、不播种
`dsh-home`，而模型设置不经过引擎启动。这里核对：

1. `build_state` 在装配资产之前调用播种；
2. 模型保存 / 列表 / 选择三条路径都先补齐 Profile；
3. 已有数据根只补缺失 Profile / Skill，不覆盖用户文件；
4. 若本机有 0.2.8+ 的 `.app`，用其自带 DSH 复现空 home 失败、seed home 成功。

证据等级：static + 本地运行（可选）。
运行：python3 tests/verification/desktop/verify_startup_seed_137.py
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
LIB = REPO / "desktop" / "src-tauri" / "src" / "lib.rs"
SUPERVISOR = REPO / "desktop" / "src-tauri" / "src" / "engine_supervisor.rs"
PREPARE = REPO / "scripts" / "prepare-runtime-assets.sh"
BASE_SAMPLE = REPO / "samples" / "profiles" / "omnidesk-base.profile.json"
APP = REPO / "desktop" / "src-tauri" / "target" / "release" / "bundle" / "macos" / "OmniDesk.app"
EVIDENCE = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-137" / "facts.json"

CASES: list[dict] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def between(text: str, start: str, end: str) -> str:
    at = text.find(start)
    if at < 0:
        return ""
    tail = text[at:]
    stop = tail.find(end, len(start))
    return tail if stop < 0 else tail[:stop]


def main() -> int:
    lib = LIB.read_text(encoding="utf-8")
    supervisor = SUPERVISOR.read_text(encoding="utf-8")
    prepare = PREPARE.read_text(encoding="utf-8")

    build = between(lib, "fn build_state()", "fn seed_dsh_home_on_startup")
    seed_fn = between(lib, "fn seed_dsh_home_on_startup", "/// 启动期装配 Profile 资产")
    save = between(lib, "async fn model_settings_save", "#[tauri::command]")
    listing = between(lib, "async fn task_list_model_routes", "/// 选定模型")
    selecting = between(lib, "fn task_set_model_route", "/// 改为跟随 DSH 设置")
    seed_home = between(supervisor, "fn seed_home_from_package", "/// 仓库根")

    record(
        "startup_seeds_before_assembly",
        "seed_dsh_home_on_startup(&paths, &config);" in build
        and build.find("seed_dsh_home_on_startup") < build.find("assemble_profile_assets_on_startup"),
        "build_state 先播种再装配",
    )
    record(
        "startup_seed_calls_seed_profile",
        "seed_profile_if_missing()" in seed_fn,
        "启动播种复用既有 seed_profile_if_missing",
    )
    record("startup_seed_logs_outcome", "seed=" in seed_fn, "日志记录 seed=created/ready/error")
    record("model_save_seeds_profile", "seed_profile_if_missing()" in save)
    record("model_list_seeds_profile", "seed_profile_if_missing()" in listing)
    record("model_set_seeds_profile", "seed_profile_if_missing()" in selecting)
    record(
        "seed_merge_preserves_existing",
        "if dst.exists()" in seed_home and "continue" in seed_home,
        "已有 Profile / Skill 不覆盖",
    )
    record(
        "prepare_seeds_both_profiles",
        "for profile in omnidesk-base web" in prepare,
        "seed 同时准备 omnidesk-base 与 web",
    )
    record("base_profile_sample_exists", BASE_SAMPLE.is_file())
    record(
        "regression_tests_exist",
        "build_state_seeds_bundled_home_before_model_settings" in lib
        and "seed_profile_copies_bundled_home_on_fresh_install" in supervisor
        and "seed_home_fills_missing_profiles_without_overwriting_existing" in supervisor,
    )

    # 可选：用本机已构建的 .app 自带 DSH 复现「空 home 失败 / seed home 成功」。
    node = shutil.which("node")
    cli = APP / "Contents" / "Resources" / "runtime" / "dsh-runtime" / "node_modules" / "@deepseek-ai" / "dsh" / "lib" / "bin.js"
    seed = APP / "Contents" / "Resources" / "runtime" / "dsh-home-seed"
    if cli.is_file() and seed.is_dir() and node:
        with tempfile.TemporaryDirectory(prefix="od137-dsh-") as tmp:
            root = Path(tmp)
            empty = root / "empty"
            empty.mkdir()
            failed = subprocess.run(
                [node, str(cli), "--profile", "omnidesk-base", "--dump-config"],
                env={"DSH_HOME": str(empty), "HOME": str(root), "PATH": "/usr/bin:/bin:/usr/local/bin"},
                capture_output=True,
                text=True,
                timeout=30,
            )
            record(
                "dsh_dump_empty_home_is_rejected",
                failed.returncode != 0 and "does not exist" in (failed.stderr + failed.stdout),
                "空 home 复现 0.2.8 的 profile 不存在",
            )

            home = root / "home"
            shutil.copytree(seed, home)
            seeded = subprocess.run(
                [node, str(cli), "--profile", "omnidesk-base", "--dump-config"],
                env={"DSH_HOME": str(home), "HOME": str(root), "PATH": "/usr/bin:/bin:/usr/local/bin"},
                capture_output=True,
                text=True,
                timeout=30,
            )
            record(
                "dsh_dump_seeded_home_succeeds",
                seeded.returncode == 0 and bool(seeded.stdout.strip()),
                "铺开 seed 后 DSH 配置转储成功",
            )
    else:
        print("[skip] 本机没有可用的 .app / DSH CLI / node，跳过 DSH 转储实测")

    failed = [c for c in CASES if not c["ok"]]
    report = {
        "plan": "OD-ARCH-137",
        "platform": sys.platform,
        "cases": CASES,
        "summary": {"total": len(CASES), "failed": len(failed)},
        "passed": not failed,
    }
    EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
    EVIDENCE.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    for case in CASES:
        mark = "pass" if case["ok"] else "fail"
        suffix = (" " + case["detail"]) if case["detail"] else ""
        print("[%s] %s%s" % (mark, case["name"], suffix))
    print("\n证据：%s" % EVIDENCE)
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
