#!/usr/bin/env node
/**
 * 模型路由的**列出**与**选择**（OD-ARCH-034）。
 *
 * 为什么需要它：DSH 的「模型选择」是**按 Profile 存**的——用户在 DSH 界面（web profile）里配的
 * 自定义 provider 与默认模型，引擎跑的另一个 profile（acp）**看不到**；两边只有**凭据**是共享的
 * （`$DSH_HOME/.credentials.yaml` 里的 ref）。于是引擎仍在用旧路由，任务会失败/只拿到部分结果。
 *
 * 本脚本做两件事（都只用 DSH 公开的 `--dump-config`，**不读凭证值**）：
 *   1. `--list`：列出各 Profile 生效的 provider / model / 端点（只取非机密字段）。
 *   2. `--write-overlay`：把某个 Profile 里的 provider 配置 + 选中的默认模型，写成一份
 *      **OmniDesk 自己的** patch 覆盖文件（由宿主以 `--patch` 传给引擎与端点分类器），
 *      于是引擎与界面判定用的是同一份路由。
 *
 * 安全边界：
 *   * 写出的覆盖文件里**只含非机密字段**（baseURL / api / apiKeyEnv / 模型 id）；
 *     任何 key/secret/token 字段一律剔除，绝不复制。
 *   * 只写 OmniDesk 自己数据根下的那一个文件，**不碰用户的 DSH Profile**。
 */
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import fs from "node:fs";
import path from "node:path";

const SECRET_KEYS = new Set(["key", "secret", "token", "password", "apikey", "api_key", "accessToken"]);

function fail(code, message, basis = []) {
  return { ok: false, error: { code, message }, basis };
}

function parseArgs(argv) {
  const out = { profiles: [], patches: [], dshCli: null, dshHome: null, list: false, dumpRoutes: false, builtinProviders: false, writeOverlay: null, provider: null, model: null, reasoningEffort: null };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--list") out.list = true;
    else if (arg === "--dump-routes") out.dumpRoutes = true;
    else if (arg === "--builtin-providers") out.builtinProviders = true;
    else if (arg === "--dsh-cli") out.dshCli = argv[++i];
    else if (arg === "--dsh-home") out.dshHome = argv[++i];
    else if (arg === "--profile") out.profiles.push(argv[++i]);
    else if (arg === "--patch") out.patches.push(argv[++i]);
    else if (arg === "--write-overlay") out.writeOverlay = argv[++i];
    else if (arg === "--provider") out.provider = argv[++i];
    else if (arg === "--model") out.model = argv[++i];
    else if (arg === "--reasoning-effort") out.reasoningEffort = argv[++i];
    else throw new Error("unknown argument: " + arg);
  }
  return out;
}

function loadYaml(dshCli) {
  const requireFromDsh = createRequire(dshCli);
  try {
    return requireFromDsh("yaml");
  } catch {
    const root = path.join(path.dirname(dshCli), "..", "..", "..", "..", "package.json");
    return createRequire(root)("yaml");
  }
}

