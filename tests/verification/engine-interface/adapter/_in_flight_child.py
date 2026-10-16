#!/usr/bin/env python3
"""
OmniDesk P1 第三步 —— 「客户端被强制退出」子进程夹具（E07 / E10 专用）。

用途：在独立进程中建立一次**在途任务**，把回执写入标记文件后保持存活，
      由父进程对其进程组发送 SIGKILL，从而精确模拟「适配层/客户端被强杀」：
      * 适配层没有机会写终止记录；
      * 只留下创建任务时的最后一份本地快照（非终态）；
      * 引擎进程随进程组一同被杀，不残留孤儿进程。

安全边界：全部使用一次性 /tmp 数据根与合成端点，不涉及任何凭证与用户数据。
"""

from __future__ import annotations

import json
import os
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
sys.path.insert(0, str(REPO / "engine"))
sys.path.insert(0, str(HERE))

from omnidesk_engine import AcpEngineAdapter, SubmitTaskRequest  # noqa: E402


def main(argv):
    args = {}
    for item in argv:
        if "=" in item:
            key, value = item.split("=", 1)
            args[key] = value
    overlay = args["overlay"]
    dsh_bin = args["dsh_bin"]
    state_dir = args["state"]
    cwd = args["cwd"]
    marker = args["marker"]
    prompt = args["prompt"]

    adapter = AcpEngineAdapter.for_dsh(
        dsh_bin, cwd=cwd, profile="acp", patch_files=[overlay],
        env=dict(os.environ), state_dir=state_dir,
        init_timeout=45, session_timeout=45, default_timeout=300,
    )
    adapter.initialize()
    receipt = adapter.submit_task(SubmitTaskRequest(
        intent="synthetic-writing-task", workspace_dir=cwd, prompt_text=prompt))
    Path(marker).write_text(json.dumps({
        "omni_task_id": receipt.omni_task_id,
        "engine_session_id": receipt.engine_session_id,
        "status": receipt.status.value,
        "child_pid": os.getpid(),
    }, ensure_ascii=False), encoding="utf-8")
    # 保持存活，让任务停留在在途状态；父进程随后强杀整个进程组。
    while True:
        time.sleep(1.0)


if __name__ == "__main__":
    main(sys.argv[1:])

