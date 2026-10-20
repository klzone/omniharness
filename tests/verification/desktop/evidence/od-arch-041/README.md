# OD-ARCH-041 证据索引：场景切换卡顿的定位与修复

> 执行日：2026-10-21。起因：用户实测「从首页切换到周报或其他场景都超过 2 秒以上，再回首页也是卡顿」。

## 1. 根因（源码级 + 实测）

**两条宿主命令都在 Tauri 主线程上同步 spawn 一个 node 进程（各 ~1-2 秒）：**

| 命令 | 做什么 | 慢在哪 |
| --- | --- | --- |
| `engine_admission_status` | 读端点事实 | → `route_classifier::classify` → `Command::new("node")` 跑分类脚本 |
| `task_list_model_routes` | 列各 Profile 的模型候选 | → `model_routes::list_routes` → `Command::new("node")` 跑 `model-routes.mjs` |

而**对话区（`HomeChat`）在每次场景切换时重新挂载** ⇒ 每次切换都重拉这两项 ⇒
同步命令占住主线程 ⇒ **WebView 无法重绘 ⇒ 整页卡住**。

**实测基线（真实窗口、AT-SPI 轮询到目标节点出现）：**

| 切换 | 修复前 | 修复后 |
| --- | --- | --- |
| 首页 → 周报与随记 | **6.07 s** | **0.62 s** |
| 周报与随记 → 首页 | **3.51 s** | **0.48 s** |
| 首页 → 项目跟踪 | **6.03 s** | **0.46 s** |
| 项目跟踪 → 首页 | **3.35 s** | **0.73 s** |

（测量口径：点击 Dock 按钮 → AT-SPI 轮询到目标页面的可访问节点出现；轮询本身有约 0.2 s 粒度。）

## 2. 处置（两处，缺一不可）

1. **宿主侧改成异步命令**（`desktop/src-tauri/src/lib.rs`）：
   `engine_admission_status` 与 `task_list_model_routes` 由 `fn` 改为 `async fn`，
   node 进程那段等待离开主线程，界面照常可交互。
   *技术约束：* Tauri v2 的异步命令的 future 必须 `'static`，**不能借用 `State<'_, _>`**，
   因此改用 `AppHandle` 取一次状态、立刻克隆出需要的值并放掉锁（`admission_context` 的语义不变）。
2. **前端加缓存**（`desktop/src/api.ts`）：
   - 模型候选：缓存到下次失效为止（只在改选模型/思考等级时变化）；
   - 端点事实：短缓存 2 秒（权限开关变化由设置页自己刷新）。
   失效点：`invalidateEngineFactsCache()`，在**改选模型**与**提交被拒**时调用。
   并发的同 key 请求合并为一个在途 Promise（不重复起进程）。

## 3. 回归

Rust **143/143**、`tsc` 0、`npm run build` 通过、
`verify_conversation_prompt` 24/24、`verify_ai_draft` 27/0、`verify_scenario_materials` 5/0。

## 4. 已知边界（本轮未改）

- **首次提交仍会卡一下**：`engine_submit_task` 是同步命令且要启动引擎进程
  （DSH node 启动 ~2-4 秒），这段等待仍占住主线程 ⇒ 第一次提交时界面短暂无响应。
  改成异步需要重构「引擎调用守卫」与宿主锁的持有方式（风险较高），**另立计划**。
- 切换场景仍会**重新挂载对话区**，因此该场景的对话草稿不跨场景保留（既有行为，未变）。
