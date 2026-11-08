# OD-WR-309 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 授权：用户在本轮显式授权执行 309。

## 改动文件

- 新增 desktop/src-tauri/src/worklog_migrate.rs：周文件解析、预览、迁移、回滚与 8 项单测。
- 修改 desktop/src-tauri/src/lib.rs：worklog_migrate_preview / worklog_migrate_apply IPC。
- 修改 desktop/src/api.ts、desktop/src/types.ts：预览/执行方法与迁移类型。
- 修改 desktop/src/SettingsPanel.tsx：工作记录临时层下的「迁移旧周文件」入口（先预览、勾选确认后才移动）。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --tests | 0 | 13 个测试目标全 ok；lib 653 passed / 0 failed |
| npx tsc --noEmit | 0 | 无输出 |
| npm --prefix desktop run build | 0 | vite 构建成功 |

原始输出：cargo-test-tests.log、tsc.log、npm-build.log、exit-*.txt。

## 真实迁移（用户授权的一次性动作）

- 库根：/Users/yvan/OmniDesk/库；工作根：/Users/yvan/OmniDesk/work。
- 源文件：工作记录/2026-W45.md（1127 字节，4 条记录）。
- 结果：新建 4 条记录到 work/工作记录/；原文件移动为 work/周文件存档/2026-W45.md，1127 字节，SHA-256 6cffc44f076ba0669abd1ed167f09a976bc5de9d7fdf469d94105ce15778a4dd。
- 源目录剩余 8 个 .bak 文件，未被读取、移动或删除。
- 迁移后预览 entry_count = 0；源文件已不在，重跑是空操作。
- 310 落地后，4 条迁移记录按周文件存档里的去向补写了 destination 字段（全部匹配）。

## 回滚与幂等用例（临时目录）

- 解析遇到无法识别的内容：整体失败，零记录、零移动。
- 存档目标已存在：预检失败，零记录。
- 存档目录不可创建：建完记录后失败，记录被删除、源文件留在原位。
- 成功后重跑：0 记录、0 归档。
- CRLF 周文件：解析正常，归档字节原样。

## 已知限制

- 崩溃中断（进程被强杀）不在回滚覆盖范围；错误路径回滚已实现。
- 仅迁移 YYYY-Www.md；.bak 与其他文件跳过并在预览中列出。
- 真实迁移已执行一次，不需要再跑；UI 入口保留给其他数据根或重装环境。
