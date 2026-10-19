# OD-ARCH-010 证据索引：准入闸门与离线工作场景

- 采集时间：2026-10-18 09:54–10:05（Asia/Shanghai）
- 采集者：BUILDER
- 对应计划：[PLANS.md](../../../../../PLANS.md) 的 **OD-ARCH-010**
- 工作区：`/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`
- 结论范围：**只证明准入关闭时的宿主拒绝与三个纯合成本地流程可走通**。模型撰写步骤受准入阻塞，未验证。

---

## 1. 场景与结果

| 场景 | 输入 | 实际输出 | 证据等级 |
| --- | --- | --- | --- |
| 周记录 → 周报 | `samples/workflows/synthetic-weekly.txt`（4 条虚构事项） | 事项 4 / 已完成 2 / 待完成 1 / 受阻 1；标注「需人工确认」两条 | 真实窗口运行 + Rust 运行 |
| 一般文案结构 | 目的 `Synthetic review`、受众 `Fictional internal reviewers`，未填模板与事实 | 模板=`未使用模板（没有与用途匹配的模板，按用途组织）`；首节为「【待确认】事实依据缺失，暂不能形成初稿」 | 真实窗口运行 + Rust 运行 |
| 项目跟进汇总 | `samples/workflows/synthetic-project.txt`，判定日 2026-10-15 | 事项 4 / done 1 / in_progress 1 / blocked 1 / not_started 1 / 逾期 2 | 真实窗口运行 + Rust 运行 |
| 人工确认后保存 | 周报场景勾选确认并保存 | 写入仓库外 `/tmp/<session>/local-data/offline-weekly-report-20261018-015939.md`，986 字节 | 真实窗口运行 |
| 未确认即保存 | 未勾选确认复选框 | 宿主拒绝，提示需人工确认；未写入任何文件 | Rust 运行（单元测试） |
| 红线拒绝 | 合成占位文本「内部资料占位（纯虚构）」 | 拒绝处理，只回显命中类别与计数，不回显原文，且不生成结果 | 真实窗口运行 + Rust 运行 |
| 准入拒绝 | 直接调用宿主描述/提交/目录准备/引擎启动入口 | 全部拒绝，未创建目录、未启动子进程、未提交任务 | Rust 运行（12 项宿主集成 + 5 项单元测试） |

三个场景在窗口内都显示 `模型步骤：受准入阻塞，未验证：本次未调用任何模型`，没有把模型步骤显示为已生成。

---

## 2. 窗口运行环境

| 项 | 值 |
| --- | --- |
| 显示器 | 一次性 `Xvfb :95`（1320×900×24），不占用用户桌面 |
| 窗口 | `OmniDesk Discipline`，1280×820，正常渲染（distinct colors 2998） |
| 前端 | `vite preview` on `127.0.0.1:5273`，加载 `desktop/dist` |
| 应用 | `desktop/src-tauri/target/debug/omnidesk-desktop` |
| 数据根 | 一次性 `/tmp/od010-*/{dsh-home,agents-home,app-data,engine-state,local-data}`，`HOME` 也指向该临时目录 |
| 渲染规避 | `WEBKIT_DISABLE_DMABUF_RENDERER=1`（本机 WebKitGTK 环境差异，未进仓库） |
| 引擎状态 | 顶栏显示「引擎未就绪」；`ENDPOINT_ISOLATION_CONFIRMED = false` |

截图：

| 文件 | 内容 |
| --- | --- |
| `window-01-workbench.png` | 主工作台，引擎未就绪，任务类型仍为「已实现：脱敏数据分析与生态研判」 |
| `window-02-weekly-result.png` | 周报场景载入合成样例并生成本地结果 |
| `window-03-saved.png` | 勾选确认后保存成功，显示落盘路径与字节数 |
| `window-04-project-result.png` | 项目跟进汇总，含逾期 2 项 |
| `window-05-writing-result.png` | 无匹配模板时按用途组织结构 |
| `window-06-writing-pending.png` | 事实与明确要求均为「待确认」，模型步骤标注未验证 |
| `window-07-redline-refusal.png` | 合成红线占位文本被拒绝，不回显命中原文 |

`offline-scenarios.txt` 是 `examples/evaluate_offline_scenarios` 的直接输出（脚本运行证据，不是窗口证据）。
`saved-offline-weekly-report.md` 是窗口内保存的成果副本，用于核对内容；原始文件保存在一次性 `/tmp` 目录，未进入仓库。

---

## 3. 隔离与边界核对

- `git diff -- runtime/` 为空；未修改 DSH Runtime。
- `~/.dsh` 与 `~/.agents` 的 mtime 在执行前后一致（`2026-10-15 20:17:43`、`2026-09-23 15:12:11`），未读取其内容。
- 隔离 `dsh-home`、`agents-home` 在窗口运行后仍为空，说明本次没有启动引擎或写入 DSH 数据。
- 本机应用数据只落在一次性 `/tmp` 根下的 `app-data`（配置、日志、UI 状态）与 `local-data`（成果）。
- 未读取、打印、复制凭证；未更改模型路由；未提交、推送、打标签。

---

## 4. 限制与未验证事项

- **模型撰写未验证。** 准入常量固定为 `false`，本轮没有任何模型调用；周报、文案、项目三个场景只走本地确定性步骤。不能据此推断模型输出质量、模板实际注入或风格规则生效。
- **任务类型仍是保留状态。** `task_types()` 把周报、一般写作、项目管理标为 `reserved`，本轮只提供离线流程，不构成正式能力或用途获准。
- **日期校验是基本范围检查。** `parse_date` 只验证 `YYYY-MM-DD` 形式与月/日范围，不拒绝 2 月 30 日这类日历非法日期。
- **保存目录共用。** 离线成果与既有分析流程共用本地数据根，以 `offline-` 前缀区分；是否需要单独子目录留待 ARCHITECT 决定。
- **窗口点击为合成输入。** 通过 XTEST 注入鼠标与键盘事件完成操作，属真实窗口运行，但不是人工手工点选。
- **跨平台未验证。** 证据仅来自 Linux + Xvfb。

---

## 5. 复现要点

1. 在 `desktop/` 执行 `npm run build`，在 `desktop/src-tauri/` 按仓库工具链脚本执行 `cargo build`。
2. 起一个一次性 `Xvfb`，把 `OMNIDESK_DSH_HOME`、`OMNIDESK_AGENTS_HOME`、`OMNIDESK_APP_DATA_ROOT`、`OMNIDESK_ENGINE_STATE_DIR`、`OMNIDESK_LOCAL_DATA_ROOT` 和 `HOME` 全部指向该临时根。
3. 在 `desktop/` 起 `vite preview --port 5273 --strictPort`，再带 `DISPLAY` 与 `WEBKIT_DISABLE_DMABUF_RENDERER=1` 启动桌面二进制。
4. 进入「离线流程」页签，依次验证三个场景、保存与红线拒绝。
