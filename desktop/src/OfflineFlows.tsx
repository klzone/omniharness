// 三个工作场景：周报与随记（weekly）、文案写作（writing）、项目跟踪（project）。
//
// OD-ARCH-026：模型是内容处理步骤。用户输入 + 用户**明确选择**的材料文件（原样）经现有
// 引擎适配链路，由可编辑、带版本的流程包规则驱动起草；结果回到界面供人工编辑、确认、保存。
//
// 硬规矩（本文件只是 UI，真正强制在 desktop/src-tauri/src）：
//   1. 代码只做确定性工作（日期校验、状态计数、逾期判定）与文件边界控制；
//      确定性核对单独成栏，清楚标注“代码计算”，且不作为成果保存；
//   2. 成果只能来自模型初稿；没有模型输出时明确失败/阻塞，不用本地汇总冒充；
//   3. 材料只读用户显式给出的一个文件：不列目录、不递归、不解析数据库；
//   4. 保存必须人工确认，不覆盖已有文件，落仓库外本地数据目录。

import { useCallback, useEffect, useState } from "react";
// 页面局部样式：仅作用于本组件的 .ofl-v2 子树，不影响首页与任务工作台。
import { host, shell } from "./api";
import {
  requestAiDraftCancel,
  runAiDraftSubmission,
  type AiDraftPhase,
  type AiMaterial,
} from "./aiDraft";
import UsageLog from "./UsageLog";
import type {
  MaterialRead,
  OfflineArtifactEntry,
  OfflineFlowCopy,
  OfflineFlowPack,
  OfflineScenario,
  SaveArtifactReceipt,
  WorkflowSample,
} from "./types";

type Tab = "weekly" | "writing" | "project";

/** 单个场景最多加入的材料文件数，避免提示词无界膨胀。 */
const MAX_MATERIALS = 5;

/** 流程包不可用时的兜底文案：只用于说明“配置被拒绝”，不用于生成结果。 */
const FALLBACK_LABEL: Record<Tab, string> = {
  weekly: "周记录 → 周报",
  writing: "一般文案结构",
  project: "项目跟进",
};

const SCENARIO: Record<Tab, OfflineScenario> = {
  weekly: "weekly_report",
  writing: "material_writing",
  project: "project_management",
};

