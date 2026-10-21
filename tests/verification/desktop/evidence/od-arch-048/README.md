# OD-ARCH-048 证据索引：对话通道切到 DSH 本地 Web API

- 执行日：2026-10-21。开工 HEAD：`15aef4f`（`main`）。**未提交、未推送、未打标签。**
- 未改 `runtime/`（`git diff -- runtime/` 为空），未碰 `~/.dsh`、`~/.agents`、`~/OmniDesk`、凭证与模型路由文件。
- 全部运行验证使用 `/tmp` 下一次性数据根（`DSH_HOME` / `DSH_AGENTS_HOME` 均指向该根）+ 本机回环合成端点（`fake / synthetic-model`）。零真实凭证、零外网。
- 证据等级：**文件事实**、**静态检查**、**脚本运行**（真实 DSH web 进程 + 真实 WebSocket）、**未验证**。

改动结构：`engine/omnidesk_engine/task_core.py`（从 ACP 抽出的任务核心）、`web_api_adapter.py`（新通道）、`serve.py`（按 `dsh.channel` 选适配器，默认 web-api）、宿主 `engine_supervisor/engine_bridge/route_classifier/lib.rs`、前端 `HomeChat/ModelPicker/api/types`、脚本 `dev-ui.sh`/`omnidesk.sh`。

## 1. Web API 通道场景（W01–W10）

驱动：`tests/verification/engine-interface/adapter/run_web_api_scenarios.py`；证据：`tests/verification/engine-interface/evidence/web-api/W01..W10.json`。

| 场景 | 验证内容 | 判定 | 关键证据 | 等级 |
| --- | --- | --- | --- | --- |
| W01 | 启动、token→cookie 换取了即丢弃、目录可读、诊断文本不含 token | pass | `token_discarded_after_auth=true`、`startup_token_redacted=true`、`no_token_in_public_objects=true` | 脚本运行 |
| W02 | 真实分批流式：3 块分 3 次到达、序号单调、拼接等于最终结果 | pass | `delta_arrivals` 到达时刻 0.434 / 0.673 / 0.920s；`stream_join_eq_result=true` | 脚本运行 |
| W03 | 上游 400 → `turn/end=error` → failed，无假成功、无内容 | pass | `status=failed`、`evidence_source=engine_terminal`、`content=''` | 脚本运行 |
| W04 | 挂起中取消 → `turn/end=aborted` 确认取消 | pass | `phase=confirmed_cancelled`、`settled_within_2s` 实测 0.032s | 脚本运行 |
| W05 | 取消与完成竞争不产生矛盾终态、不回退 | pass | 终态唯一；若引擎先完成则保留 succeeded 并注明 | 脚本运行 |
| W06 | 同一 sessionId 连续两轮 | pass | `session_mode=continued`、两轮结果互不覆盖（FIRST / SECOND） | 脚本运行 |
| W07 | 进程重启后列会话、恢复并继续 | pass | `restarted_engine_lists_session=true`、`recover_ok=true`、`session_mode=resumed`、新文本 AFTER-RESTART | 脚本运行 |
| W08 | `system/message` 不进结果/事件；人格前缀不外泄 | pass（外泄检查）+ **未验证**（是否送达模型） | 帧里实际观察到 `system/message`；结果与事件均不含标记；整包探针 `body_hits=[]` | 脚本运行 |
| W09 | 未知会话立即报 `UNKNOWN_SESSION`，不建会话、不调模型 | pass | 8s 超时设置下 0.0x s 返回；`model_request_count=0` | 脚本运行 |
| W10 | 模型目录归一化（provider/分组/efforts），不含凭证字段 | pass | `synthetic-model@fake` 在列；分组合计等于模型数；关键词扫描无 apikey/cookie | 脚本运行 |

场景脚本自带一次性数据根清理；证据中不保存 prompt 全文以外的用户数据，全部为合成占位文本。

## 2. 适配层单元测试

`tests/verification/engine-interface/adapter/test_web_api_unit.py`（纯 Python，不启动引擎）：**25 passed / 0 failed**。覆盖：

- `_frame_error`：顶层错误帧、`value.type=error`、无载荷错误帧、正常帧不误判；not-found→`UNKNOWN_SESSION`、unsupported→`UNSUPPORTED`、其它→`ENGINE_UNAVAILABLE`。
- `_on_stream_frame`：text-delta 追加并标 `streamed`、空增量不产生事件、finish 返回 reason、usage→progress、stream end 非 committed→progress、未知帧忽略。
- `_settle_turn`：completed→succeeded、aborted→cancelled、error/failed→failed、max-tokens→succeeded+truncated、interrupted→interrupted、blocked/空/未知→unknown（绝不判成功）；无增量时用最终消息补齐并标 `streamed=false`。
- `for_dsh` 参数顺序（`--profile` / `--patch` 在 web app 参数之前）、相对 cwd 拒绝、空命令拒绝；`_redact` 去掉 token 但保留其余查询参数。

