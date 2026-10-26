---
name: anysearch
description: OmniDesk 的联网检索薄包装。转发到本机已安装的 anysearch CLI，用于规范、公开资料与新闻查询；不复制上游脚本，不读取或打印密钥。
user-invocable: true
---

# 联网检索 (AnySearch)

只在需要**当前信息或公开来源**时使用：查规范、政策原文、公开报道、产品文档、事实核对。纯凭已有材料能回答的问题不要检索。

## 一、调用方式

本 Skill 是薄包装，实体位于本机 Codex 技能目录，不在本仓库内。默认位置：

```
ANYSEARCH_SKILL_DIR = $HOME/.codex/skills/anysearch
```

需要改位置时，由运行环境提供 `ANYSEARCH_SKILL_DIR` 指向含 `scripts/anysearch_cli.py` 的目录。命令：

```bash
python3 "${ANYSEARCH_SKILL_DIR:-$HOME/.codex/skills/anysearch}/scripts/anysearch_cli.py" search "查询词" --max_results 5
```

其他常用形状（输出为 JSON，字段以 CLI 实际返回为准）：

```bash
python3 "$ANYSEARCH_SKILL_DIR/scripts/anysearch_cli.py" get_sub_domains --domain finance
python3 "$ANYSEARCH_SKILL_DIR/scripts/anysearch_cli.py" extract "https://example.com/page"
```

不要编造 `--format` 之类不存在的参数；参数报错时先跑该子命令的 `--help`。

## 二、凭据与边界

- CLI 自行从它所在目录加载 `.env`，也接受环境变量与 `--api_key`。**不要读取、打印、复制 `.env`，不要把密钥写进查询、仓库、日志或成果文件。**
- 无密钥可匿名低限额检索；额度耗尽、服务报错或网络失败时如实说明，**不得伪造检索结果**。是否改用其他检索方式，按当时可用工具与用户许可决定。
- 查询词与提取的 URL 会发给外部服务。案件、线索、人物画像、未公开调查信息、身份证号、手机号等一律不检索；确需检索时先去掉可识别信息，并只保留完成检索所必需的词。
- 检索结果默认只作当次回答的依据。写入库或成果前按现有确认流程处理，不自动落盘。

## 三、结果呈现

1. 先给结论，再列出实际使用的来源链接与日期。
2. 区分「检索结果原文」「自己的归纳」与「未核实」；没有检索到就直说没有，不用模型记忆补齐。
3. 同一问题检索一次即可，不重复刷同一查询。需要多个独立事实时，用 `batch_search` 一次并行。

## 四、垂直域

金融、学术、医疗、法律、代码、安全、专利、能源、环境、农业、社交平台等主题，先 `get_sub_domains --domain <域>`，再按其返回的 `sub_domain` 与必填参数调用；不确定是否属于垂直域时，用一条通用查询加一条垂直查询并行。

## 五、不可用时

若上述目录或 CLI 不存在，明确回报「AnySearch 未安装或路径不可用」，不要尝试安装、下载或改写上游技能包，也不要凭空返回检索结果。

