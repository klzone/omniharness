# OD-BUILD-005 真实 Tauri 窗口证据（五阶段完整流程）

- 采集时间：2026-10-17 02:25–02:28（Asia/Shanghai）
- 采集者：BUILDER
- 对应计划：[PLANS.md](../../../../PLANS.md) 的 **OD-BUILD-005：把已实现的引擎链路接进日常任务流程**
- 证据等级：**脚本/运行**（真实 Tauri 窗口 + 真实引擎子进程 + 真实 DSH CLI + 本机回环**合成**模型端点）。
  其中模型一端为合成回环端点，**不是真实模型运行**，不代表任何真实模型输出质量或已获授权。

---

## 0. 结论

在真实 OmniDesk 桌面窗口内，用合成样例走完了「阶段 1 输入 → 阶段 2 确定性统计 → 阶段 3 引擎解释回填
→ 阶段 4 人工确认 → 阶段 5 保存到仓库外」的完整五阶段，并在仓库外真实落盘产物。统计数值全部来自
本地确定性代码；引擎阶段只做解释，产物中记录了统计指纹与解释来源 `model_unverified`。

本目录同时记录两次**本机环境问题**（均非应用代码缺陷），用于后续复现时不再误判：

1. **窗口全黑**：本机 Wayland + XWayland + 混合显卡下 WebKitGTK 的 DMABUF 渲染器失败；注入
   `WEBKIT_DISABLE_DMABUF_RENDERER=1` 后正常渲染（同一窗口的 distinct 颜色数由约 43 升至 3000+）。
   这是**本机 WebKitGTK 渲染规避**，不是 OmniDesk 缺陷。
2. **点击无反应**：在用户真实会话 `:0` 上，XTEST 合成的按键事件被会话层吞掉（指针移动事件可达、
   按键事件不可达）。已用最小对照窗口（GTK / WebKit `mousedown` / Tkinter）逐层排除，**与 OmniDesk 无关**。
   改用 Xvfb 干净显示器后点击正常。

---

## 1. 运行环境（可复现）

| 项 | 值 |
| --- | --- |
| 显示器 | `Xvfb :79 -screen 0 1360x3600x24`（干净、无用户会话干扰） |
| 窗口 | label `main`，id `0x200004`，`configure(x=0, y=0, w=1280, h=3500)` |
| 应用 | `desktop/src-tauri/target/debug/omnidesk-desktop`（debug 构建，前端从 Vite `127.0.0.1:5273` 加载） |
| 隔离数据根 | `/tmp/od011/run0/{dsh-home,agents-home,app-data,engine-state,local-data,result}`（一次性，仓库外） |
| 引擎 Profile | `OMNIDESK_DSH_PROFILE=acp` |
| DSH CLI | `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js`（锁定版本底座） |
| 模型端点 | 本机回环合成端点 `http://127.0.0.1:37587/v1`（零凭证、零外网） |
| 网关来源 | `OMNIDESK_DSH_PATCHES=/tmp/od005/synthetic-provider.yml`（合成 provider 覆盖层） |

窗口高度需 ≥ 内容高度，否则阶段 2/5 的面板会被压扁裁切、保存按钮不可点；窗口高度超过屏幕高度会让
抓图抛 `BadMatch`，故屏幕高度设为 3600、窗口高度设为 3500。

---

## 2. 五阶段实测结果

| 阶段 | 动作 | 可观察结果 |
| --- | --- | --- |
| 1 输入 | 点击合成样例「基础分类计数」 | 按钮转为 primary 高亮，输入框填入 5 行合成数据 |
| 2 确定性统计 | 点击「运行确定性统计」 | 行数 5 / 类别数 5 / 合计 204 / 平均 40.8 / 最小 29 / 最大 51 / 最高类别「制度执行」/ 指纹 `320dae745102cd0d…`，并列出按类别表格 |
| 3 引擎解释 | 点击「用引擎生成解释」 | 合成端点 `request_count` 由 1 → 2；界面显示 `引擎任务 omni-7f32dbfd-…`、`状态 已完成 stop_reason=end_turn`；解释来源自动变为 **「模型输出 · 未复核 (model_unverified)」**；解释框回填「合成解释文本：本期事项甲3件。」 |
| 4 人工确认 | 勾选确认框 | 确认状态置位；未确认时保存被拒绝（见 §4） |
| 5 保存 | 点击「保存产物」 | 界面显示 `已保存 /tmp/od011/run0/local-data/20261016-182827-320dae745102.md`、`字节 1739`、`写入时间 2026/10/17 02:28:27` |

产物文件内容见 [`saved-artifact.md`](saved-artifact.md)。其中已核对：

