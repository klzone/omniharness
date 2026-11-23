# OD-GROW-301 H6+G11 派工：回归集与指标

## 任务编号与名称
OD-GROW-301 / H6+G11：习惯提取回归框架 + 习惯页指标（只读展示）

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（H6/G11 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：desktop/src-tauri/src/habit_extract.rs（若需把表驱动样例共享为回归入口，只加不改）；新增 tests/verification/desktop/run_habit_regression.py（回归脚本：跑 cargo habit_extract 测试 + habit_checks 测试 + Python habit_extract_test.py，任一失败退出非零）；desktop/src/shell/HabitCardsPage.tsx（加指标只读区，≤30 行）；desktop/src/metricsView.ts + metricsView.test.mjs（新增，指标计算纯函数）；证据 tests/verification/desktop/evidence/od-grow-301/h6-g11/
- 明确不做的事：不改既有测试断言；不做拦截逻辑；不改 engine/ 提示词内容

## 功能要求
1. 回归脚本 run_habit_regression.py：依次运行
   - cd desktop/src-tauri && cargo test --lib --offline -- habit_extract（全部样例）
   - cargo test --lib --offline -- habit_checks
   - cd engine && python3 -m unittest discover -s omnidesk_engine -p habit_extract_test.py
   任一失败 exit 1；输出汇总 PASS/FAIL 与数字。这是后续「改动提取提示词/白名单/校验逻辑/规则种类/SKILL 必须先跑」的回归入口。
2. metricsView.ts 纯函数：输入 {candidates: {accepted, rejected, reasons: Record<code, number>}, violations: {cardId: {count, lastDate}}, consolidations: {success, fail, tokens?}}，输出指标卡数据 {acceptRate, rejectionTop, violationTrend: number[], consolidationSuccessRate}。数值缺失显示 —，不估算。
3. HabitCardsPage.tsx：底部加只读指标区（接受率、驳回原因 Top3、违反趋势最近 7 次、整理成功率）。数据来源：G1 的候选决定记录与统计 jsonl——宿主未提供统计读取命令时，用 habits.activeList() + localStorage 的 lastInjected 近似，缺失项如实显示「待 G11 宿主命令」；不伪造数字。
4. metricsView.test.mjs：接受率计算（0 分母 → null）、Top3 排序、趋势窗口、缺失显示 —。

## 验证
- cd desktop && npm run test（passed ≥ 124+新增）；npx tsc --noEmit；npm run build
- python3 tests/verification/desktop/run_habit_regression.py 全过
- 证据落 tests/verification/desktop/evidence/od-grow-301/h6-g11/verify_output.txt

## 硬约束
- 不修改 runtime/、engine/ 既有代码（回归脚本调用它们）；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；cargo/npm passed 数不得减少；同一失败测试修两次不过即停
- 指标区只读，不拦截任何流程；缺失数据如实显示，不伪造

## 交付
- 改动文件清单 + 测试数字 + 回归脚本用法说明
