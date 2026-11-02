"""GUI 环境下的 node 可执行文件解析（与 Rust 侧 platform::resolve_node 同一口径）。

背景：macOS 上从 Finder/Dock 启动的应用 PATH 只有 /bin:/usr/bin:/usr/ucb:/usr/local/bin，
homebrew 的 node（/opt/homebrew/bin）不在其中。Rust 宿主侧已修过同类问题，
但引擎适配层用裸 "node" 启动 DSH，导致「引擎子进程在运行（未初始化）」——
DSH 进程永远起不来，初始化也就永远完不成。

解析顺序（与 Rust 侧一致）：
  1. OMNIDESK_NODE 环境变量（用户显式指定，优先级最高）；
  2. 当前 PATH 里的 node（开发态/终端启动时命中）；
  3. 常见安装位置的固定候选（GUI 启动时命中）。
"""

from __future__ import annotations

import os
import shutil
from pathlib import Path

# 覆盖 macOS（homebrew 两种安装根 + 系统）与 Linux（包管理器常见位置）。
_CANDIDATES = (
    "/opt/homebrew/bin/node",
    "/usr/local/bin/node",
    "/usr/bin/node",
    "/usr/local/sbin/node",
    "/snap/bin/node",
)


def resolve_node() -> str:
    """返回可用的 node 绝对路径；找不到时返回 "node" 让报错保持原有形态。"""
    env_value = (os.environ.get("OMNIDESK_NODE") or "").strip()
    if env_value:
        return env_value
    found = shutil.which("node")
    if found:
        return found
    for candidate in _CANDIDATES:
        if Path(candidate).is_file():
            return candidate
    return "node"
