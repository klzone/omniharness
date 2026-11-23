# OD-GROW-301 T13 派工：复盘周期关键词结构化

## 任务编号与名称
OD-GROW-301 / T13：PERIOD_REVIEW_KEYWORDS 结构化（{keyword, period, offset} 表）

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（T12 余项、T13、T14 段）与 PLANS.md OD-HARDEN-2043R 的 T13 条目
- 批准时间与批准人：2026-11-23 / 用户（批准本任务书时含 T12 余项、T13、T14）
- 允许修改的文件或目录：desktop/src/workIntent.ts（该常量与相关函数）、desktop/src/workIntent.test.mjs（追加用例）；证据 tests/verification/desktop/evidence/od-grow-301/t13/
- 明确不做的事：不改 workIntentDecision.ts 既有行为；不加新依赖；不改宿主

## 功能要求
1. PERIOD_REVIEW_KEYWORDS 从字符串数组 + slice 下标判断改为 { keyword, period, offset } 结构表；period = weekly/monthly，offset 是相对当月的月偏移（weekly 用 offset 逻辑不同——先读现有实现再定结构，保持行为不变）。
2. 新增词条：「上月复盘」（monthly，上月）等；跨年起止正确。
3. 表驱动单测：每个词条的起止与标签；跨年样例；「插入新词后既有用例仍过」。
4. 全部既有 workIntent 测试保持通过。

## 验证
- cd desktop && npm run test（passed ≥ 129 + 新增）；npx tsc --noEmit
- 证据落 tests/verification/desktop/evidence/od-grow-301/t13/verify_output.txt

## 硬约束
- 不修改 runtime/、engine/、desktop/src-tauri/；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言（可追加）；npm passed 数不得减少；同一失败测试修两次不过即停

## 交付
- 改动文件清单 + 测试数字
