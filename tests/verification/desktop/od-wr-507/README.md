# OD-WR-507「交给 AI 继续分析」：验收证据

日期：2026-11-11
基线：eec28fd635（OD-WR-500 规划）；501–506 的未提交改动全部保留，未重置、未提交、未推送。

## 改动范围

| 文件 | 改动 |
| --- | --- |
| desktop/src/shell/WorkRecordLibrary.tsx | 记录详情操作行新增「交给 AI 继续分析」按钮与预填处理 |
| desktop/src/shell/ReviewArchive.tsx | 复盘详情操作行新增同名按钮与预填处理 |
| desktop/src/shell/WorkRecordPage.tsx | 向 ReviewArchive 透传既有 onNavigate（一行） |
| tests/verification/desktop/verify_wr_507.py | 新增静态契约验证 |

## UI 参考借鉴

参考 Gmail Web、Sana AI、Pluto AI 的上下文转交与助手入口：按钮放在详情操作行末尾，沿用现有 pill 样式。跳转后只在输入框预填带对象编号的上下文句，不自动发送，不引入新的视觉体系。

## 实跑命令与结果

| 命令 | 结果 | 输出 |
| --- | --- | --- |
| cd desktop && npx tsc --noEmit | exit 0 | tsc.txt |
| cd desktop && npm --prefix . run build | exit 0 | build.txt |
| python3 tests/verification/desktop/verify_wr_507.py | 33/33 通过 | verify-507.txt、verify-507.json |
| python3 tests/verification/desktop/verify_wr_506.py | 58/58 通过 | verify-506.txt、verify-506.json |
| python3 tests/verification/desktop/verify_wr_505.py | 71/71 通过 | verify-505.txt |
| python3 tests/verification/desktop/verify_wr_504.py | 41/41 通过 | verify-504.txt |
| python3 tests/verification/desktop/verify_wr_503.py | 58/58 通过 | verify-503.txt、verify-503.json |
| python3 tests/verification/desktop/verify_work_record_ui_301.py | 15/15 通过 | verify-301.txt |

## 裁决记录

* 首页是应用页面键，不是 ThreadTag；工作记录对应的既有场景键是 `weekly`，两处转交均使用 `requestNewThread("weekly", ...)` 后 `onNavigate("home")`。
* 预填文本只进入首页输入框草稿，不调用发送、任务提交或任何数据写入接口。
* 全局对话采集保持保守不扩：代码注释已写明理由（避免任何输入被误记），本轮未改 HomeChat 发送链路或采集逻辑。
* ReviewArchive 需要导航回调，沿用 WorkRecordPage 已持有的 `onNavigate`，仅做一行透传。

## 未做与已知限制

* 本轮只改前端；未修改 Rust、IPC、runtime/、数据层，未重跑 cargo。OD-WR-506 证据中 process_owner 的 3 项沙箱误报与本轮改动无关，保留原说明。
* 未做真实模型调用；验证覆盖按钮、预填文本、对象编号、场景键、导航与不自动发送的静态契约。
