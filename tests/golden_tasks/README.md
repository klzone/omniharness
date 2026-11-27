# OmniDesk Discipline 黄金任务验证基准集 (Golden Tasks v0.2)

本基准集收录了 **40 个** 面向纪检监察综合履职场景的**基准任务**（GT-01 ~ GT-40），是衡量工作台可靠性、升级回归及模型质检的基准依据。

> **素材边界（见 [GOALS.md](../../GOALS.md)、[授权与数据流审计](../../docs/OmniDesk-Authorization-and-DataFlow-Gap-Audit.md)）**：任务内容为**合成/公开口径的场景描述**，不是、也不得替换为真实工作材料的脱敏副本。在取得适用授权前，本基准集只用于验证**技术与评测流程**是否走通；其结果**不得**用于宣称真实业务收益、真实材料准确率或真实场景效率提升。

## 任务品类覆盖分布 (共 40 项)
1. **材料起草类 (GT-01 ~ GT-10)**：工作总结、专项汇报、专题研讨发言、心得体会、建议书整改督查。
2. **法规条文精确检索类 (GT-11 ~ GT-20)**：四种形态、问责方式、处分影响期、形式主义定性、检举控告分流等。
3. **文稿审查与去 AI 润色类 (GT-21, GT-30, GT-31, GT-32)**：文风审查、去套话、法规合规质检。
4. **领导致辞与交流研讨 (GT-22, GT-23)**：全员警示教育讲话、纪律检查建议书交办动员。
5. **深度调研与机制创新 (GT-24, GT-25)**：小微权力微腐败治理、风腐同查同治机制。
6. **PPT 展示大纲提炼 (GT-26, GT-27)**：纪律处分条例培训提纲、年度履职汇报 PPT 大纲。
7. **业务学习与理论感悟 (GT-28, GT-29)**：自我革命战略思想体会、纪法贯通业务实践心得。
8. **重点法规业务纵深检索 (GT-33 ~ GT-40)**：干预司法、公款旅游、容错纠错、陈述申辩、澄清正名、建议书时限。

## 验收铁律
- 材料与审查类：严格杜绝假大空，三级大纲清晰，事实量化扎实。
- 知识检索类：严格四要素（答案 + 原文引用 + 来源文件 + 引用位置），准确率底线 ≥ 95%。

## Work Review A/B 对照

`work-review/ab_runner.py` 用同一批 12 道题、同一把尺子比较两版提示词或两批已有输出。

跑两版提示词时，每道题调用一次 runner。runner 从 stdin 读取 `{case_id, variant, prompt}`，把候选 JSON 写到 stdout，日志走 stderr。
runner 可以是任何本地模型 CLI、HTTP 包装或测试替身，只要遵守这个 stdin/stdout 协议。

```bash
python3 tests/golden_tasks/work-review/ab_runner.py \
  --variant baseline=variants/baseline.txt \
  --variant strict=variants/strict.txt \
  --runner-cmd "python3 my_model_runner.py"
```

只比较已有输出时，不需要 runner。

```bash
python3 tests/golden_tasks/work-review/ab_runner.py \
  --score baseline=/tmp/run-a \
  --score strict=/tmp/run-b
```

输出是一张 Markdown 表，列出事实引用、条数、需复核、重复匹配、敏感拒绝、知识提示六项准确率和平均耗时。脚本默认把中间输出写到临时目录，只打印分数与耗时，不打印提示词或候选正文。

这个对照只用于合成基准上的相对比较，不代表真实业务材料的绝对质量。
