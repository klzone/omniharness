---
name: anysearch
description: OmniDesk 的联网检索薄包装。转发到本机已安装的 AnySearch CLI，用于规范、公开资料与新闻查询；不复制上游脚本，不读取或打印密钥。
user-invocable: true
---

# 联网检索

## 一、用途

只在需要当前信息或公开来源时使用：查规范、政策原文、公开报道、产品文档、事实核对。纯凭已有材料能回答的问题不要检索。

## 二、触发与排除

- 触发：需要 2025 年之后的新信息；核对公开链接是否仍可访问；查规范与公开报道的原文。
- 排除：材料里已有答案；内部记录、私人材料；案件、线索、人物画像、未公开审查调查信息。

## 三、调用

本 Skill 是薄包装，实体位于本机 Codex 技能目录，不在本仓库内。默认位置：

```
ANYSEARCH_SKILL_DIR = $HOME/.codex/skills/anysearch
```

需要改位置时，由运行环境提供 `ANYSEARCH_SKILL_DIR`，指向含 `scripts/anysearch_cli.py` 的目录。

```bash
python3 "${ANYSEARCH_SKILL_DIR:-$HOME/.codex/skills/anysearch}/scripts/anysearch_cli.py" search "查询词" --max_results 5
python3 "$ANYSEARCH_SKILL_DIR/scripts/anysearch_cli.py" extract "https://example.com/page"
```

参数以 CLI 实际返回为准；不要编造 `--format` 之类不存在的参数。参数报错时先跑该子命令的 `--help`。

## 四、结果与边界

1. 先给结论，再列实际使用的来源链接与日期。
2. 区分「检索结果原文」「自己的归纳」与「未核实」；没有检索到就直说，不用模型记忆补齐。
3. 同一问题检索一次即可，不重复刷同一查询；需要多个独立事实时用 `batch_search` 一次并行。
4. CLI 自行加载 `.env`。不要读取、打印、复制 `.env`，不要把密钥写进查询、仓库、日志或成果文件。
5. 查询词与 URL 会发给外部服务。案件、线索、人物画像、未公开调查信息、身份证号、手机号一律不检索；确需检索时先去掉可识别信息。

## 五、垂直域

金融、学术、医疗、法律、代码、安全、专利、能源、环境、农业、社交平台等主题，先 `get_sub_domains --domain <域>`，再按返回的 `sub_domain` 与必填参数调用；不确定是否属于垂直域时，用一条通用查询加一条垂直查询并行。

## 六、不可用时

OmniDesk 引擎环境默认没有本机 Codex 技能目录，也没有 `ANYSEARCH_SKILL_DIR` 变量——找不到 `scripts/anysearch_cli.py` 属于预期，不是故障。此时不要按参数报错指引反复重试，也不要搜索安装路径；直接改用可用的联网手段（web_fetch 直连权威来源等），并在结果里如实标注实际使用的渠道与来源限制。

目录或 CLI 不存在时，明确回报「AnySearch 未安装或路径不可用」，不尝试安装、下载或改写上游技能包，也不凭空返回检索结果。

## 七、依赖

脚本需要 Python 的 `requests`（证书链由 `certifi` 提供）。发布包已在 `runtime/python-vendor` 内置，宿主会把该目录加入 `PYTHONPATH`，无需用户 pip；只有开发态缺内置目录时才在引擎所用 Python 中安装。
