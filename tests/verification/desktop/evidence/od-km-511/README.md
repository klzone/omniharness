# OD-KM-511 关联视图扩展 — 本轮证据

证据等级：静态源码检查 + 前端构建/类型检查。未开真实窗口；测试数据未进入仓库。

| 命令 | 结果 | 原始输出 |
| --- | --- | --- |
| cd desktop && npm --prefix . run build | exit 0（仅既有 >500kB chunk 提示） | npm-build.txt / .exit |
| cd desktop && npx tsc --noEmit | exit 0，无输出 | tsc-noemit.txt / .exit |
| python3 tests/verification/desktop/verify_linkage_ui_511.py | 25/25 通过，exit 0 | verify-linkage-ui-511.txt / .exit |

JSON 证据：../../verify_linkage_ui_511.json（passed=true，25 项，0 失败）。

说明：首次构建曾撞上并行 OD-KM-512 正在编辑的 ProjectBoard.tsx 中途状态
（未使用导入与未定义类型），对方文件落定后复跑通过；本轮未改动该文件。
