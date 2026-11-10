#!/usr/bin/env python3
"""OD-PROC-149 进程名归一验证脚本（分节，按任务追加）。

分节约定：
  * 149a 段：Windows exe 副本机制（omnidesk-node.exe / omnidesk-python.exe、
    回退开关、探针）——由 OD-PROC-149a 负责追加，未交付前为空。
  * 149b 段：macOS 进程标题（node --require shim + python setproctitle）。

用法：python3 tests/verification/desktop/verify_proc_alias_149.py
退出码 0 表示全部 PASS；任何 FAIL 返回 1。平台不支持的实跑探针记为 SKIP，不计失败。
"""
from __future__ import annotations

import ast
import importlib.util
import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
ENGINE = REPO / "engine"
SHIM = REPO / "scripts" / "proc_title_shim.cjs"
REQS = REPO / "scripts" / "python-requirements.txt"

CHECKS: list[tuple[str, str, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, "PASS" if ok else "FAIL", detail))


def skip(name: str, detail: str = "") -> None:
    CHECKS.append((name, "SKIP", detail))


# ---------------------------------------------------------------------------
# 149a 段：Windows exe 副本机制（OD-PROC-149a 追加，未交付前为空）
# ---------------------------------------------------------------------------
def check_149a() -> None:
    """Windows exe 副本机制：静态断言（实跑在 cargo 单测里）。"""
    rust = REPO / "desktop" / "src-tauri" / "src"
    alias_path = rust / "process_alias.rs"
    alias = _read(alias_path)
    lib = _read(rust / "lib.rs")
    supervisor = _read(rust / "engine_supervisor.rs")
    owner = _read(rust / "process_owner.rs")

    check("149a_module_exists", alias_path.is_file(), str(alias_path))
    check(
        "149a_alias_names",
        'PYTHON_ALIAS_FILE: &str = "omnidesk-python.exe"' in alias
        and 'NODE_ALIAS_FILE: &str = "omnidesk-node.exe"' in alias,
        "两个副本名",
    )
    check(
        "149a_copy_dir",
        'data_root.join("runtime").join("bin")' in alias,
        "<data_root>/runtime/bin",
    )
    check(
        "149a_marker_records_source",
        all(
            key in alias
            for key in ('"source"', '"len"', '"mtime_secs"', '"mtime_nanos"')
        )
        and "fn marker_matches" in alias,
        "标记记录源路径 + len + mtime",
    )
    check(
        "149a_refresh_on_mismatch",
        "if target.is_file() && marker_matches" in alias
        and "fn refresh_copy" in alias,
        "目标缺失或标记不符时重拷",
    )
    check(
        "149a_switch_env_off",
        'SWITCH_ENV: &str = "OMNIDESK_PROC_ALIAS"' in alias
        and '"off" | "false" | "0"' in alias,
        "回退开关",
    )
    check(
        "149a_windows_gated_passthrough",
        "if !cfg!(windows)" in alias and "Selection::passthrough(python)" in alias,
        "非 Windows 直接透传",
    )
    check(
        "149a_python_probe",
        "fn import_sys" in alias and "import sys" in alias and "encodings" in alias,
        "副本探针经 trait 注入，失败回退",
    )
    check(
        "149a_probe_failure_removes_copy",
        "remove_alias(&target)" in alias and "回退原名" in alias,
        "探针失败删副本并回退",
    )
    check(
        "149a_python_path_prefix",
        "prefixed_path(&prefix)" in alias and '"PATH".to_string()' in alias,
        "原解释器目录进子进程 PATH",
    )
    check(
        "149a_user_managed_skipped",
        '"OMNIDESK_PYTHON"' in alias and '"OMNIDESK_NODE"' in alias
        and "python_user_managed" in alias and "node_user_managed" in alias,
        "显式指定时不复制",
    )
    check(
        "149a_module_declared",
        "pub mod process_alias;" in lib,
        "lib.rs 模块声明",
    )
    check(
        "149a_engine_wiring",
        "process_alias::select_for_engine(&data_root, &self.config.python)" in supervisor
        and "spawn_with_stderr_log(\n            &alias.python" in supervisor
        and "envs.extend(alias.env.iter().cloned())" in supervisor,
        "config.python 经副本层转换并下发环境",
    )
    check(
        "149a_node_env_downstream",
        '"OMNIDESK_NODE".to_string()' in alias,
        "OMNIDESK_NODE 指向副本",
    )
    check(
        "149a_owner_alias_compat",
        "fn is_node_image" in owner
        and 'name.starts_with("omnidesk-node")' in owner
        and "c == '/' || c == '\\\\'" in owner,
        "回收判定兼容副本名与两种路径分隔符",
    )
    required_tests = (
        "copies_missing_target_and_records_marker",
        "refreshes_when_source_len_changes",
        "refreshes_when_marker_mismatches",
        "probe_failure_removes_copy_and_falls_back",
        "user_managed_paths_are_not_copied",
        "non_windows_is_passthrough",
        "node_copy_sets_env_override",
        "switch_values",
    )
    missing = [name for name in required_tests if f"fn {name}(" not in alias]
    check("149a_unit_tests_present", not missing, f"缺：{missing}" if missing else "8 项")


