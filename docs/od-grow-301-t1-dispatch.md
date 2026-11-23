# OD-GROW-301 T1 派工：引擎提交工作目录由宿主决定

## 任务编号与名称
OD-GROW-301 / T1：engine_submit_task 的 workspace_dir 缺省时由宿主决定

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（T1 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：desktop/src-tauri/src/lib.rs（engine_submit_task 及其测试区域）
- 明确不做的事：不改 engine/、runtime/、前端 api.ts（前端仍可显式传 workspace_dir）；不改超时与红线逻辑

## 背景事实（ARCHITECT 已核实，文件事实）
- lib.rs engine_submit_task（约 465 行起）当前行为：workspace_dir 为 None 时，params 完全不含 workspace_dir，直接 submit。
- 引擎侧契约：engine/omnidesk_engine/task_core.py SubmitTaskRequest.workspace_dir 是必填字符串；web_api_adapter.py submit_task 用 canonical_cwd 校验必须是绝对路径且真实存在，否则 INVALID_ARGUMENT。
- 宿主已有 prepare_workspace_dir()（lib.rs 约 9017 行）内部调用 task_shell::prepare_local_data_root()，返回本地数据根作为隔离工作目录；task_prepare_workspace_dir 命令目前供前端调用。
- 前端 WorkRecordLibrary 等先例是显式先 shell.prepareWorkspaceDir() 再传入。

## 单一动作
engine_submit_task 在 workspace_dir 为 None 或空字符串时，由宿主在提交链路内调用现有 prepare_local_data_root() 逻辑（与 prepare_workspace_dir 同一来源）取得目录并下发；显式传入时行为不变。失败时返回与现有准入错误同结构的 serde_json::Value 错误（engine_error_code=workspace_unavailable），不触达引擎。

## 实施要求
- 复用 task_shell::prepare_local_data_root()，不新建第二套目录逻辑。
- 缺省路径的创建发生在 with_engine_call 守卫内（或之前），确保失败可报错且不产生半途状态。
- 新增单测：缺省时使用数据根目录；显式传入不受影响；目录创建失败返回 workspace_unavailable 错误。

## 验证
- cd desktop/src-tauri && cargo test --lib --offline -- --test-threads=1 全绿且 passed 数不少于基线加新增
- 证据落 tests/verification/desktop/evidence/od-grow-301/t1/

## 硬约束
- 不修改 runtime/；不动模型路由与凭证；不提交、不推送、不打标签
- 同一失败测试修两次不过即停下报告

## 交付
- 改动文件清单 + 测试输出摘要
