# OD-ARCH-022 执行证据：端点去向说明、权限切换一致性与安装验收

- 执行时间：2026-10-18 17:40–20:25（Asia/Shanghai）
- 执行者：OpenCode（用户授权；主工作区 `/mnt/data/Code/MyWorkStation`）
- 对应计划：[PLANS.md](../../../../PLANS.md) 的 **OD-ARCH-022**
- 依据复核：[REVIEW.md](../../../../REVIEW.md) 顶部的 OD-ARCH-021 复核
- 证据等级：脚本/运行 + 构建 + 静态检查

## 0. 结论摘要

1. **端点类别改为不承诺组织归属**：`internal/external` → `private/public`，只区分“本机回环 / 非回环”，说明文字显式声明“不代表所属组织”。
2. **权限变更失败关闭**：只有确认旧引擎已停止才报告生效；停止失败则返回失败、置位屏障并拒绝一切新的模型提交，并提供恢复路径。
3. **分类用例数校准**：OD-ARCH-021 记录 22 为误记（实际 18）；本次改名并新增 3 条组织归属断言后，脚本实际为 **21**。
4. **隔离安装验收完成**：在用户级隔离根完成 `.deb` 安装（`ii`）→ 从桌面入口启动并渲染 → 卸载；**无 root、无系统级改动**。
5. 计划要求的回归全部通过；`runtime/` 无差异。

## 1. 改动文件

**端点分类（第 1 项）**
| 文件 | 改动 |
| --- | --- |
| `desktop/src-tauri/scripts/dump-model-route.mjs` | 类别 `internal/external` → `private/public`；判定理由改为地址范围事实并显式声明“不代表所属组织/实际转发路径”；自测新增 3 条组织归属断言 |
| `desktop/src-tauri/src/route_classifier.rs` | 类别白名单与 `is_configurable` 同步为 `local/private/public`；新增单测：新类别可配置、旧类别名被拒绝 |
| `desktop/src-tauri/src/settings.rs` | `ModelPermissions` 字段 `internal/external` → `private/public`（`serde(alias)` 兼容旧文件）；`for_category`、`revoke_all`、`set_model_permission`（旧类别名作为别名接受）；新增旧键迁移单测 |
| `desktop/src-tauri/src/admission.rs` | 准入说明与 `permissions` 出参键同步；单测覆盖新类别 + 旧类别名不放行 |
| `desktop/src-tauri/src/lib.rs` | 权限出参键同步 |
| `desktop/src/types.ts` | `ModelCategory`、`ModelPermissions` 同步；`AdmissionStatusResponse` 增加 `blocked`/`block_reason` |
| `desktop/src/api.ts` | `setModelPermission` 类别类型；新增 `shutdownEngine()` |
| `desktop/src/SettingsPanel.tsx` | 类别标签与三档文案重写（不承诺组织归属）；权限变更失败与屏障提示；新增“停止引擎以恢复” |
| `desktop/src/shell/EngineTechSlice.tsx` | 准入提示改用中文类别标签，不再显示原始类别名 |

**权限切换一致性（第 2 项）**
| 文件 | 改动 |
| --- | --- |
| `desktop/src-tauri/src/lib.rs` | `HostState` 增加 `permission_barrier`；`engine_submit_task` 先检查屏障并拒绝；权限命令改为“先持久化 → 停止旧引擎 → 确认停止才报生效”，失败置位屏障；`engine_shutdown` 确认停止后清除屏障；`engine_admission_status` 返回 `blocked`/`block_reason`；失败语义抽为纯函数并新增 2 条单测 |

**兼容修复**：`desktop/src-tauri/tests/{admission_classification,host_integration}.rs`、`desktop/src-tauri/examples/evaluate_offline_scenarios.rs` 同步字段改名。

**未改动**：`runtime/`、`REVIEW.md`、`docs/decisions.md`、模型路由、凭证、DSH 数据、第二 worktree（`/mnt/data/Code/omniharness-ui`）。

## 2. 逐项结果

### 2.1 端点去向说明不再承诺组织归属

`dump-model-route.mjs` 现只回答“是不是本机回环”：

| 类别 | 含义 | 说明文字要点 |
| --- | --- | --- |
| `local` | 回环（localhost / 127.0.0.0/8 / ::1） | 可就地判定内容不出本机 |
| `private` | 非回环的私有 / 链路本地 / `.local` | “仅为地址范围事实，不代表所属组织或实际转发路径” |
| `public` | 非回环的公网地址 | “是否离开所在组织无法从地址判断” |
| `unknown` | 缺端点 / 非 HTTP(S) / 带凭证·查询·片段 / provider 无法识别 | 恒拒绝，无可开启开关 |

