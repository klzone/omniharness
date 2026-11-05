# 交接文件模板与样例（已批准生效）

> 状态：现行（2026-11-05 用户批准激活规范）。来源：docs/development/CONTEXT_MANAGEMENT_STANDARD.md §3.3、OMNIDESK_ENGINEERING_STANDARD.md §1.2。
> 配套约束：任务卡以 docs/BUILDER-Task-Instruction-Template.md 现行模板为底稿（其"单一动作、可独立验收"等条款不变），本模板补充规范新增字段（架构契合、裁决记录、豁免声明）；执行会话必须先有获批任务卡才能开工。

## 一、交接文件固定字段（约一屏，40 行内）

    # 交接：OD-ARCH-xxx
    阶段：<已完成 / 进行中 / 受阻>
    已完成：<任务编号列表，一行一个>
    未完成：<任务编号 + 卡点一句话；无则写"无">
    修改文件：<相对仓库根的路径列表>
    测试结果：<命令 + 通过数/退出码，仅本轮实跑>
    Git：<HEAD 短哈希 + dirty 文件数>
    技术决策：<新增决策 + decisions.md 条目号；无则写"无">
    风险：<已知限制与未验证项>
    下阶段入口：<下一个任务卡位置 + 必读文件清单>

规则：不复制聊天历史；不贴长日志（测试只留结论行）；层 4/层 5 上下文（迭代输出、会话历史）不进交接文件。位置由 GLM 在派工时指定（证据目录或 docs/）。

## 二、标准样例（真实数据，2026-11-05 实测）

    # 交接：OD-ARCH-137
    阶段：已完成（REVIEW 待 GLM 复核结论）
    已完成：137 全部实施与验证（启动播种、三条模型路径兜底、seed 合并语义、12 项验证脚本）
    未完成：无
    修改文件：desktop/src-tauri/src/lib.rs；desktop/src-tauri/src/engine_supervisor.rs；
      tests/verification/desktop/verify_startup_seed_137.py（新增）；
      tests/verification/desktop/evidence/od-arch-137/（新增）
    测试结果：verify_startup_seed_137.py 12/12 通过；cargo test --offline --lib 509 passed / 0 failed；
      DSH 转储实测空 home 退出 1（复现）、seed home 退出 0
    Git：HEAD 98a7e27（已提交），dirty 1 项（docs/development/ 未跟踪，与本任务无关）
    技术决策：无新增（沿用既有 seed 不覆盖用户文件原则）
    风险：0.2.9 发布后全新机器完整路径（下载→首启→设置模型）仍需用户实测；
      decisions.md 尚无 137 条目（属决策级变更时补登记）
    下阶段入口：无待办任务卡；下一任务启动时从 GOALS.md / PLANS.md 现行小节进入

样例说明：此样例数据全部来自 STATUS.md OD-ARCH-137 小节与当日 git 实测，仅作格式示范；不是本日新增验证，不作为 137 的验收证据使用。
