# OD-WR-303 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 范围说明：未修改 STATUS.md / REVIEW.md / PLANS.md。

## 布局借鉴（OD-ARCH-035）

动工前用 screenshots-design-inspiration 看了 Asana、Notion、Linear 三个同类布局。借鉴：左侧行式清单把日期、标题、状态压在同一行，详情放右侧，筛选收成一行；状态用贴近行尾的彩色小 chip，属性横向对齐。没有复制它们的深色侧栏与多层卡片，沿用 Aetheria 浅色卡面。

## 改动文件

- 新增 desktop/src/shell/WorkRecordPage.tsx：记录 / 任务 / 周视图三个视图。
- 新增 desktop/src/shell/WorkRecordLibrary.tsx：快速记录、筛选、列表、详情、编辑、删除。
- 修改 desktop/src/App.tsx：工作记录场景接到新页面；周视图保留原 ScenePage。
- 新增 tests/verification/desktop/verify_work_record_ui_301.py：15 项静态检查。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| npx tsc --noEmit | 0 | 无输出 |
| npm --prefix desktop run build | 0 | vite 构建成功（936ms） |
| python3 tests/verification/desktop/verify_work_record_ui_301.py | 0 | 15/15 通过 |

原始输出：tsc.log、npm-build.log、verify-work-record.log、exit-*.txt。

## 已知限制

- 解析入口在 303 阶段是诚实占位；OD-WR-306 已接上 prepare / submit / apply 三段链路，以 od-wr-306 证据为准。
- 关联任务数读 linked_task_ids，双向回写由 OD-WR-307 完成，当前显示 0 是真实值。
- 未在真实 Tauri 窗口目视核对；静态检查加构建通过，界面验收仍待人工。
