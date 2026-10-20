# OD-ARCH-031 证据索引：模型设置跳转 DSH（Part A）+ 对话流工作区（Part B）

> 执行日：2026-10-20。开工 HEAD：`82a6b26`（OD-ARCH-030 收口后的远端 HEAD）。
> 未改 `runtime/`、模型路由、凭证；未合并 `omniharness-ui` worktree；未提交真实材料或用户数据。

## Part A：设置页跳转 DSH + 刷新入口始终可见

### A.1 改了什么

| 层 | 文件 | 内容 |
| --- | --- | --- |
| 宿主 | `desktop/src-tauri/src/task_shell.rs` | 新增 `open_dsh_settings()`：固定白名单的 DSH Desktop 启动器（`gtk-launch <app-id>` → 回退 `xdg-open <desktop file>`），**不拼接 shell、不接受前端传入的命令/路径/URL、不读不写不打印任何凭证**；未安装时**明确报错**。另抽出 `find_dsh_desktop_file()` 供单测在不启动任何东西的前提下断言「装没装」。 |
| 宿主 | `desktop/src-tauri/src/lib.rs` | 注册命令 `task_open_dsh_settings`（**无参数**：前端只能请求这一个固定动作）。 |
| 前端 | `desktop/src/api.ts` | `shell.openDshSettings()` 绑定。 |
| 前端 | `desktop/src/SettingsPanel.tsx` | 「模型权限」区块顶部**常驻**「打开 DSH 设置（登记 key）」与「刷新端点信息」；端点事实读不出时给出明确说明与同样两个入口；`facts && !facts.configurable` 的旧提示合并为一条；底部只留「撤销全部模型权限」。**三类开关与默认全关语义未动。** |

### A.2 实测（真实窗口，用户会话，正式环境）

- 设置页按钮（AT-SPI 实读）：`['撤销全部模型权限', '刷新端点信息', '打开 DSH 设置（登记 key）', '刷新状态', '打开当前目录', '校验并保存', …]` — **两个入口在端点已识别时可见可点**。
- 点「打开 DSH 设置」→ **DSH Desktop 真的被拉起**（`/opt/DeepSeek Harness/deepseek-harness` 进程出现，窗口在用户桌面），
  界面给出提示与 DSH 数据根路径；宿主全程未读、未写、未打印任何凭证值。
- 端点**不可识别**状态：另起一个实例（空 DSH 数据根）复核 → 两个入口同样可见；权限开关在端点可识别时正常渲染（3 个）。
  **限制：** 本机 DSH 在没有自定义 baseURL 时会回落到 DeepSeek 默认端点，因此**自然构造不出「端点不可识别」的状态**；
  该分支是按代码条件渲染的（`!facts || factsError`），已由源码核对，**未做真实不可识别状态的截图**。
- 回归：Rust **144/144**（新增 1 条 `dsh_desktop_lookup_finds_only_existing_desktop_files`）、`npx tsc --noEmit` 0、
  `npm run build` 通过。

### A.3 需要 ARCHITECT 注意的**计划偏离**

计划原文写「宿主不新增命令」，但 Part A 第 1 条要求设置页能**拉起 DSH Desktop 的设置界面**——
Tauri v2 前端没有可用的外部启动器（`@tauri-apps/plugin-opener` 未安装），不做宿主命令就无法实现。
因此新增了**一个**最小命令 `task_open_dsh_settings`：无参数、固定白名单、不读写凭证、失败即明确报错。
这是**唯一**的新增宿主能力（不涉及协议、不涉及通信层），已在 REVIEW.md 标注，供复核确认。

## Part B：对话流工作区

（执行结果见本节与 REVIEW.md 同名小节。）