## 3. ACP 通道回归

`python3 tests/verification/engine-interface/adapter/run_engine_adapter_scenarios.py`：E01–E20 全量重跑，结论见 `STATUS.md` 与 `tests/verification/engine-interface/evidence/adapter/E*.json`（本轮 E01–E19 证据被重写，属预期）。

## 4. 宿主与前端

- `desktop/src-tauri/scripts/dump-model-route.mjs --self-test`：24/24 通过（新增 `--channel`，按通道选 `agent-default-model` 或 `acp` 条目）。
- `cd desktop && npx tsc --noEmit`：0 错误；`npm run build`：通过。
- `cargo test`：165 passed / 0 failed（需 `eval "$(scripts/setup-desktop-toolchain.sh --print-env)"`）。

## 5. 与计划的偏差（需 ARCHITECT 决定是否接受）

1. **终态字段**：DSH 的 `turn/end` 实际 `reason.kind="completed"`，`stop` 出现在 assistant-stream 的 finish 帧。适配层按 `turn/end` 落定、用 finish 作为 stop_reason，两者都记录。
2. **路由分类器**：原 `dump-model-route.mjs` 只认 `acp` 配置，web Profile 覆盖层（`agent-default-model`）会被宿主判为 `route_missing`，web 通道经宿主准入直接不可用。为此给脚本与 `route_classifier.rs` 增加 `channel` 选择（最小修复，属计划外）。
3. **失败态映射**：实测 `turn/end` 的失败态是 `error`，原实现只认 `failed`，会把失败落成 unknown；已修正并补 `max-tokens` / `interrupted` / `blocked` 分支。
4. **WS 错误帧**：`session/not-found` 是顶层 `{"type":"error"}` 帧，原实现只看 `value.type`，导致未知会话拖到超时；已修正并加单测。

## 6. 未验证事项

- **思考块流式**：合成端点不产思考内容，未验证。
- **personaPrefix 是否进入 web 通道的模型请求**：`--dump-config` 显示配置被覆盖，但请求体整包探针未命中标记。只记录为“配置可见、请求不可见”，不推断模型是否收到。
- **真实模型端点**：本阶段未接真实模型（凭证不动），逐字流式仅在合成端点验证。
- **真实窗口抽验**：本轮未做人工窗口验证（见 STATUS 说明）。
- **wss://**：内置 WS 客户端仅支持 `ws://`，对 https 端点返回 `UNSUPPORTED`；当前 DSH 本地服务为 http，未阻塞。

## 7. 卡死修复与慢启动实测（同日追加）

触发：用户报告本计划改动后桌面启动即整窗无响应、场景无法切换，只能强制退出。

### 根因（文件事实）

`~/.cargo/registry/src/*/tauri-macros-2.7.3/src/command/wrapper.rs`：命令宏的 `execution_context` 缺省为 `ExecutionContext::Blocking`，只有 `#[tauri::command(async)]` 或 `async fn` 才置 `Async`（生成 `kind = "sync_threadpool"`）。**同步命令在 UI 主线程执行。**

本仓库 `lib.rs` 原有 49 个同步命令，绝大多数第一步就 `state.lock()` 取全局宿主锁 `SharedHost = Arc<Mutex<HostState>>`，随后同步等引擎（`engine_ensure_started` 冷启动预算 60s）。首屏 `hydrateThreads()` 调的 `thread_store_load`、模型选择器调的 `engine_list_models` 都在其中。任一条占住主线程 → WebView 不能重绘、DOM 事件不入队 → 场景切不动。

### 改动

1. `desktop/src-tauri/src/lib.rs`：49 个同步命令属性改为 `#[tauri::command(async)]`。函数体、签名、`Mutex` 语义均未变（同样阻塞等待同一把锁，只是不再占 UI 线程）。6 个已有 `async fn` 命令不动。
2. `desktop/src/sceneThreads.ts`：新增 `HYDRATE_TIMEOUT_MS = 4000`、`withTimeout()`、`HydrateTimeout`、`applyListing()`。`hydrateThreads()` 4 秒未拿到数据根就先渲染本地缓存并在 `storageNote` 说明，后台 `pending.then` 读到后再 `applyListing` 补上；`hydrated` 仍只在真正读到数据根时置位。
3. 新增回归 `desktop/src-tauri/src/lib.rs::every_tauri_command_runs_off_the_ui_thread`：扫 `run()` 之前的生产代码，逐行找 `#[tauri::command]`，取其修饰的第一个 fn 签名；若为同步 fn 即失败。注释行跳过，`async fn` 跳过。
4. `desktop/src-tauri/src/lib.rs::engine_lifecycle_wiring_tests` 中两处 `find("\n#[tauri::command]")` 改为 `find("\n#[tauri::command")`，否则属性改为 `(async)` 后切不出边界（原本 1 failed）。