export default function OfflineFlows({ initialTab = "weekly" }: { initialTab?: Tab }) {
  const [tab, setTab] = useState<Tab>(initialTab);
  const [samples, setSamples] = useState<WorkflowSample[]>([]);
  const [pack, setPack] = useState<OfflineFlowPack | null>(null);
  const [packPath, setPackPath] = useState<string | null>(null);
  const [packError, setPackError] = useState<string | null>(null);
  const [packBusy, setPackBusy] = useState(false);
  const [artifactRoot, setArtifactRoot] = useState<string | null>(null);
  const [rootError, setRootError] = useState<string | null>(null);
  const [permissionNote, setPermissionNote] = useState<string>("正在读取模型权限…");

  const [weeklyInput, setWeeklyInput] = useState("");
  const [projectInput, setProjectInput] = useState("");
  const [today, setToday] = useState("");

  const [purpose, setPurpose] = useState("");
  const [audience, setAudience] = useState("");
  const [requirements, setRequirements] = useState("");
  const [facts, setFacts] = useState("");

  // 用户明确选择的材料文件（只读、单文件、有界）。
  const [materials, setMaterials] = useState<MaterialRead[]>([]);
  const [materialPath, setMaterialPath] = useState("");
  const [materialBusy, setMaterialBusy] = useState(false);
  const [materialError, setMaterialError] = useState<string | null>(null);

  // 确定性核对（代码计算，不是成果）。
  const [check, setCheck] = useState<string | null>(null);
  const [checkError, setCheckError] = useState<string | null>(null);
  const [checkBusy, setCheckBusy] = useState(false);

  // AI 初稿：本场景的成果来源。准入关闭时提交会被拒绝，界面只显示真实原因。
  const [aiPhase, setAiPhase] = useState<AiDraftPhase>("idle");
  const [aiTaskId, setAiTaskId] = useState<string | null>(null);
  const [aiStatus, setAiStatus] = useState<string | null>(null);
  const [aiMessage, setAiMessage] = useState<string | null>(null);
  const [aiCancelRequested, setAiCancelRequested] = useState(false);
  const [aiPurpose, setAiPurpose] = useState("");
  const [aiAudience, setAiAudience] = useState("");
  const [aiRequirements, setAiRequirements] = useState("");
  const [draft, setDraft] = useState<string | null>(null);
  const [workspaceDir, setWorkspaceDir] = useState<string | null>(null);

  const [title, setTitle] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [receipt, setReceipt] = useState<SaveArtifactReceipt | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [artifacts, setArtifacts] = useState<OfflineArtifactEntry[]>([]);
  const [opened, setOpened] = useState<string | null>(null);
  const [artifactError, setArtifactError] = useState<string | null>(null);

  const refreshArtifacts = useCallback(() => {
    shell.offlineArtifacts().then(
      (r) => {
        setArtifacts(r.items);
        setArtifactError(null);
      },
      (e) => setArtifactError(String(e)),
    );
  }, []);

  useEffect(() => {
    shell.workflowSamples().then(
      (r) => setSamples(r.samples),
      () => setSamples([]),
    );
    shell.localRootInfo().then(
      (info) => {
        setArtifactRoot(info.resolved ? info.artifact_root : null);
        setRootError(info.resolved ? null : info.error);
      },
      (e) => setRootError(String(e)),
    );
    // 只显示宿主回报的真实权限状态；权限判定与强制执行都在宿主侧。
    host.admissionStatus().then(
      (s) => {
        if (!s.ok || !s.facts) {
          setPermissionNote(s.error?.message ?? "无法读取模型端点信息。");
          return;
        }
        if (s.blocked) {
          setPermissionNote(s.block_reason ?? "权限变更尚未确认，宿主暂时拒绝新提交。");
          return;
        }
        setPermissionNote(
          s.facts.allowed
            ? `模型权限已开启（${s.facts.category}）。宿主在提交时强制执行。`
            : `模型权限未开启（${s.facts.category_reason || "端点未核准"}）；请到「成果目录与模型权限」中开启后重试。`,
        );
      },
      (e) => setPermissionNote(String(e)),
    );
    refreshArtifacts();
  }, [refreshArtifacts]);

  const reloadPack = useCallback(async () => {
    setPackBusy(true);
    try {
      const r = await shell.offlineFlowPack();
      setPackPath(r.path);
      if (r.ok && r.pack) {
        setPack(r.pack);
        setPackError(null);
      } else {
        setPack(null);
        setPackError(r.error ?? "流程包不可用，未显示默认文案。");
      }
    } catch (e) {
      setPack(null);
      setPackError(String(e));
    } finally {
      setPackBusy(false);
    }
  }, []);

  useEffect(() => {
    void reloadPack();
  }, [reloadPack]);

  const label = (key: Tab): string => pack?.[key]?.tab_label ?? FALLBACK_LABEL[key];
  const copy = (key: Tab): OfflineFlowCopy | null => pack?.[key] ?? null;

  const resetOutput = () => {
    setCheck(null);
    setCheckError(null);
    setReceipt(null);
    setSaveError(null);
    setConfirmed(false);
    setTitle("");
    setDraft(null);
    setAiPhase("idle");
    setAiTaskId(null);
    setAiStatus(null);
    setAiMessage(null);
    setAiCancelRequested(false);
    // 材料随场景一起清空，避免把上一场景选的文件带进下一次提交。
    setMaterials([]);
    setMaterialPath("");
    setMaterialError(null);
  };

  const loadSample = (id: string) => {
    const sample = samples.find((s) => s.sample_id === id);
    if (!sample) return;
    if (id.startsWith("weekly")) setWeeklyInput(sample.content);
    if (id.startsWith("project")) setProjectInput(sample.content);
    if (id.startsWith("writing")) {
      for (const line of sample.content.split("\n")) {
        const trimmed = line.trim();
        if (trimmed.startsWith("目的：")) setPurpose(trimmed.slice(3).trim());
        else if (trimmed.startsWith("受众：")) setAudience(trimmed.slice(3).trim());
        else if (trimmed.startsWith("明确要求：")) setRequirements(trimmed.slice(5).trim());
      }
      const factLines = sample.content
        .split("\n")
        .filter((line) => line.trim().startsWith("- "))
        .map((line) => line.trim());
      setFacts(factLines.join("\n"));
    }
    resetOutput();
  };

  /** 确定性核对：代码计算，单独成栏；不作为成果，也不进入保存。 */
  const runCheck = useCallback(async () => {
    setCheck(null);
    setCheckError(null);
    setCheckBusy(true);
    try {
      if (tab === "writing") {
        // 写作场景没有确定性“成稿”；这里只做输入完整性检查（不生成任何模板文本）。
        const missing: string[] = [];
        if (!purpose.trim()) missing.push("材料目的");
        if (!audience.trim()) missing.push("受众");
        if (!facts.trim()) missing.push("可用事实与数据");
        setCheck(
          missing.length === 0
            ? "输入完整性：目的、受众、事实均已填写。"
            : "输入完整性：仍缺少 " + missing.join("、") + "。模型可能要求补充。",
        );
        return;
      }
      if (!pack) {
        setCheckError(packError ?? "流程包不可用：请先修正或重载流程包，再运行核对。");
        return;
      }
      const response =
        tab === "weekly"
          ? await shell.weeklySummary(weeklyInput)
          : await shell.projectSummary(projectInput, today.trim() || null);
      if (response.ok && response.markdown) {
        setCheck(response.markdown);
      } else {
        setCheckError(response.error ?? "核对失败，未生成结果。");
      }
    } catch (e) {
      setCheckError(String(e));
    } finally {
      setCheckBusy(false);
    }
  }, [tab, pack, packError, weeklyInput, projectInput, today, purpose, audience, facts]);

  const readMaterialFile = useCallback(async () => {
    const p = materialPath.trim();
    if (!p) return;
    if (materials.length >= MAX_MATERIALS) {
      setMaterialError(`最多加入 ${MAX_MATERIALS} 个材料文件，请先移除一个。`);
      return;
    }
    setMaterialBusy(true);
    setMaterialError(null);
    try {
      const m = await shell.readMaterial(p);
      setMaterials((prev) => (prev.some((x) => x.path === m.path) ? prev : [...prev, m]));
      setMaterialPath("");
    } catch (e) {
      setMaterialError(String(e));
    } finally {
      setMaterialBusy(false);
    }
  }, [materialPath, materials.length]);

  const removeMaterial = (path: string) => {
    setMaterials((prev) => prev.filter((m) => m.path !== path));
  };

  const materialForAi = (): string => {
    if (tab === "weekly") return weeklyInput;
    if (tab === "project") {
      return projectInput + (today.trim() ? `\n（逾期判定日期：${today.trim()}）` : "");
    }
    return [
      facts.trim() ? "可用事实：\n" + facts.trim() : "",
      requirements.trim() ? "明确要求：\n" + requirements.trim() : "",
    ]
      .filter(Boolean)
      .join("\n\n");
  };

  const aiMaterials = (): AiMaterial[] =>
    materials.map((m) => ({ name: m.file_name, sha256: m.sha256, content: m.content }));

  const runAi = useCallback(async () => {
    setAiCancelRequested(false);
    const isWriting = tab === "writing";
    // 引擎需要绝对工作目录；宿主未就绪时不要假装可用，提交会给出真实原因。
    let dir = workspaceDir;
    if (!dir) {
      try {
        const prep = await shell.prepareWorkspaceDir();
        if (prep.ok && prep.workspace_dir) {
          dir = prep.workspace_dir;
          setWorkspaceDir(dir);
        }
      } catch {
        /* 交由提交步骤给出真实拒绝原因 */
      }
    }
    await runAiDraftSubmission(
      host,
      {
        sceneLabel: label(tab),
        purpose: isWriting ? purpose : aiPurpose,
        audience: isWriting ? audience : aiAudience,
        requirements: isWriting ? requirements : aiRequirements,
        material: materialForAi(),
        materials: aiMaterials(),
        sceneRules: copy(tab)?.draft?.rules ?? null,
        sceneSections: copy(tab)?.draft?.sections ?? null,
      },
      SCENARIO[tab],
      {
        onPhase: setAiPhase,
        onTaskId: setAiTaskId,
        onStatus: setAiStatus,
        onDraft: setDraft,
        onMessage: setAiMessage,
      },
      { workspaceDir: dir },
    );
    // 提交被拒绝往往意味着权限/准入变化：重新读一遍宿主事实。
    host.admissionStatus().then(
      (s) => {
        if (s.ok && s.facts) {
          setPermissionNote(
            s.facts.allowed
              ? `模型权限已开启（${s.facts.category}）。宿主在提交时强制执行。`
              : `模型权限未开启（${s.facts.category_reason || "端点未核准"}）；请在「成果目录与模型权限」中开启后重试。`,
          );
        }
      },
      () => {},
    );
  }, [
    tab,
    purpose,
    audience,
    requirements,
    aiPurpose,
    aiAudience,
    aiRequirements,
    weeklyInput,
    projectInput,
    today,
    facts,
    materials,
    pack,
    workspaceDir,
  ]);

  const cancelAi = useCallback(async () => {
    const sent = await requestAiDraftCancel(
      host,
      { taskId: aiTaskId, phase: aiPhase, status: aiStatus },
      {
        onRequestSent: () => setAiCancelRequested(true),
        onMessage: (m) => setAiMessage(m),
        onStatus: (s) => setAiStatus(s),
      },
      aiCancelRequested,
    );
    if (sent === "skipped") setAiMessage("当前没有可取消的模型任务（或取消请求已发出）。");
  }, [aiTaskId, aiPhase, aiStatus, aiCancelRequested]);

  /** 材料来源记录：由 OmniDesk 追加，明确标注非模型输出。 */
  const sourceHeader = (): string => {
    if (materials.length === 0) return "";
    const lines = ["", "> 材料来源（由 OmniDesk 记录，非模型输出）："];
    for (const m of materials) {
      lines.push(`> - ${m.file_name}（sha256:${m.sha256.slice(0, 12)}，${m.bytes} 字节）`);
    }
    return lines.join("\n") + "\n";
  };

  const save = useCallback(async () => {
    if (draft === null) {
      setSaveError("尚未取得模型初稿，无法保存成果。请先生成初稿。");
      return;
    }
    setSaveError(null);
    setBusy(true);
    try {
      const r = await shell.saveOffline({
        scenario: SCENARIO[tab],
        title: title.trim() || label(tab),
        markdown: sourceHeader() + draft,
        userConfirmed: confirmed,
        // 成果来自模型初稿，来源记为“模型生成、未复核”。
        aiAssisted: true,
      });
      setReceipt(r);
      refreshArtifacts();
    } catch (e) {
      setSaveError(String(e));
    } finally {
      setBusy(false);
    }
  }, [draft, title, confirmed, tab, materials, refreshArtifacts]);

  const reopen = useCallback(async (fileName: string) => {
    setArtifactError(null);
    try {
      const item = await shell.readOfflineArtifact(fileName);
      setOpened(item.content);
    } catch (e) {
      setArtifactError(String(e));
    }
  }, []);

  return (
    <div className="workbench ofl-v2">
      <section className="pane wb-header">
        <h2 className="pane-title">
          工作场景（合成数据）
          <span className="count">模型起草 · 代码核对</span>
        </h2>
        <div className="scope-notice">
          <div>
            <strong>模型是内容处理步骤。</strong>
            你填写的内容与明确选择的材料文件会经现有引擎适配链路交给模型整理、提炼并起草；
            结果标注“模型生成、未复核”，需人工核对后再保存。
          </div>
          <ul>
            <li>
              <strong>输入框不是授权入口。</strong>
              只能输入合成、公开或已明确获准的内容；真实工作材料是否可用，取决于单独取得的书面授权。
            </li>
            <li>
              <strong>代码只做确定性工作。</strong>
              日期校验、状态计数与逾期判定由代码计算，单独成栏；它只是核对，不是成果，也不会被保存成成稿。
            </li>
            <li>
              <strong>新成果保存位置。</strong>
              保存只写入仓库外的 OmniDesk 数据目录，按时间戳另存为新文件，不覆盖任何已有文件。
            </li>
          </ul>
        </div>
        <div className="type-row">
          {(Object.keys(FALLBACK_LABEL) as Tab[]).map((key) => (
            <button
              key={key}
              className={tab === key ? "tab active" : "tab"}
              onClick={() => {
                setTab(key);
                resetOutput();
              }}
            >
              {label(key)}
            </button>
          ))}
        </div>
        <div className="scope-path">模型权限：{permissionNote}</div>
        <div className="scope-path">
          {pack ? `流程包：${packPath ?? "已加载"}` : `流程包未生效：${packError ?? "未加载"}`}
          <button className="ghost tiny" onClick={() => void reloadPack()} disabled={packBusy}>
            {packBusy ? "重载中…" : "重载流程包"}
          </button>
        </div>
        {packError ? <div className="alert">{packError}</div> : null}
        <div className="scope-path">
          {artifactRoot
            ? `新成果保存到：${artifactRoot}（每次保存按时间戳新建文件，不覆盖已有文件）`
            : `新成果保存目录当前不可用：${rootError ?? "位置未解析"}`}
        </div>
      </section>

      {/* 输入 */}
      <section className="pane">
        {tab === "weekly" ? (
          <div className="field">
            <label className="field-label">
              {copy("weekly")?.input_label ??
                "每行格式：日期 | 事项 | 状态（done / pending / blocked）"}
            </label>
            <textarea
              className="prompt"
              rows={8}
              value={weeklyInput}
              onChange={(e) => setWeeklyInput(e.target.value)}
              placeholder={copy("weekly")?.input_placeholder ?? "2026-10-12 | 合成事项 | done"}
            />
            <div className="actions">
              <button className="ghost" onClick={() => loadSample("weekly-basic")}>
                {copy("weekly")?.sample_label ?? "载入合成样例"}
              </button>
            </div>
          </div>
        ) : null}

        {tab === "project" ? (
          <div className="field">
            <label className="field-label">
              {copy("project")?.input_label ??
                "每行格式：事项 | 负责人代号 | 期限 | 状态 | 下一步"}
            </label>
            <textarea
              className="prompt"
              rows={8}
              value={projectInput}
              onChange={(e) => setProjectInput(e.target.value)}
              placeholder={
                copy("project")?.input_placeholder ??
                "合成事项 | P1 | 2026-10-20 | in_progress | 补齐说明"
              }
            />
            <div className="field">
              <label className="field-label">
                {copy("project")?.today_label ?? "逾期判定日期（YYYY-MM-DD）"}
              </label>
              <input
                className="text-input"
                value={today}
                onChange={(e) => setToday(e.target.value)}
                placeholder={copy("project")?.today_placeholder ?? "2026-10-15"}
              />
            </div>
            <div className="actions">
              <button className="ghost" onClick={() => loadSample("project-basic")}>
                {copy("project")?.sample_label ?? "载入合成样例"}
              </button>
            </div>
          </div>
        ) : null}

        {tab === "writing" ? (
          <div className="record-form">
            <div className="actions">
              <button className="ghost" onClick={() => loadSample("writing-basic")}>
                {copy("writing")?.sample_label ?? "载入合成样例"}
              </button>
            </div>
            <div className="field">
              <label className="field-label">{copy("writing")?.purpose_label ?? "材料目的"}</label>
              <input className="text-input" value={purpose} onChange={(e) => setPurpose(e.target.value)} />
            </div>
            <div className="field">
              <label className="field-label">{copy("writing")?.audience_label ?? "受众"}</label>
              <input className="text-input" value={audience} onChange={(e) => setAudience(e.target.value)} />
            </div>
            <div className="field">
              <label className="field-label">
                {copy("writing")?.requirements_label ?? "明确要求"}
              </label>
              <textarea
                className="prompt"
                rows={3}
                value={requirements}
                onChange={(e) => setRequirements(e.target.value)}
              />
            </div>
            <div className="field">
              <label className="field-label">{copy("writing")?.facts_label ?? "可用事实与数据"}</label>
              <textarea className="prompt" rows={4} value={facts} onChange={(e) => setFacts(e.target.value)} />
            </div>
          </div>
        ) : null}

        {tab !== "writing" ? (
          <div className="record-form">
            <div className="field">
              <label className="field-label">材料目的（交给模型时使用）</label>
              <input className="text-input" value={aiPurpose} onChange={(e) => setAiPurpose(e.target.value)} />
            </div>
            <div className="field">
              <label className="field-label">受众</label>
              <input className="text-input" value={aiAudience} onChange={(e) => setAiAudience(e.target.value)} />
            </div>
            <div className="field">
              <label className="field-label">明确要求（篇幅、时限、必须包含的内容）</label>
              <textarea
                className="prompt"
                rows={2}
                value={aiRequirements}
                onChange={(e) => setAiRequirements(e.target.value)}
              />
            </div>
          </div>
        ) : null}
      </section>

      {/* 材料：用户明确选择的文档/表格 */}
      <section className="pane">
        <h2 className="pane-title">
          材料文件（可选）
          <span className="count">用户明确选择 · 只读单文件</span>
        </h2>
        <p className="hint">
          粘贴一个文档/表格的**绝对路径**后点击“读取”，其正文会原样进入模型输入。
          只读你指定的这一个文件：不扫描目录、不递归、不解析数据库；只接受纯文本表格（
          md / txt / csv / tsv / json / yaml / log 等），单文件不超过 512 KiB。
        </p>
        <div className="field">
          <label className="field-label">材料文件绝对路径</label>
          <input
            className="text-input mono"
            value={materialPath}
            onChange={(e) => setMaterialPath(e.target.value)}
            placeholder="/tmp/od26/合成材料.md"
          />
        </div>
        <div className="actions">
          <button className="ghost" onClick={() => void readMaterialFile()} disabled={materialBusy || !materialPath.trim()}>
            {materialBusy ? "读取中…" : "读取材料文件"}
          </button>
          <span className="hint">
            已加入 {materials.length} / {MAX_MATERIALS} 个
          </span>
        </div>
        {materialError ? <div className="alert">{materialError}</div> : null}
        {materials.length > 0 ? (
          <ul className="record-list">
            {materials.map((m) => (
              <li key={m.path} className="record-item">
                <span className="mono">{m.file_name}</span>
                <span className="hint">
                  {m.bytes} 字节 · sha256:{m.sha256.slice(0, 12)}
                </span>
                <button className="ghost" onClick={() => removeMaterial(m.path)}>
                  移除
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p className="hint">尚未加入材料文件；只用输入框内容也可以提交。</p>
        )}
      </section>

      {/* 确定性核对（代码计算，不是成果） */}
      <section className="pane">
        <h2 className="pane-title">
          确定性核对（代码计算）
          <span className="count">不是成果 · 不参与保存</span>
        </h2>
        <p className="hint">
          日期是否真实存在、状态计数与逾期判定由代码完成，供你核对模型输出；它不生成成稿，
          也不会被当成模型产出保存。
        </p>
        <div className="actions">
          <button className="ghost" onClick={() => void runCheck()} disabled={checkBusy}>
            {checkBusy ? "核对中…" : "运行确定性核对"}
          </button>
        </div>
        {checkError ? <div className="alert">{checkError}</div> : null}
        {check ? <pre className="text">{check}</pre> : null}
      </section>

      {/* 模型初稿：本场景的成果来源 */}
      <section className="pane">
        <h2 className="pane-title">
          模型初稿
          <span className="count">成果来源 · 模型生成、未复核</span>
        </h2>
        <p className="hint">
          把上面的输入与所选材料交给现有引擎适配链路起草；规则来自可编辑的流程包。
          提交、取消、失败都按宿主真实回报显示。模型权限未开启时提交会被拒绝，不会生成任何文本。
        </p>
        <div className="actions">
          <button className="primary" onClick={() => void runAi()} disabled={aiPhase === "running"}>
            {aiPhase === "running" ? "生成中…" : "生成模型初稿"}
          </button>
          <button className="ghost" onClick={() => void cancelAi()} disabled={aiPhase !== "running" || aiCancelRequested}>
            {aiCancelRequested ? "取消请求已发出" : "取消生成"}
          </button>
        </div>
        {aiPhase !== "idle" ? (
          <div className="status-card">
            <div className="status-row">
              <span className="k">阶段</span>
              <span className="v">{aiPhase}</span>
            </div>
            <div className="status-row">
              <span className="k">任务标识</span>
              <span className="v mono">{aiTaskId ?? "（尚未取得）"}</span>
            </div>
            <div className="status-row">
              <span className="k">宿主状态</span>
              <span className="v mono">{aiStatus ?? "（未取得）"}</span>
            </div>
          </div>
        ) : null}
        {aiMessage ? <div className={aiPhase === "done" ? "hint" : "alert"}>{aiMessage}</div> : null}
        {draft !== null ? (
          <div className="field">
            <label className="field-label">模型初稿（可编辑；来源：模型生成、未复核）</label>
            <textarea className="prompt" rows={14} value={draft} onChange={(e) => setDraft(e.target.value)} />
            <p className="hint">
              编辑后保存的是这份人工核对过的文本；原始输入与材料文件仍保留在上方，不会被改写或覆盖。
            </p>
          </div>
        ) : null}
      </section>

      {/* 保存：仅模型初稿 */}
      <section className="pane">
        <h2 className="pane-title">
          成果（模型初稿）
          <span className="count">人工确认后保存</span>
        </h2>
        {draft === null ? (
          <p className="hint">尚未取得模型初稿：请先在上方生成，再人工确认并保存。代码核对结果不作为成果。</p>
        ) : (
          <div className="record-form">
            <div className="field">
              <label className="field-label">成果标题（可选）</label>
              <input className="text-input" value={title} onChange={(e) => setTitle(e.target.value)} />
            </div>
            <label className="check">
              <input type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} />
              <span>
                我已人工核对以上内容，确认只含合成 / 公开 / 经明确批准的数据，并同意保存到仓库外的本地数据目录。
              </span>
            </label>
            <div className="actions">
              <button className="primary" onClick={() => void save()} disabled={!confirmed || busy}>
                保存成果
              </button>
            </div>
            {saveError ? <div className="alert">{saveError}</div> : null}
            {receipt ? (
              <div className="status-card">
                <div className="status-row">
                  <span className="k">已保存</span>
                  <span className="v mono">{receipt.path}</span>
                </div>
                <div className="status-row">
                  <span className="k">字节数</span>
                  <span className="v mono">{receipt.bytes}</span>
                </div>
              </div>
            ) : null}
          </div>
        )}
      </section>

      <section className="pane">
        <h2 className="pane-title">已保存的成果</h2>
        <p className="hint">
          只列出本页保存到仓库外数据目录的成果。重新打开是只读操作，不修改原文件。
        </p>
        <div className="actions">
          <button className="ghost" onClick={refreshArtifacts}>
            刷新列表
          </button>
        </div>
        {artifactError ? <div className="alert">{artifactError}</div> : null}
        {artifacts.length === 0 ? (
          <p className="hint">还没有保存过成果。</p>
        ) : (
          <ul className="record-list">
            {artifacts.map((a) => (
              <li key={a.file_name} className="record-item">
                <span className="mono">{a.file_name}</span>
                <span className="hint">
                  {a.bytes} 字节 · {a.scenario}
                </span>
                <button className="ghost" onClick={() => void reopen(a.file_name)}>
                  重新打开
                </button>
              </li>
            ))}
          </ul>
        )}
        {opened ? (
          <div className="field">
            <label className="field-label">已重新打开的成果（只读）</label>
            <pre className="text">{opened}</pre>
          </div>
        ) : null}
      </section>

      <UsageLog scenario={SCENARIO[tab]} />
    </div>
  );
}
