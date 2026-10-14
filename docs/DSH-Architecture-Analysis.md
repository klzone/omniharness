# Yvan AI Workbench — DSH 架构分析与实施方案

> 基于 DeepSeek Harness v0.2.1-alpha.2 源码分析
> 分析日期: 2026-10-14

---

## 第一部分：DSH 架构分析报告

### 1. Runtime 整体结构

DSH 基于 Cordis 框架构建，核心设计哲学是 **"一切皆插件"**（everything-is-a-plugin）。没有特权核心代码——模型适配器、工具注册表、会话日志、Agent 循环本身都是可替换的插件。

**运行时组件清单（52 个 package group）：**

| 层级 | 核心组件 | 职责 |
|------|---------|------|
| 基础 | core/agent, core/agent-loop, core/session, core/tools, core/system-prompt | Agent 生命周期、会话日志、工具管道、提示词组装 |
| 模型 | llm/llm, llm-deepseek, llm-pi-ai, llm-retry | 模型适配器抽象、多厂商路由、重试策略 |
| 执行 | shell, subprocess, sandbox, terminal | 命令执行、子进程管理、沙箱隔离 |
| 持久化 | session/*, storage/*, attachment | 会话存储、KV 存储、附件管理 |
| 扩展 | skill, mcp, plugin-manager, extensions | Skill 加载、MCP 外部工具、插件管理、运行时自修改 |
| 应用 | boot/app-boot, bundle/*, preset/*, host, client | 启动流程、配置分层、Agent 预设、GUI 宿主 |

**关键架构概念：**

DSH 的配置通过 YAML 分层组合，从空列表开始，依次叠加 Bundle 补丁、Profile 补丁、Home 补丁和命令行 overlay。每一层可以通过 id 定位某行并替换其整个 config，也可以插入新行。这意味着你可以在不修改源码的情况下替换或禁用任何组件。

---

### 2. Profile 机制深度分析

#### 存放位置

    ~/.dsh/profiles/
    ├── desktop/          ← Desktop 应用独占，Electron 进程锁定
    │   ├── package.json  ← 声明使用的 bundles
    │   ├── cordis.yml    ← 空数组（真正的配置来自补丁层）
    │   ├── cordis.patch.yml  ← 用户自定义配置（最重要的文件）
    │   ├── node_modules/ ← 已安装的插件
    │   └── .plugin-manager/
    ├── headless/         ← 无头模式
    └── web/              ← Web UI 模式

#### 文件结构

package.json 声明 Profile 使用哪些 Bundle：

    {
      "dsh": {
        "profile": {
          "bundles": [
            "@deepseek-ai/dsh-base",
            "@deepseek-ai/dsh-web-app",
            "@deepseek-ai/dsh-experimental-inspector-profile"
          ]
        }
      }
    }

cordis.patch.yml 是用户定制的核心文件。你当前的配置已经在做以下事情：
- 设置默认模型为 Agnes 3.0 Flash（通过 agnescom provider）
- 配置 Agnes.cn 和 agnes.com 两个 API 端点
- 配置了 DeepSeek 官方 Web 搜索
- 隐藏了设置面板、设置了 UI 偏好

#### 加载流程

1. dsh CLI 或 Desktop Host 读取 DSH_HOME/profiles/<name>
2. 加载 package.json 中声明的 bundles（按顺序）
3. 每个 bundle 提供一份 cordis.patch.yml
4. 应用 Profile 目录下的 cordis.patch.yml（你的定制层）
5. 应用 Home 级补丁和 --patch 命令行叠加
6. 后面的层通过 id 覆盖前面的行

#### 可覆盖的能力

理论上可以覆盖一切，因为所有组件都是通过 id 索引的插件行。实际常用的定制点：

- agent-default-model: 默认模型和推理等级
- llm-pi-ai: 添加/修改模型 providers（OpenAI、Claude、Gemini 等）
- ui-settings-*: 各种 UI 设置
- web-search-*: 搜索配置
- 任何工具的 disabled 状态
- 通过 insert 注入全新插件

#### 如何创建自定义 Profile

    # 从模板创建
    dsh --profile yvan-work --from-default-profile web
    # 或手动创建目录
    mkdir -p ~/.dsh/profiles/yvan-work

> **结论：Profile 机制非常适合做个人工作台的定制层。** 你的所有模型配置、UI 偏好、工具开关都可以通过一个 cordis.patch.yml 文件管理，而且不触碰 DSH 源码。

---

### 3. Plugin 机制深度分析

#### Plugin 目录结构

插件本质上是一个 npm 包，核心结构：

    my-plugin/
    ├── package.json      ← 声明 dsh.bundle（如果是 bundle 类型）
    ├── cordis.patch.yml  ← bundle 提供的配置补丁
    ├── src/index.ts      ← 插件入口，导出 Cordis apply() 函数
    └── skills/           ← 可选：附带的 Skill 文件

#### Plugin 声明与生命周期

在 cordis.patch.yml 中通过 Cordis 的行格式声明。插件通过 ctx.effect() / ctx.on() 注册能力——所有注册都是可逆 effect，插件卸载时自动回收。

生命周期：发现 → 解析（ESM import）→ 激活（apply）→ 运行 → 停用（effect 回收）→ 卸载

#### Plugin 升级风险

| 风险 | 等级 | 说明 |
|------|------|------|
| DSH 升级导致插件不兼容 | 高 | peer 版本检查会阻止不兼容插件加载 |
| 插件互相冲突 | 中 | Cordis 的 service 注册有冲突检测 |
| 安装过程损坏 Profile | 低 | Plugin Manager 有 manifest/lockfile 快照回滚 |
| 第三方插件质量不一 | 中 | 插件代码在 Host 进程内执行，无沙箱隔离 |

> **结论：Plugin 机制成熟，但 DSH 仍在 developer preview，API 可能有破坏性变更。** 建议初期以 Skill（纯 Markdown 指令）为主要扩展方式。

---

### 4. Session 机制深度分析

#### Session 保存位置

    ~/.dsh/sessions/
    ├── --home-kevin--/               ← 按工作目录编码
    │   ├── session-<uuid>/
    │   │   ├── session.v4.jsonl.zstd   ← v4 格式，Zstandard 压缩 JSONL
    │   │   ├── session.v4.jsonl.zstd.bak-*  ← 自动备份
    │   │   └── session.lock             ← 进程锁
    │   └── ...

#### 数据格式

- 格式：JSONL（每行一个 JSON 事件），Zstandard 压缩
- 版本：v4，历经 v0→v1→v2→v3→v4 迁移链
- 事件类型：turn/start、user/message、assistant/message、tool/call、tool/result 等
- 设计原则：append-only，模型看到的一切都必须能从日志重建

#### 恢复能力

- 崩溃恢复：JSONL 后端支持 torn-tail 修复
- 格式迁移：自动通过迁移链转换到当前格式
- checkpoint 策略：在模型请求前、工具执行前都有持久化检查点
- 手动恢复：没有内置的"恢复已删除 Session"功能

> **结论：Session 丢失的根本原因可能是 Desktop 进程意外退出时 checkpoint 未完成。** DSH 有修复能力但没有用户级自动备份。这是 Memory Agent 要解决的核心问题。

---

### 5. 模型管理

DSH 通过 llm-pi-ai 插件实现多厂商路由。你当前已配置 Agnes 和 DeepSeek。

可以在 cordis.patch.yml 中添加的 provider 配置示例：

    - id: llm-pi-ai
      name: "@deepseek-ai/dsh-llm-pi-ai"
      config:
        providers:
          openai:
            apiKeyEnv: OPENAI_API_KEY
          anthropic:
            apiKeyEnv: ANTHROPIC_API_KEY
          gemini:
            displayName: Google Gemini
            api: openai-completions
            apiKeyEnv: GEMINI_API_KEY
            baseURL: https://generativelanguage.googleapis.com/v1beta/openai
            models:
              - id: gemini-2.5-flash
                name: Gemini 2.5 Flash
          glm:
            displayName: 智谱 GLM
            api: openai-completions
            apiKeyEnv: GLM_API_KEY
            baseURL: https://open.bigmodel.cn/api/paas/v4
            models:
              - id: glm-4-flash
                name: GLM-4-Flash

---

## 第二部分：定制路线建议

### 1. DSH 作为 Workbench 底座的可行性

**适合，但需要清醒认识边界。**

适合的原因：
- Cordis 的 "一切皆插件" 架构提供了极强的可定制性
- Profile/Bundle/Patch 分层设计天然支持"不修改核心"的定制模式
- 多模型路由已经解决了多厂商接入
- Skill 系统支持纯 Markdown 的轻量扩展
- Session 持久化和事件驱动架构为 Memory Agent 提供了基础

需要注意的：
- DSH 仍在 developer preview，会有破坏性变更
- Desktop 和 CLI 的插件是隔离的
- 没有内置的个人知识库/记忆机制

### 2. 继承与外置决策

**应该继承的：**

| 继承项 | 方式 | 原因 |
|--------|------|------|
| Runtime 引擎 | 直接使用 | Agent loop、工具管道、模型路由都很成熟 |
| Profile 体系 | 创建专属 Profile | 所有定制通过 cordis.patch.yml |
| Credential 管理 | 直接使用 | API Key 管理已经做得很好 |
| Session 持久化 | 直接使用 + 外部备份 | 核心能力完善 |
| Skill 系统 | 编写自定义 Skill | 最安全的扩展方式 |
| Web UI | 直接使用 | 不建议自己做 UI |

**应该外置的：**

| 外置项 | 位置 | 原因 |
|--------|------|------|
| 个人知识库 | ~/YvanAI/knowledge/ | 不能绑定 DSH 数据格式 |
| Memory 数据 | ~/YvanAI/memory/ | 跨工具持久，通用格式 |
| 项目模板 | ~/YvanAI/templates/ | 可被多个工具共用 |
| 备份 | ~/YvanAI/backups/ | Session 内容的结构化提取 |

### 3. 升级防护策略

四层防御：

1. **双环境隔离**：稳定版 Desktop 日常使用，源码 Web 版测试新版本
2. **配置版本控制**：cordis.patch.yml 纳入 Git 管理
3. **数据独立**：~/YvanAI/ 用 Markdown/JSON 通用格式存储
4. **Skill 优先**：纯 Markdown 指令不受 Plugin API 变更影响

### 4. 第一版最小实现路径

**Step 1：环境搭建与配置管理（第 1 周）**
1. 初始化 MyWorkStation Git 仓库
2. 创建 ~/YvanAI/ 数据目录结构
3. 将 cordis.patch.yml 纳入版本控制
4. 补充缺失模型 provider 配置
5. 编写配置同步和 Session 备份脚本

**Step 2：核心 Skill 开发（第 2-3 周）**
1. memory-agent Skill：每次重要对话后提取决策/状态/TODO 到外部 Markdown
2. knowledge-agent Skill：搜索和引用 Obsidian vault 和本地文件
3. project-agent Skill：读取项目文档生成状态报告

**Step 3：工作流整合（第 4 周）**
1. writing-agent Skill：整合知识库 + 记忆 → 生成工作材料
2. Session 自动备份
3. 完善文档

---

## 第三部分：风险分析

### DSH 版本升级风险 ⚠️ 高

| 风险点 | 影响 | 缓解措施 |
|--------|------|----------|
| Plugin API 变更 | 自定义 Plugin 失效 | 优先使用 Skill |
| Session 格式变更 | 旧 Session 需迁移 | DSH 有完整迁移链 |
| cordis.patch.yml 语法变更 | 配置失效 | Git 版本控制 |
| Bundle 结构变更 | Profile 无法启动 | 保留最后可用的安装包 |

### Plugin 兼容风险 ⚠️ 中

第三方 Plugin 运行在 Host 进程内无沙箱。建议初期不开发 Plugin，只用 Skill。未来如需 Plugin，优先做成独立 MCP Server。

### 数据迁移风险 ⚠️ 低

前提是 ~/YvanAI/ 目录结构和文件格式从第一天就设计好，用 Markdown/JSON 通用格式。

### Session 稳定性风险 ⚠️ 中高

可能原因：Desktop 进程异常退出、lock 文件冲突、压缩文件写入不完整、格式迁移失败。

缓解措施：定期备份 sessions 目录、Memory Agent 提取重要内容到外部文件。

---

## 第四部分：当前环境状态

| 资源 | 路径 | 状态 |
|------|------|------|
| DSH v0.2.1-alpha.2 源码 | /home/kevin/dsh-build/ | 已解压，部分编译 |
| DSH Desktop Profile | ~/.dsh/profiles/desktop/ | 已配置 Agnes 模型 |
| Credentials | ~/.dsh/.credentials.yaml | 已配置 Agnes, DeepSeek 等 |
| Sessions | ~/.dsh/sessions/ | 多个工作区约 30 个 Session |
| User Skills | ~/.dsh/skills/ | 空，待创建 |
| DSH CLI | ~/.local/bin/dsh | 需要 Desktop 先运行才能初始化 |
| MyWorkStation | ~/Code/MyWorkStation/ | 空，待初始化 |

---

## 附录：关键文件位置速查

    # DSH 核心配置
    ~/.dsh/profiles/desktop/cordis.patch.yml    ← 你的定制配置
    ~/.dsh/profiles/desktop/package.json        ← 使用哪些 bundle
    ~/.dsh/.credentials.yaml                    ← API Key 存储

    # DSH 数据
    ~/.dsh/sessions/                            ← 所有会话数据
    ~/.dsh/storages/workspace.json              ← 工作区索引
    ~/.dsh/storages/cost-meter/ledger.json      ← 使用量统计

    # DSH 扩展
    ~/.dsh/skills/                              ← 用户自定义 Skill

    # 你的工作台（待创建）
    ~/YvanAI/knowledge/                         ← 个人知识库
    ~/YvanAI/memory/                            ← 长期记忆
    ~/YvanAI/projects/                          ← 项目管理数据
    ~/YvanAI/templates/                         ← 模板
    ~/YvanAI/backups/                           ← 备份

    # 项目仓库（待初始化）
    ~/Code/MyWorkStation/                       ← Git 仓库