`leaves_device` 语义限定为“是否离开本机”，不再是组织判断；非回环一律 `null`（无法确认）。

### 2.2 权限切换一致性（失败关闭）

新顺序：**持久化新权限 → 停止旧引擎 → 仅当确认停止才报告生效**。

- 停止成功：清除屏障，返回 `ok: true`、`engine_stopped: true`、`blocked: false`。
- 停止失败：返回**失败**（`permission_change_incomplete`），置位屏障；此后 `engine_submit_task` 直接拒绝并说明原因，不启动引擎、不发送输入；恢复路径为“重试权限变更”或“停止引擎”（`engine_shutdown` 确认停止后自动清除屏障），设置页有对应按钮与提示。

### 2.3 用例数校准

- OD-ARCH-021 记录“22 项”为误记，实际为 **18**（已在 `od-arch-021/README.md` 注明）。
- 本次改名并新增 3 条“私有地址不声称组织归属”断言后，`--self-test` 实际为 **21 项**，`ok: true`。

### 2.4 隔离安装验收

| 步骤 | 结果 |
| --- | --- |
| 隔离根 | `/tmp/od022-install`（`--root`/`--admindir`/`--instdir`） |
| 安装 | `dpkg --force-not-root --force-depends --force-script-chrootless -i` → 状态 `ii`（已配置） |
| 依赖说明 | 隔离 dpkg 库不含系统包记录，故用 `--force-depends`；系统实际已有 `libwebkit2gtk-4.1.so.0`、`libgtk-3.so.0`（`ldconfig` 核对） |
| 内容 | `usr/bin/omnidesk-desktop`、`usr/share/applications/OmniDesk.desktop`、多尺寸图标 |
| 启动 | `gtk-launch OmniDesk`（`XDG_DATA_DIRS` 指向隔离根）→ 窗口 “OmniDesk Discipline” 正常渲染；进程为 `/tmp/od022-install/usr/bin/omnidesk-desktop` |
| 卸载 | `dpkg -r omni-desk` → 启动器与 desktop 条目移除，dpkg 库不再列出 |
| 权限 | 全程 `uid=1000`，未提权、未改系统或桌面配置 |

截图：`installed-launch.png`。卸载时 dpkg 对 `/var/log/dpkg.log` 有一次权限提示（隔离根下无写权限），不影响卸载结果，如实记录。

## 3. 运行的检查与结果

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `npx --no-install tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `npm run build` | 成功（JS 317.96 kB / CSS 71.95 kB） | 前端构建 |
| `cargo test --offline` | **103 lib + 2 admission + 13 host + 3 e2e = 121 通过 / 0 失败** | Rust 离线回归 |
| `node scripts/dump-model-route.mjs --self-test` | **21/21，`ok: true`** | 脚本运行 |
| `run_desktop_e2e.py chain` | success/failure/cancel 全 pass | 脚本运行（合成端点） |
| `verify_engine_cancel.py` | `RESULT: pass` | 脚本运行 |
| `verify_ai_draft.py` | `RESULT: pass` | 脚本运行 |
| `verify_session_continuity.py` | 37 项 0 失败，`RESULT: pass` | 脚本运行 |
| `git diff --check HEAD` | 退出码 0 | 文件事实 |
| `git diff --exit-code HEAD -- runtime/` | 无差异 | 文件事实 |
| `.deb` 隔离安装 → 入口启动 → 卸载 | 通过 | 运行 |

自测原始输出：`dump-model-route-self-test.json`。

## 4. 未验证与限制

- **真实模型输出与质量**：未调用任何真实供应商端点，未读取凭证。
- **停止失败路径**：以纯函数单测覆盖“停止失败 → 失败关闭 + 屏障”；未在窗口内制造真实子进程无法退出的场景（`engine_bridge::shutdown` 真实失败难以稳定复现）。
- **屏障的端到端窗口验证**：未在真实窗口内触发一次“停止失败”，仅覆盖逻辑与界面提示接线。
- **仅 Linux**；Windows/macOS 未验证。AppImage 仍未产出。
- 安装验证为**用户级隔离根**模拟，不等于系统级安装。
- 真实业务材料、跨平台、长期使用未验证。

## 5. 边界声明

- 未修改 `runtime/`、模型路由、凭证、DSH 数据；未使用真实工作材料；未调用真实模型。
- 未修改第二 worktree；`REVIEW.md` 未改（留 ARCHITECT 复核）。
- 未提交、未推送、未打标签；既有未提交/未跟踪改动全部保留。