# ---------------------------------------------------------------------------
# 149b 段：macOS 进程标题
# ---------------------------------------------------------------------------
def _load_engine_module(name: str):
    """按仓库路径加载引擎子模块，避免依赖安装形态。"""
    if str(ENGINE) not in sys.path:
        sys.path.insert(0, str(ENGINE))
    return importlib.import_module(f"omnidesk_engine.{name}")


def _read(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError:
        return ""


def _module_body_first(node: ast.AST) -> list:
    return list(getattr(node, "body", []) or [])


def _contains_setproctitle_call(tree: ast.Module) -> bool:
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        if not isinstance(func, ast.Attribute) or func.attr != "setproctitle":
            continue
        if getattr(func.value, "id", None) != "setproctitle":
            continue
        if node.args and isinstance(node.args[0], ast.Constant):
            if node.args[0].value == "omnidesk-engine":
                return True
    return False


def _first_top_level_try_before_serve(tree: ast.Module) -> tuple[bool, bool]:
    """返回（setproctitle 的 try 块在 import .serve 之前, 该 try 捕获异常）。"""
    for index, node in enumerate(_module_body_first(tree)):
        if isinstance(node, ast.ImportFrom) and (node.module or "").endswith("serve"):
            return False, False
        if isinstance(node, ast.Try):
            has_call = _contains_setproctitle_call(ast.Module(body=node.body, type_ignores=[]))
            catches = any(h.type is not None for h in node.handlers)
            return has_call, catches
    return False, False


def check_149b() -> None:
    # 1) shim 文件与内容
    shim_text = _read(SHIM)
    check("149b_shim_exists", SHIM.is_file(), str(SHIM))
    check(
        "149b_shim_sets_title",
        'process.title = "omnidesk-dsh"' in shim_text,
        "shim 内容含 process.title = \"omnidesk-dsh\"",
    )

    # 2) 适配层：darwin 插入 --require，其他平台不插入
    try:
        web_mod = _load_engine_module("web_api_adapter")
        acp_mod = _load_engine_module("acp_adapter")
        owner_mod = _load_engine_module("process_owner")
        loaded_err = ""
    except Exception as exc:  # noqa: BLE001 - 验证脚本需要给出原因
        web_mod = acp_mod = owner_mod = None
        loaded_err = repr(exc)
    check("149b_engine_modules_import", web_mod is not None, loaded_err)

    if web_mod is not None:
        real_platform = sys.platform
        try:
            sys.platform = "darwin"
            args_darwin = list(acp_mod.proc_title_shim_args())
            sys.platform = "win32"
            args_win = list(acp_mod.proc_title_shim_args())
            sys.platform = "linux"
            args_linux = list(acp_mod.proc_title_shim_args())
        finally:
            sys.platform = real_platform

        check(
            "149b_shim_args_darwin",
            args_darwin[:1] == ["--require"]
            and len(args_darwin) == 2
            and args_darwin[1].endswith(os.path.join("scripts", "proc_title_shim.cjs")),
            repr(args_darwin),
        )
        check("149b_shim_args_windows_empty", args_win == [], repr(args_win))
        check("149b_shim_args_linux_empty", args_linux == [], repr(args_linux))

        # 两处 for_dsh 的 cmd 构造（实跑构造，非文本匹配）
        for label, adapter_cls in (
            ("web", getattr(web_mod, "WebApiAdapter", None)),
            ("acp", getattr(acp_mod, "AcpEngineAdapter", None)),
        ):
            if adapter_cls is None:
                check(f"149b_{label}_cmd_shim", False, "找不到适配器类")
                continue
            try:
                adapter = adapter_cls.for_dsh("/tmp/dsh/lib/bin.js", cwd="/tmp", profile=label)
                cmd = list(adapter._command)
            except Exception as exc:  # noqa: BLE001
                check(f"149b_{label}_cmd_shim", False, repr(exc))
                continue
            if sys.platform == "darwin":
                expect = [args_darwin[0], args_darwin[1], "/tmp/dsh/lib/bin.js"]
                ok = cmd[1:4] == expect and "--profile" in cmd
                detail = repr(cmd[1:5])
            else:
                ok = cmd[:2] == [cmd[0], "/tmp/dsh/lib/bin.js"]
                detail = repr(cmd[:3])
            check(f"149b_{label}_cmd_shim", ok, detail)
            # 插了 --require 之后 process_owner 仍能认出这是引擎命令
            if owner_mod is not None:
                check(
                    f"149b_{label}_cmd_recognized_owner",
                    bool(owner_mod.is_engine_cmdline(cmd)),
                    "is_engine_cmdline 命中（any arg endswith bin.js）",
                )

        # 静态兜底：源码里确有 darwin 分支（防止 helper 被改成恒插入）
        acp_src = _read(ENGINE / "omnidesk_engine" / "acp_adapter.py")
        check(
            "149b_source_platform_guard",
            'sys.platform != "darwin"' in acp_src and "proc_title_shim_args" in acp_src,
            "acp_adapter 内含 darwin 判定",
        )
        web_src = _read(ENGINE / "omnidesk_engine" / "web_api_adapter.py")
        check(
            "149b_web_uses_helper",
            "resolve_node(), *proc_title_shim_args(), dsh_bin" in web_src,
            "web 侧复用同一 helper 插入 shim",
        )

    # 3) __main__ 的 setproctitle 段
    main_path = ENGINE / "omnidesk_engine" / "__main__.py"
    try:
        tree = ast.parse(_read(main_path))
        guarded, catches = _first_top_level_try_before_serve(tree)
        has_call = _contains_setproctitle_call(tree)
    except SyntaxError as exc:
        tree = None
        guarded = catches = has_call = False
        check("149b_main_parses", False, repr(exc))
    else:
        check("149b_main_parses", True, "")
    check("149b_main_sets_title", has_call, "调用 setproctitle('omnidesk-engine')")
    check(
        "149b_main_first_and_guarded",
        guarded and catches,
        "try 块位于 import .serve 之前且捕获异常",
    )

    # 4) 依赖清单
    reqs = _read(REQS)
    req_lines = [ln.strip() for ln in reqs.splitlines()]
    check(
        "149b_requirements_setproctitle",
        any(ln.startswith("setproctitle") for ln in req_lines),
        "python-requirements.txt 含 setproctitle",
    )
    check(
        "149b_requirements_marked",
        any("OD-PROC-149b" in ln for ln in req_lines),
        "依赖行附任务号注释",
    )

    # 5) 实跑探针：node 真的把标题改成 omnidesk-dsh
    node = shutil.which("node")
    if sys.platform not in ("darwin", "linux"):
        skip("149b_node_probe", "当前平台不适用（Windows 任务管理器忽略标题）")
    elif node is None:
        skip("149b_node_probe", "PATH 里没有 node")
    elif not SHIM.is_file():
        skip("149b_node_probe", "shim 文件缺失")
    else:
        try:
            proc = subprocess.run(
                [node, "--require", str(SHIM), "-e", "console.log(process.title)"],
                capture_output=True,
                text=True,
                timeout=20,
            )
            out = (proc.stdout or "").strip().splitlines()
            title = out[-1] if out else ""
            check(
                "149b_node_probe",
                proc.returncode == 0 and title == "omnidesk-dsh",
                f"rc={proc.returncode} title={title!r}",
            )
        except (OSError, subprocess.SubprocessError) as exc:
            check("149b_node_probe", False, repr(exc))


def main() -> int:
    check_149a()
    check_149b()
    failed = 0
    for name, status, detail in CHECKS:
        if status == "FAIL":
            failed += 1
        line = f"[{status}] {name}"
        if detail:
            line += f" — {detail}"
        print(line)
    total = len(CHECKS)
    passed = sum(1 for _, status, _ in CHECKS if status == "PASS")
    skipped = sum(1 for _, status, _ in CHECKS if status == "SKIP")
    print(f"\n{passed}/{total} PASS, {failed} FAIL, {skipped} SKIP")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