function dumpConfig({ dshCli, dshHome, profile, patches }) {
  const childEnv = {
    PATH: process.env.PATH ?? "",
    HOME: process.env.HOME ?? "",
    DSH_HOME: dshHome,
  };
  if (process.env.DSH_AGENTS_HOME) childEnv.DSH_AGENTS_HOME = process.env.DSH_AGENTS_HOME;
  // 补丁必须一起带上：引擎实际加载的是「Profile + 补丁」，漏掉补丁会让只存在于
  // 补丁里的路由在合并时消失（OD-ARCH-065 实测踩到过 fake 路由被抹掉）。
  const argv = [dshCli, "--profile", profile];
  for (const p of patches ?? []) argv.push("--patch", p);
  argv.push("--dump-config");
  const result = spawnSync(process.execPath, argv, {
    env: childEnv,
    encoding: "utf8",
    timeout: 30000,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error) throw new Error("无法运行 DSH 配置转储：" + result.error.message);
  if (result.status !== 0) throw new Error("DSH 配置转储失败（退出码 " + result.status + "）");
  return result.stdout;
}

/**
 * pi-ai 自带目录里的 provider 名（OD-ARCH-065 S1）。
 *
 * llm/discoverModels 只要请求里带 provider 且命中这份目录，就直接返回内置表、
 * 不发网络请求。界面必须据此标注「未发起真实网络请求」，不能当作连通性证明。
 * 只能按 dshCli 推导绝对路径动态 import：包 exports 不开放该子路径。
 */
async function builtinProviders(dshCli) {
  const target = path.join(
    path.dirname(dshCli),
    "..",
    "..",
    "..",
    "@earendil-works",
    "pi-ai",
    "dist",
    "providers",
    "all.js",
  );
  if (!fs.existsSync(target)) return [];
  const mod = await import(pathToFileURL(target).href);
  if (typeof mod.getBuiltinProviders !== "function") return [];
  return mod.getBuiltinProviders();
}

/** 在配置层里找 id 命中的那一条，取它的 config。 */
function findEntry(entries, id) {
  for (const entry of entries) {
    if (entry && entry.id === id) return entry;
  }
  return null;
}

function stripSecrets(value) {
  if (Array.isArray(value)) return value.map(stripSecrets);
  if (value && typeof value === "object") {
    const out = {};
    for (const [k, v] of Object.entries(value)) {
      if (SECRET_KEYS.has(k)) continue;
      out[k] = stripSecrets(v);
    }
    return out;
  }
  return value;
}

/** 汇总一个 Profile 生效的 provider 与默认模型。 */
function summarize(entries) {
  const llm = findEntry(entries, "llm-pi-ai");
  const providers = [];
  const providersCfg = (llm && llm.config && llm.config.providers) || {};
  for (const [id, cfg] of Object.entries(providersCfg)) {
    const clean = stripSecrets(cfg || {});
    // 每个模型的完整非机密字段（名字、上下文窗口、输出上限、模态、思考等级、兼容项）。
    // 合并写回时原样带上，避免补丁把 Profile 里声明的能力字段抹掉。
    const modelEntries = (clean.models ?? [])
      .map((m) => (typeof m === "string" ? { id: m } : m && typeof m === "object" ? m : null))
      .filter((m) => m && m.id);
    providers.push({
      id,
      displayName: clean.displayName ?? id,
      api: clean.api ?? null,
      apiKeyEnv: clean.apiKeyEnv ?? null,
      baseURL: clean.baseURL ?? null,
      models: modelEntries.map((m) => m.id),
      modelEntries,
    });
  }
  const def = findEntry(entries, "agent-default-model");
  const defaultModel = def && def.config ? { provider: def.config.provider ?? null, model: def.config.model ?? null } : null;
  return { providers, default: defaultModel };
}

async function main() {
  let argv = process.argv.slice(2);
  if (argv.length === 0 && process.env.OMNIDESK_ROUTE_ARGS) {
    try {
      const parsed = JSON.parse(process.env.OMNIDESK_ROUTE_ARGS);
      argv = [];
      if (parsed.dshCli) argv.push("--dsh-cli", String(parsed.dshCli));
      if (parsed.dshHome) argv.push("--dsh-home", String(parsed.dshHome));
      for (const p of Array.isArray(parsed.profiles) ? parsed.profiles : []) argv.push("--profile", String(p));
      for (const p of Array.isArray(parsed.patches) ? parsed.patches : []) argv.push("--patch", String(p));
      if (parsed.writeOverlay) argv.push("--write-overlay", String(parsed.writeOverlay));
      if (parsed.dumpRoutes) argv.push("--dump-routes");
      if (parsed.builtinProviders) argv.push("--builtin-providers");
      if (parsed.provider) argv.push("--provider", String(parsed.provider));
      if (parsed.model) argv.push("--model", String(parsed.model));
      if (parsed.reasoningEffort) argv.push("--reasoning-effort", String(parsed.reasoningEffort));
    } catch {
      process.stdout.write(JSON.stringify(fail("bad_arguments", "OMNIDESK_ROUTE_ARGS 不是合法 JSON")) + "\n");
      process.exit(2);
    }
  }
  const args = parseArgs(argv);
  if (!args.dshCli || !args.dshHome) {
    process.stdout.write(JSON.stringify(fail("bad_arguments", "缺少 --dsh-cli 或 --dsh-home")) + "\n");
    process.exit(2);
  }
  try {
    const YAML = loadYaml(args.dshCli);
    if (args.builtinProviders) {
      const providers = await builtinProviders(args.dshCli);
      process.stdout.write(JSON.stringify({ ok: true, providers }) + "\n");
      process.exit(0);
    }
    if (args.dumpRoutes) {
      // 只读：把引擎 Profile 当前生效的路由以非机密 JSON 交回宿主，
      // 供多提供商合并时保留 Profile 自带的路由（补丁层是浅合并）。
      if (args.profiles.length === 0) {
        process.stdout.write(JSON.stringify(fail("bad_arguments", "转储路由需要 --profile")) + "\n");
        process.exit(2);
      }
      const entries = YAML.parse(dumpConfig({ dshCli: args.dshCli, dshHome: args.dshHome, profile: args.profiles[0], patches: args.patches }), {
        logLevel: "silent",
      });
      const summary = summarize(Array.isArray(entries) ? entries : []);
      process.stdout.write(
        JSON.stringify({ ok: true, profile: args.profiles[0], providers: summary.providers, default: summary.default }) + "\n",
      );
      process.exit(0);
    }
    if (args.writeOverlay) {
      if (!args.provider || !args.model) {
        process.stdout.write(JSON.stringify(fail("bad_arguments", "写覆盖文件需要 --provider 与 --model")) + "\n");
        process.exit(2);
      }
      const entries = YAML.parse(dumpConfig({ dshCli: args.dshCli, dshHome: args.dshHome, profile: args.profiles[0], patches: args.patches }), {
        logLevel: "silent",
      });
      const summary = summarize(Array.isArray(entries) ? entries : []);
      const provider = summary.providers.find((p) => p.id === args.provider);
      if (!provider) {
        process.stdout.write(
          JSON.stringify(fail("provider_not_found", "该 Profile 里没有这个 provider：" + args.provider)) + "\n",
        );
        process.exit(3);
      }
      // 补丁层对 config 是浅合并：只写一条路由会把 Profile 自带的其它路由抹掉。
      // 因此这里把该 Profile 的全部可渲染路由一起写回，选中项保持不变。
      const providers = {};
      for (const route of summary.providers) {
        if (!route.api || !route.baseURL) continue;
        const block = {
          displayName: route.displayName,
          api: route.api,
          baseURL: route.baseURL,
          models: route.modelEntries && route.modelEntries.length
            ? route.modelEntries
            : route.models.map((id) => ({ id, name: id })),
        };
        if (route.apiKeyEnv) block.apiKeyEnv = route.apiKeyEnv;
        providers[route.id] = block;
      }
      if (!providers[provider.id]) {
        const block = {
          displayName: provider.displayName,
          api: provider.api,
          baseURL: provider.baseURL,
          models: provider.modelEntries && provider.modelEntries.length
            ? provider.modelEntries
            : provider.models.map((id) => ({ id, name: id })),
        };
        if (provider.apiKeyEnv) block.apiKeyEnv = provider.apiKeyEnv;
        providers[provider.id] = block;
      }
      const acpConfig = { provider: provider.id, model: args.model };
      if (args.reasoningEffort && args.reasoningEffort !== "default") {
        // 思考等级（对应 DSH 的 reasoningEffort）：不传时由模型/Provider 默认决定。
        acpConfig.reasoningEffort = args.reasoningEffort;
      }
      const overlay = [
        {
          id: "llm-pi-ai",
          name: "@deepseek-ai/dsh-llm-pi-ai",
          config: { providers },
        },
        {
          // ACP 会话实际使用的路由在 `acp` 这条配置里（provider/model），
          // `agent-default-model` 只服务没有显式选择的 Agent——两处都写，避免歧义。
          id: "acp",
          name: "@deepseek-ai/dsh-acp",
          config: acpConfig,
        },
        {
          id: "agent-default-model",
          name: "@deepseek-ai/dsh-agent-default-model",
          config: { provider: provider.id, model: args.model },
        },
      ];
      fs.mkdirSync(path.dirname(args.writeOverlay), { recursive: true });
      const text = YAML.stringify(overlay);
      fs.writeFileSync(args.writeOverlay, text, { mode: 0o600 });
      process.stdout.write(
        JSON.stringify({
          ok: true,
          overlay: args.writeOverlay,
          provider: provider.id,
          model: args.model,
          baseURL: provider.baseURL,
          apiKeyEnv: provider.apiKeyEnv,
          note: provider.apiKeyEnv
            ? "覆盖文件已写入（只含非机密字段）。key 仍由共享凭据服务按 " + provider.apiKeyEnv + " 提供。"
            : "覆盖文件已写入，但该 provider 没有 apiKeyEnv——key 需要你在 DSH 凭据服务里另行登记。",
        }) + "\n",
      );
      process.exit(0);
    }
    // --list
    const profiles = [];
    for (const profile of args.profiles) {
      try {
        const entries = YAML.parse(dumpConfig({ dshCli: args.dshCli, dshHome: args.dshHome, profile, patches: args.patches }), {
          logLevel: "silent",
        });
        profiles.push({ profile, ...summarize(Array.isArray(entries) ? entries : []) });
      } catch (error) {
        profiles.push({ profile, error: String(error && error.message ? error.message : error), providers: [], default: null });
      }
    }
    process.stdout.write(JSON.stringify({ ok: true, profiles }) + "\n");
    process.exit(0);
  } catch (error) {
    process.stdout.write(
      JSON.stringify(fail("classifier_error", String(error && error.message ? error.message : error))) + "\n",
    );
    process.exit(2);
  }
}

main();