### 运行验证

| 检查 | 命令 | 结果 | 等级 |
| --- | --- | --- | --- |
| 类型检查 | `cd desktop && npx tsc --noEmit` | exit 0 | 静态检查 |
| 前端构建 | `cd desktop && npm run build` | 通过 | 脚本运行 |
| Rust 全量 | `eval "$(bash scripts/setup-desktop-toolchain.sh --print-env)" && cd desktop/src-tauri && cargo test --offline` | 166 passed / 0 failed（lib 147、2、13、3、1；doc 0） | 脚本运行 |
| Runtime 未改 | `git diff --stat -- runtime/` | 空 | 文件事实 |
| 新增回归 | `cargo test --offline every_tauri_command_runs_off_the_ui_thread` | 1 passed | 脚本运行 |

### 真实窗口慢启动实测（Xvfb 隔离）

方法：`Xvfb :99 -screen 0 1600x900x24`；`OMNIDESK_DEV_ROOT=/tmp/omni-fast-r1`（一次性数据根，写 `settings.v1.json` 开本机模型权限、artifact 指向 `/tmp/omni-fast-r1/artifacts`，端点为回环合成服务）；`OMNIDESK_PYTHON=/tmp/slow-python.sh`（脚本内容 `sleep 40; exec /usr/bin/python3 "$@"`），只拖慢引擎子进程；`bash scripts/dev-ui.sh --no-build`（新构建的二进制）。点击与计时用一次性脚本 `/tmp/odx_scene.py`：每次点击后只比较**内容区**（x 300–1240、y 120–700）的像素签名，内容区变化即判定切换完成；截图存 `/tmp/odshots/`。

结果（引擎 40 秒慢启动期间，即窗口刚起来的前 30 秒内）：首页→周报 0.73s、周报→文案写作 0.30s、文案→项目跟进 0.22s、项目→任务工作台 0.22s、回首页 0.38s；每张截图都显示对应场景已正确渲染（未出现白屏或旧画面残留）。引擎接通后重复同一组切换：0.23 / 0.84 / 0.24 / 0.22 / 0.22 / 0.36s。底栏在等待期间显示「模型权限：正在读取宿主事实…」，接通后变「已开启 · 本机」，顶栏出现 `LOCAL IPC ACTIVE`。

**方法修正（重要）**：本节早先记录的 0.17–0.43s 用的是 `/tmp/odxvfb_resp.py`，它只截左侧 180px 宽的区域，实际测到的是 Dock 的悬停提示气泡，属假阳性；另外那一轮跑的进程是修复前的旧二进制。以上数字已用修复后重新构建的二进制 + 内容区判据重测替换。

证据：`/tmp/odshots/slow11_1_weekly.png` … `slow11_5_home.png`、`chk1_1_weekly.png` … `chk1_5_home.png`、`idle_0_hover.png` / `idle_6_restored.png`。**这些位于 `/tmp`，属一次性运行证据，不入库**；结论、方法与本轮修正已固化在本节与 `STATUS.md`。

### 首页「空白」不是缺陷（同轮澄清）

权限开启后曾观察到首页中部整块空白。按 `SceneScaffold` 的既有设计（OD-ARCH-040/042）：指针停在对话区时顶部卡片带**整体淡出到 opacity 0**，把空间让给对话；此时没有历史消息，中间自然是空的。把指针移回卡片带，问候与三模块立即恢复（`idle_0_hover.png` 为指针在对话区时的空态，`idle_6_restored.png` 为指针在卡片区/静止 5 秒后的恢复态，两者差异只在卡片带）。这不是本轮改动引入的渲染异常，未改该交互。

### 已知限制

- 未改全局锁本身。命令仍会等待同一把 `HostState` 互斥锁，因此「同时发起两条需引擎的命令，后者排队」的行为不变——只是不再冻界面。若日后需要并行独立任务，属架构变更，须 ARCHITECT 决策。
- 本机 `DISPLAY=:0` 有其它程序持有 pointer/keyboard grab，xtest 无法注入，故未在用户日常桌面环境复测；用户的 DSH `:3080` 全程未被触碰。
