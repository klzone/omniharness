# 文稿生成设计（OD-WT-600 规划成果二）

> 状态：已批准。配套任务卡见 PHASE4_IMPLEMENTATION_PLAN.md。

## 一、WritingTask 数据模型（D-083）

work/写作/<writing_id>.md（schema omnidesk.writing.v1，Markdown+frontmatter 照 material_store 先例）：

- writing_id / title / document_type（十二类白名单起步）/ writing_purpose / audience / writing_period / topic / requirements / expected_length / writing_style
- selected_material_ids / selected_knowledge_ids（素材包挂载）
- document_id / status（九态）/ created_at / updated_at
- ## 状态记录 追加段（迁移留痕）

九态：需求待整理/素材收集中/大纲待确认/写作中/待核验/待审核/已定稿/已归档/已取消。迁移合法性表约束（大纲待确认→写作中须经大纲确认动作；归档仅从已定稿）；非法迁移拒绝并留痕。与正式任务零外键。

## 二、文稿与版本（D-084）

work/写作/<wid>/ 下：outline.md + 正文.md + versions/v<n>_<label>.md + manifest.json。版本快照在关键节点（大纲确认/章节确认/修订应用/定稿）由宿主写入；manifest 记版本号/时间/来源 user|ai/范围/说明；恢复=旧版内容落为新当前版+新快照，历史不删。段落级差异对比（空行分段比对），不引外部 diff 库。

## 三、素材包（D-085，不设独立实体）

素材包 = WritingTask frontmatter selected_* 字段 + 大纲章节→素材映射派生视图。用户操作：采纳/排除/标注重点/写作说明（605 IPC）。五类区分标注：已确认事实/AI 推荐/待审核/仅供参考知识/用户外部资料。

## 四、大纲与分章写作（606/607）

大纲：宿主组提示词（需求+素材包+类型模板），候选暂存→用户确认（可编辑）→outline.md+状态迁移+快照。四套类型模板（总结/交流/调研/汇报）落 config/writing-templates.v1.json。

分章：每章提示词=确认大纲节+该章素材+知识依据+上下章摘要（不全量塞历史）；material-writing Skill 四分法（事实/分析/判断/建议）进提示词；禁止清单照方案 §六（不虚构成果/领导讲话、计划不写成完成、相关性不写成因果、不空洞凑篇幅）。章节稿候选→用户确认→写入正文节+快照+citations.jsonl。全文按大纲顺序组合，未确认章节显式占位。

## 五、审阅修改（607 revise / 608）

十维审阅检查单（事实/数据/制度表述/逻辑/衔接/重复/表达/规范/篇幅/与要求一致性）作为 AI 审阅提示词；修改指令（选中段落/章节/精简/扩展/语气/术语/消除重复）产出修订建议候选，应用时新建版本快照，原文保留。版本对比/恢复 IPC。

## 六、导出引擎与样式包（D-089 修订版，613）

导出时序：定稿→导出（可多格式多次）→归档（MD）。导出回执写 manifest。

| 格式 | 引擎 | 样式 |
|------|------|------|
| Markdown | 宿主纯函数（元数据剥离+三态：正式稿/带来源审阅稿/含核验工作稿） | 无需 |
| Word 公文版 | DSH office-docx 技能（D-042 LibreOffice Kit）优先；textutil 兜底（简化版如实标注） | config/writing-export-styles/gongwen：一/（一）/1./（1）标题层级、仿宋正文、黑体标题、国标页边距 |
| PDF 阅读版 | soffice headless；macOS 打印流 fallback | 大字号宽行距窄版心、页内脚注 |
| 公众号 HTML | 宿主确定性渲染（Markdown→HTML+内联样式） | 移动端行高、卡片分段、强调样式，粘贴即用 |

样式包落 config/writing-export-styles/（仓库外可编辑，照 flow_pack 先例），三套起步不做通用排版引擎。正式稿剥离内部 ID/AI 调试信息（断言测试）。导出前敏感词扫描（场景十三）。归档仅 MD 干净版；DOCX/PDF/HTML 落用户导出目录不入 vault。

## 七、归档与知识回流（614）

writing_archive：校验已定稿→MD 走 vault_archive 报告目录→归档记录关联写作任务/素材/知识/核验结果/版本历史/导出清单→状态已归档。归档页软引导（先导出后归档的清单提示），硬门禁仅校验已定稿。知识回流：用户勾选可复用内容→既有治理流程，不自动转条目。

