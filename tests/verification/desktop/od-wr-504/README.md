# OD-WR-504 时间线视图验收证据

## 基线

- HEAD：`eec28fd6357b4a8490613fa4694115e6e04c42d4`（main）
- 动工前已存在未提交改动，完整清单见 `baseline.txt`；本轮只改 `desktop/src/shell/WorkRecordLibrary.tsx`、`desktop/src/theme.css`，并新增本目录与 `../verify_wr_504.py`。

## 实跑结果

| 命令 | 结果 | 证据 |
| --- | --- | --- |
| `cd desktop && npx tsc --noEmit` | exit 0 | `tsc.log` / `tsc.exit` |
| `cd desktop && npm --prefix . run build` | exit 0 | `build.log` / `build.exit` |
| `python3 tests/verification/desktop/verify_wr_504.py` | 41/41 通过 | `verify_wr_504.log` / `verify_wr_504.json` |
| `python3 tests/verification/desktop/verify_work_record_ui_301.py` | 15/15 通过 | `verify_work_record_ui_301.log` / `.json` |
| `python3 tests/verification/desktop/verify_wr_503.py` | 58/58 通过 | `verify_wr_503.log` / `verify_wr_503.json` |

## UI 参考

Timepage 的左侧细日期轴 + 右侧事件块、AllWork 的日期节点 + 内容卡、Feedly 的内容优先列表。
本轮只借用「左侧细轴节点、右侧当日卡片纵向排布、组头仅日期/条数/摘要入口」三点，沿用现有 Aetheria 令牌。

## 实现判断

- 分组发生在宿主返回的 `listing.items` 之后，筛选仍由 `shell.workRecordList(filter)` 完成；空组不生成。
- 组内排序由纯函数按 `created_at` 倒序兜底，同刻按 `id` 倒序，与宿主排序方向一致。
- 当天组保留 503 的「生成当日摘要 / 查看摘要」；非当天组只按需读取已有摘要，不提供生成入口。
- 旧数据：`category=null` → 「未分类」；`source/raw_source` 都为空 → 省略来源；空正文 → 「（无正文）」；坏 `created_at` → 只显示记录日期。

## 限制

- 证据为编译、构建、纯函数运行与静态源码检查；未在真实用户数据窗口里做人工视觉验收。
- 非当天摘要按展开时读取；本次未改动摘要落盘、详情、编辑、删除、解析与宿主数据层。
