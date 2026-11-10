"""允许 python3 -m omnidesk_engine 启动 IPC-2 服务端。

见 docs/OmniDesk-Desktop-Engine-IPC.md §1.2 候选 A（stdio JSON-RPC）。
宿主（Rust）以本模块作为 Engine 子进程入口。
"""

# OD-PROC-149b：进程标题显示为 omnidesk-engine（活动监视器 / ps 可见）。
# 只影响展示名，放在任何参数解析与 IO 之前；setproctitle 未安装或平台
# 不支持时静默跳过，启动流程不受影响。
try:
    import setproctitle

    setproctitle.setproctitle("omnidesk-engine")
except Exception:
    pass

from .serve import main

if __name__ == "__main__":
    raise SystemExit(main())
