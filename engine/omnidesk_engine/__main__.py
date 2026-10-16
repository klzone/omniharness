"""允许 python3 -m omnidesk_engine 启动 IPC-2 服务端。

见 docs/OmniDesk-Desktop-Engine-IPC.md §1.2 候选 A（stdio JSON-RPC）。
宿主（Rust）以本模块作为 Engine 子进程入口。
"""

from .serve import main

if __name__ == "__main__":
    raise SystemExit(main())
