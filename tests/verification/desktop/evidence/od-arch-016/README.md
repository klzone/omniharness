# OD-ARCH-016：取消入口修复证据

日期：2026-10-18。工作区 `/mnt/data/Code/MyWorkStation`，HEAD `5ffc710`（未提交改动保留）。

## 范围

只修 `desktop/src/workbench.tsx`：任务运行中可点取消、请求在途防重复、取消结果以宿主终态为准。
未改 `runtime/`、`types.ts`、IPC、Rust 层、准入常量、模型路由或凭证。

## 证据等级

| 项目 | 等级 | 文件 |
| --- | --- | --- |
| 取消按钮/结果判定 | 编译真实源码后用 Node 执行（离线确定性） | `engine-cancel-model.json` |
| 按钮接线与防重入 | 静态源码检查 | 同上 `static_checks` |
| 前端类型检查与构建 | 实际命令运行 | `frontend-build.log` |
| 真实宿主取消、窗口点击、模型任务 | **未验证** | — |

## 实际命令与结果

```bash
cd desktop && npx tsc --noEmit        # 退出码 0
cd desktop && npm run build           # 退出码 0，631ms
python3 tests/verification/desktop/verify_engine_cancel.py --json tests/verification/desktop/evidence/od-arch-016/engine-cancel-model.json
# state cases: 20 total, 0 failed; static checks: 5 total, 0 failed
git diff --exit-code HEAD -- runtime/ # 退出码 0
git diff --check HEAD                 # 退出码 0
```

## 覆盖的场景

- 无任务、无宿主状态时禁用；`queued/starting/running/cancelling` 可点。
- 取消请求在途时禁用并显示等待；源码用同步 ref 在首个 `await` 前置位，快速连点只发一次。
- `confirmed_cancelled` 或宿主最终态 `cancelled` 才显示“取消已确认”。
- 取消与正常完成竞争：请求已发但最终 `succeeded`，显示“取消未确认”，以宿主终态为准。
- `timeout_unknown` 显示未确认，且允许“重试取消”。
- 终态（成功/失败/取消/中断/未知）后禁用。
- 取消函数内不含 `setExplanation` / `setSource`，不改写已有解释正文。

## 限制

本轮没有启动引擎或窗口，因此“真实点击按钮后宿主确实收到取消请求”仍属未验证；该结论必须由 ARCHITECT 在隔离环境用合成任务复核。端点准入两侧常量仍为 `false`。