- 外壳版本 `omnidesk-task-shell/1`、任务类型 `desensitized_analysis`；
- **统计指纹 SHA-256** 与阶段 2 一致，冻结统计块明确标注「禁止重算或修改」；
- 解释来源 `model_unverified`，且**不存在任何「已复核模型来源」选项**；
- 人工确认 `user_confirmed = true`；
- 模板「未使用模板」、历史范例「未使用历史范例（`knowledge/excellent-cases/` 当前无常驻范文）」；
- 原始输入原文保留、未被覆盖。

---

## 3. 截图证据

| 文件 | 内容 |
| --- | --- |
| `full0.png` | 初始整页（阶段 1–5 全貌，未填数据） |
| `t1_after.png` | 载入合成样例后 |
| `t2_after.png` | 运行确定性统计后（阶段 2 数值与表格） |
| `t3_after.png` | 引擎解释回填后（阶段 3 任务号、状态、来源、解释文本） |
| `t4_after.png` | 人工确认勾选后 |
| `t5_after.png` | 保存产物后（阶段 5 落盘路径、字节数、写入时间） |

截图均在 `:79` 隔离显示器上采集，未触碰用户 `:0` 会话。

---

## 4. 失败与拒绝路径（脚本级回归）

窗口路径之外的确定性回归由 `tests/verification/desktop/run_workbench_engine_path.py`
（本轮复跑通过：`success=succeeded`、`failure=failed`、`cancel=cancelled`）与
`desktop/src-tauri/tests/task_shell_e2e.rs` 覆盖，包括：

- 未人工确认时保存被拒绝；
- 引擎失败 / 取消不写成成功，也不改写用户已写的解释；
- 输入变化即令下游产物（统计、提示词、解释）失效。

---

## 5. 未验证事项（不得当作已通过）

- **真实模型运行：未验证。** 本轮模型一端全部是本机回环合成端点，零凭证、零外网。
- **跨平台：未验证。** 仅当前 Linux 实测；`/proc` 进程检测、`realpath` 行为、WebKitGTK 渲染规避
  都只在 Linux 验证过。
- **窗口级自动化：未验证。** 本轮窗口内的点击由 XTEST 合成投递完成，尚无仓库内可复现的窗口自动化脚本；
  浏览器/UI 自动化属独立决策，不在本阶段范围。
- **分发与产品化：未评估。** 见 GOALS.md 的联合门槛。

---

## 6. 建议

本轮未新增仓库内桌面启动脚本（`scripts/` 不在 OD-BUILD-005 计划列明的影响范围内）。
`WEBKIT_DISABLE_DMABUF_RENDERER=1` 属本机 WebKitGTK 渲染规避，尚未进入仓库。
建议由 ARCHITECT 决定是否单列一项「仓库内桌面开发启动器（含本机渲染规避与隔离数据根校验）」，
以便后续无需手工拼环境变量即可复现真实窗口路径。


---

## 7. 补充：修复后窗口渲染与响应性冒烟（2026-10-17 22:36）

修复落地后在干净 Xvfb（`:82`，1280×1000）上直接启动 `desktop/src-tauri/target/debug/omnidesk-desktop`
（隔离数据根 `/tmp/od012/run/*`，一次性、仓库外），观察结果：

| 观察项 | 结果 |
| --- | --- |
| 窗口是否映射 | 是：`0x200004 "OmniDesk Discipline" ("omnidesk-desktop") 1280x820+0+90` |
| 是否仍有“无响应” | **否**：窗口可渲染、无卡死；颜色数 12708（全黑时约 43） |
| WebKit 渲染进程 | 存活：`WebKitNetworkProcess`、`WebKitWebProcess` |
| 引擎子进程 | 已启动：`python3 -m omnidesk_engine` |
| 产物文件 | [`smoke-window-rendered.png`](smoke-window-rendered.png) |

**该截图的前端来自并行 worktree，不是本目录 §3 的那套界面。** 说明：debug 构建的
`tauri.conf.json` 用 `devUrl = http://127.0.0.1:5273` 加载前端，而当时 5273 被同一仓库的另一个
worktree（`/mnt/data/Code/omniharness-ui`，分支 `feat/desktop-ui`，同 HEAD `5ffc710`）的 Vite
占用。因此**本图证明的是 Rust 宿主窗口已能渲染并保持响应（锁饥饿修复生效）**，图中界面内容
不属于 OD-BUILD-005 的产物。OD-BUILD-005 自有前端（含引擎解释支路）的窗口证据仍是 §3 的
`t1_after.png` … `t5_after.png`。

**由此暴露的一个本机开发冲突（未处理，留待决策）：** 同一仓库存在两个 worktree，二者的
`devUrl` 都是 5273 且 Vite 为 `strictPort`。谁先启动谁占用，另一个 `npm run tauri:dev` 会加载到
**别人的前端**，而开发者通常不会察觉。这是本地开发环境的端口约定问题，不是应用缺陷；
是否改为可配置端口或按 worktree 分配端口，需 ARCHITECT 决定。
