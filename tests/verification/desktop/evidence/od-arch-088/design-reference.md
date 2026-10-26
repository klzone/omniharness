# OD-ARCH-088 界面参考记录

- 用途：OD-ARCH-035 要求涉及界面的实现先检索同类产品布局，并用不超过两句记录借鉴点。
- 检索工具：`screenshots-design-inspiration` MCP（`search_inspiration`）。
- 检索关键词：`clarifying question option chips inline chat`、`chat assistant suggestion chips quick replies`、`approval prompt options card`。
- 参考来源（图库）：
  - ChatGPT（Web）：https://screenshots.designknowledgebase.com/Web/ChatGPT/
  - Claude（Web）：https://screenshots.designknowledgebase.com/Web/Claude/
  - Catalant（Web，含 Approve Extension Proposal 审批页）：https://screenshots.designknowledgebase.com/Web/Catalant/

## 借鉴要点

提问卡直接嵌在对话流里、紧贴触发它的助手消息，不做居中模态；选项做成整行可点的列表项（标题 + 一行灰色说明），选中只用细边框与淡底，并沿用现有审批卡的圆角、间距和字号刻度。

## 落地对应

- 组件：`desktop/src/shell/QuestionCard.tsx`；渲染位置在 `HomeChat.tsx` 的助手消息与审批卡同一区域。
- 样式：复用现有 `bg-surface-container`、`rounded-lg`、`text-[12px]` 等既有刻度，未新增配色或组件体系。
- 交互：选项为原生 radio / checkbox，整行 `label` 可点；点选只改本地选择，必须点「提交回答」才回传。

证据等级：工具检索结果 + 文件事实（组件与渲染点代码）。
