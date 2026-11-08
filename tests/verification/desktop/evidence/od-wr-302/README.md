# OD-WR-302 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 起点库单测：606 passed（OD-WR-301 收口）
- 交付后库单测：614 passed / 0 failed（新增 8 项）
- 范围说明：按用户本轮任务卡约束，未修改 STATUS.md / REVIEW.md / PLANS.md。

## 改动文件

- desktop/src-tauri/src/lib.rs：15 个 IPC 命令（工作记录 CRUD/列表、任务 CRUD/列表/状态迁移/进展、候选列表/确认/拒绝）、工作根解析、红线与掩码统一入口、对应单测。
- desktop/src/api.ts：host 对象新增 15 个调用方法。
- desktop/src/types.ts：工作记录、任务、候选与回执类型。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --lib | 0 | 614 passed / 0 failed（exit-lib.txt） |
| npx tsc --noEmit | 0 | 无输出（tsc.log 为 0 字节，exit-tsc.txt） |
| npm --prefix desktop run build | 0 | vite 构建成功（exit-build.txt） |

原始输出：cargo-test-lib.log、tsc.log、npm-build.log。

## 安全接线断言

- 记录标题/正文、任务标题、进展、状态备注、候选确认标题与备注在写入前统一过 red_line_scan_with；命中即拒绝且错误不回显原文（单测断言错误串不含命中内容）。
- 身份证/手机号在落盘前经 mask_privacy 替换，回执只给类别与数量。
- 候选确认前不进入正式任务清单；确认后才生成任务文件（单测覆盖 record -> candidate -> task_list 全链路）。

## 已知限制

- working_root 设置字段未加入 settings.v1（settings.rs 不在本任务允许文件）；本轮用 OMNIDESK_WORK_ROOT 覆盖或默认 $HOME/OmniDesk/work，边界校验复用成果目录规则。缺省目录的正式设置留给后续任务。
- 未实现候选暂存 IPC：候选只能由宿主内部（OD-WR-304）产出，避免前端注入。
