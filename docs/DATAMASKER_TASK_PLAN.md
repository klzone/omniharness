# DataMasker Task 1 实施任务卡（OD-ARCH-139 · 待批准后交实施）

> 运行目录统一：cd tests/verification/engine-interface/adapter
> 测试文件：test_security_masker_unit.py（unittest，全部虚构数据）
> 每步验收 = 指定 -k 前缀全部通过 + 脚本 docstring 声明的最小断言数达标 + 退出码 0

## S1 词典模块
- 动作：security/ 包骨架 + DictEntry + load_dictionary
- 文件：engine/omnidesk_engine/security/__init__.py、masker.py；config/sensitive-dict.sample.v1.json
- 验收：python3 -m unittest -v test_security_masker_unit -k Dict
- 断言要点：样例可加载；schema 名不符拒绝；重复 value+type 合并 aliases；非法 type 拒绝并报行号；文件缺失时降级为纯规则模式且 stats.warn 记录
- 前置：无

## S2 规则识别器
- 动作：四类正则 + GB11643 / Luhn 校验
- 验收：python3 -m unittest -v test_security_masker_unit -k Rule
- 断言要点：校验码合法的虚构证件命中、校验码非法的 18 位不命中；Luhn 同理；手机号前后贴数字不命中；邮箱超长不命中；18 位证件优先于银行卡/手机号
- 前置：S1

## S3 占位符与 MaskMapping
- 动作：生成/复用/碰撞重试/repr 脱敏
- 验收：python3 -m unittest -v test_security_masker_unit -k Placeholder
- 断言要点：同文本两处同名同占位符；两次独立 mask 占位符不同；repr 不含原文；占位符格式匹配 2.7 正则
- 前置：S1

## S4 mask 主体与重叠处理
- 动作：统一扫描、(start,-length) 排序替换、stats
- 验收：python3 -m unittest -v test_security_masker_unit -k Mask
- 断言要点：重叠长者胜；同名同 type 单占位符；同名不同 type 各自占位符；空串/None/超长(1MB)安全返回；词典与规则同段落叠加
- 前置：S2 S3

## S5 unmask + validate_mapping
- 动作：精确还原、异常占位符处理、完整性报告
- 验收：python3 -m unittest -v test_security_masker_unit -k Unmask
- 断言要点：精确还原往返一致；被改占位符保留原样并计入 modified_candidates；未知占位符保留并计入 unknown；用另一请求的 mapping 还原 → 全部 unknown；validate 不改文本
- 前置：S4

## S6 SessionScope
- 动作：会话级占位符一致性与隔离
- 验收：python3 -m unittest -v test_security_masker_unit -k Session
- 断言要点：同 scope 两次 mask 同实体同占位符；不同 scope 占位符不同；scope 与 request mapping 互不污染
- 前置：S4

## S7 收口
- 动作：全量单测 + 既有引擎单测回归
- 验收：python3 -m unittest -v test_security_masker_unit（全量）&& python3 -m unittest -v test_process_owner_unit && python3 -m unittest -v test_web_api_unit
- 断言要点：三个命令退出码均 0；grep 确认 engine/omnidesk_engine/ 无新增第三方 import；STATUS.md 更新；REVIEW.md 待 ARCHITECT 复核
- 前置：S1–S6

## 验收红线
- 不得引入第三方依赖；不得修改 Rust 侧；不得触碰 LLM 调用链；不得改动 config/ 现有文件（只新增 sample）。
- 全部测试数据虚构；测试输出不得打印原文（用断言而非 print 验证）。
