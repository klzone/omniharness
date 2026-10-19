#!/usr/bin/env node
/**
 * 只读模型路由分类器（OD-ARCH-021）。
 *
 * 用 DSH 公开的 `--dump-config` 读取合成后的 Profile（含 --patch），
 * 判断实际任务路由的 provider/model/endpoint，并把端点归为：
 *   local    本机回环（可就地判定内容不出本机）
 *   private  非回环的私有 / 链路本地 / .local 地址
 *            —— 仅为地址范围事实；**不代表**所属组织，也不能确认实际转发路径
 *   public   非回环的公网地址
 *            —— 同为地址范围事实；是否离开所在组织无法从地址判断
 *   unknown  缺端点、非 HTTP(S)、带凭证/查询/片段、provider 无法识别、配置冲突
 *
 * 类别只回答“是不是本机回环”，**不作**任何单位/组织归属承诺：地址特征无法证明
 * 组织归属，也无法证明数据是否离开所在组织。未知一律由宿主拒绝。
 * 只输出 provider/model、端点、类别和判定依据；不读取、不输出任何凭证。
 *
 * 用法：
 *   node dump-model-route.mjs --dsh-cli <bin.js> --dsh-home <dir> [--profile acp] [--patch <file>]...
 *   node dump-model-route.mjs --self-test
 */

import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import path from "node:path";
import net from "node:net";

const DEEPSEEK_DEFAULT_ENDPOINT = "https://api.deepseek.com/anthropic";

function fail(code, message, basis = []) {
  return { ok: false, error: { code, message }, basis };
}

function parseArgs(argv) {
  const out = { profile: "acp", patches: [], selfTest: false };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--self-test") out.selfTest = true;
    else if (arg === "--dsh-cli") out.dshCli = argv[++i];
    else if (arg === "--dsh-home") out.dshHome = argv[++i];
    else if (arg === "--profile") out.profile = argv[++i];
    else if (arg === "--patch") out.patches.push(argv[++i]);
    else throw new Error("unknown argument: " + arg);
  }
  return out;
}

function ipv4ToInt(ip) {
  const parts = ip.split(".").map((p) => Number(p));
  if (parts.length !== 4 || parts.some((p) => !Number.isInteger(p) || p < 0 || p > 255)) {
    return null;
  }
  return ((parts[0] << 24) + (parts[1] << 16) + (parts[2] << 8) + parts[3]) >>> 0;
}

function inCidr(ip, base, bits) {
  const value = ipv4ToInt(ip);
  const network = ipv4ToInt(base);
  if (value === null || network === null) return false;
  const mask = bits === 0 ? 0 : (0xffffffff << (32 - bits)) >>> 0;
  return (value & mask) === (network & mask);
}

function stripBrackets(host) {
  return host.startsWith("[") && host.endsWith("]") ? host.slice(1, -1) : host;
}

function classifyUrl(raw) {
  if (typeof raw !== "string" || raw.trim() === "") {
    return { category: "unknown", reason: "缺少端点地址" };
  }
  let url;
  try {
    url = new URL(raw.trim());
  } catch {
    return { category: "unknown", reason: "端点不是合法 URL" };
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    return { category: "unknown", reason: "端点不是 HTTP(S)" };
  }
  if (url.username || url.password) {
    return { category: "unknown", reason: "端点 URL 含凭证信息" };
  }
  if (url.search || url.hash) {
    return { category: "unknown", reason: "端点 URL 含查询参数或片段" };
  }
  const host = stripBrackets(url.hostname.toLowerCase());
  if (host === "localhost" || host.endsWith(".localhost")) {
    return { category: "local", reason: "回环主机名 localhost（仅本机）" };
  }
  if (host === "::1") return { category: "local", reason: "IPv6 回环地址 ::1（仅本机）" };
  if (net.isIPv4(host) && inCidr(host, "127.0.0.0", 8)) {
    return { category: "local", reason: "IPv4 回环地址 127.0.0.0/8（仅本机）" };
  }
  // 非回环地址：地址范围是事实，但**不能**据此推断所属组织，也不能确认实际
  // 转发路径。类别只表达“不是本机回环”，说明文字一律不承诺单位/组织归属。
  if (net.isIPv4(host)) {
    for (const [base, bits] of [
      ["10.0.0.0", 8],
      ["172.16.0.0", 12],
      ["192.168.0.0", 16],
      ["169.254.0.0", 16],
    ]) {
      if (inCidr(host, base, bits)) {
        return {
          category: "private",
          reason:
            "非回环：地址落在 " + base + "/" + bits +
            " 私有/链路本地网段（仅为地址范围事实，不代表所属组织或实际转发路径）",
        };
      }
    }
    if (inCidr(host, "0.0.0.0", 32) || inCidr(host, "100.64.0.0", 10)) {
      return { category: "unknown", reason: "无法确认该地址归属" };
    }
    return {
      category: "public",
      reason: "非回环：公网 IPv4 地址（是否离开所在组织无法从地址判断）",
    };
  }
  if (net.isIPv6(host)) {
    if (host.startsWith("fe8") || host.startsWith("fe9") || host.startsWith("fea") || host.startsWith("feb")) {
      return {
        category: "private",
        reason: "非回环：IPv6 链路本地地址（仅为地址范围事实，不代表所属组织）",
      };
    }
    if (host.startsWith("fc") || host.startsWith("fd")) {
      return {
        category: "private",
        reason: "非回环：IPv6 唯一本地地址（仅为地址范围事实，不代表所属组织）",
      };
    }
    return { category: "unknown", reason: "无法确认该 IPv6 地址归属" };
  }
  if (host.endsWith(".local")) {
    return {
      category: "private",
      reason: "非回环：mDNS .local 主机名（不代表所属组织；实际转发路径无法确认）",
    };
  }
  return {
    category: "public",
    reason: "非回环：公网域名（是否离开所在组织无法从地址判断）",
  };
}

function entryConfig(entries, id) {
  const hits = entries.filter((e) => e && e.id === id);
  if (hits.length === 0) return { found: false };
  if (hits.length > 1) return { found: true, conflict: "重复的 " + id + " 配置" };
  return { found: true, config: hits[0].config ?? {}, entry: hits[0] };
}

function resolveRoute(entries, env = {}) {
  const acp = entryConfig(entries, "acp");
  if (!acp.found) return fail("route_missing", "配置中没有 ACP 模型路由");
  if (acp.conflict) return fail("route_conflict", acp.conflict);
  const provider = acp.config.provider;
  const model = acp.config.model;
  if (typeof provider !== "string" || provider.trim() === "") {
    return fail("route_missing", "ACP 路由缺少 provider");
  }
  if (typeof model !== "string" || model.trim() === "") {
    return fail("route_missing", "ACP 路由缺少 model");
  }

  const basis = ["provider=" + provider, "model=" + model];
  // `acp.provider/model` 才是 ACP 会话实际使用的路由（见 dsh 源码
  // packages/acp/acp/src/index.ts：agentOptions / initialSelection）。
  // `agent-default-model` 只服务没有显式选择的 Agent，两者不同不构成冲突；
  // 这里仅把它记录为依据，便于人工核对。
  const defaultRoute = entryConfig(entries, "agent-default-model");
  if (defaultRoute.conflict) {
    basis.push("agent-default-model 配置重复，已忽略");
  } else if (defaultRoute.found) {
    const otherProvider = defaultRoute.config.provider;
    const otherModel = defaultRoute.config.model;
    if (typeof otherProvider === "string" && otherProvider.trim() !== "") {
      basis.push("agent-default-model=" + otherProvider + "/" + String(otherModel ?? "?"));
    }
  }
  let endpoint;
  if (provider === "deepseek-official") {
    if (typeof acp.config.baseURL === "string" && acp.config.baseURL.trim() !== "") {
      endpoint = acp.config.baseURL;
      basis.push("端点为 Profile 配置的 baseURL");
    } else if (typeof env.DEEPSEEK_BASE_URL === "string" && env.DEEPSEEK_BASE_URL.trim() !== "") {
      endpoint = env.DEEPSEEK_BASE_URL;
      basis.push("端点为 DEEPSEEK_BASE_URL 环境变量");
    } else {
      endpoint = DEEPSEEK_DEFAULT_ENDPOINT;
      basis.push("未配置 baseURL，使用 DSH DeepSeek 默认端点");
    }
  } else if (provider === "deepseek-account") {
    return fail(
      "route_unresolved",
      "deepseek-account 使用平台账户通道，无法从非秘密配置确定实际端点",
      basis,
    );
  } else {
    const pi = entryConfig(entries, "llm-pi-ai");
    if (!pi.found || pi.conflict) {
      return fail("route_unresolved", "自定义 provider 缺少可解析的 llm-pi-ai 配置", basis);
    }
    const providers = pi.config.providers;
    const route = providers && typeof providers === "object" ? providers[provider] : undefined;
    if (!route || typeof route !== "object") {
      return fail("route_unresolved", "provider 不在可识别的自定义路由清单中", basis);
    }
    if (typeof route.baseURL !== "string" || route.baseURL.trim() === "") {
      return fail("route_unresolved", "自定义 provider 未声明 baseURL，内置目录端点无法核实", basis);
    }
    endpoint = route.baseURL;
    basis.push("端点为 llm-pi-ai.providers." + provider + ".baseURL");
  }

  const classification = classifyUrl(endpoint);
  let host = null;
  try {
    host = new URL(endpoint).host;
  } catch {
    host = null;
  }
  return {
    ok: true,
    route: {
      provider,
      model,
      endpoint: typeof endpoint === "string" ? endpoint.trim() : null,
      endpoint_host: host,
      category: classification.category,
      category_reason: classification.reason,
      basis,
      leaves_device: classification.category === "public" ? true : classification.category === "local" ? false : null,
    },
  };
}

function dumpConfig({ dshCli, dshHome, profile, patches, env }) {
  const args = [dshCli, "--profile", profile];
  for (const patch of patches) args.push("--patch", patch);
  args.push("--dump-config");
  const childEnv = {
    PATH: env.PATH ?? process.env.PATH ?? "",
    HOME: env.HOME ?? process.env.HOME ?? "",
    DSH_HOME: dshHome,
  };
  if (env.DEEPSEEK_BASE_URL) childEnv.DEEPSEEK_BASE_URL = env.DEEPSEEK_BASE_URL;
  if (env.DSH_AGENTS_HOME) childEnv.DSH_AGENTS_HOME = env.DSH_AGENTS_HOME;
  const result = spawnSync(process.execPath, args, {
    env: childEnv,
    encoding: "utf8",
    timeout: 30000,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error) {
    throw new Error("无法运行 DSH 配置转储：" + result.error.message);
  }
  if (result.status !== 0) {
    const tail = String(result.stderr ?? "")
      .split("\n")
      .filter((line) => line.trim() !== "")
      .slice(-3)
      .join(" | ");
    throw new Error("DSH 配置转储失败（退出码 " + result.status + "）：" + tail);
  }
  return result.stdout;
}

function parseDump(text, dshCli) {
  const requireFromDsh = createRequire(dshCli);
  let YAML;
  try {
    YAML = requireFromDsh("yaml");
  } catch {
    YAML = createRequire(path.join(path.dirname(dshCli), "..", "..", "..", "..", "package.json"))("yaml");
  }
  const parsed = YAML.parse(text, { logLevel: "silent" });
  if (!Array.isArray(parsed)) throw new Error("DSH 配置转储不是配置层数组");
  return parsed;
}

function selfTest() {
  const cases = [
    ["http://127.0.0.1:9/v1", "local"],
    ["http://localhost:8080/v1", "local"],
    ["http://[::1]:8080/v1", "local"],
    ["http://192.168.1.20:8000/v1", "private"],
    ["http://10.1.2.3/v1", "private"],
    ["http://172.20.3.4/v1", "private"],
    ["http://model.local/v1", "private"],
    ["https://api.example.com/v1", "public"],
    ["https://api.deepseek.com/anthropic", "public"],
    ["ftp://example.com/v1", "unknown"],
    ["http://user:pass@example.com/v1", "unknown"],
    ["https://example.com/v1?x=1", "unknown"],
    ["", "unknown"],
    ["not a url", "unknown"],
  ];
  const results = cases.map(([url, expected]) => {
    const actual = classifyUrl(url).category;
    return { url, expected, actual, pass: expected === actual };
  });
  const entries = [
    { id: "acp", config: { provider: "fake", model: "synthetic" } },
    {
      id: "llm-pi-ai",
      config: { providers: { fake: { baseURL: "http://127.0.0.1:9/v1" } } },
    },
  ];
  const routed = resolveRoute(entries, {});
  results.push({
    case: "合成回环路由",
    expected: "local",
    actual: routed.route?.category,
    pass: routed.ok === true && routed.route.category === "local",
  });
  const unknown = resolveRoute([{ id: "acp", config: { provider: "mystery", model: "m" } }], {});
  results.push({ case: "未知 provider", expected: false, actual: unknown.ok, pass: unknown.ok === false });
  const differing = resolveRoute(
    [
      { id: "acp", config: { provider: "fake", model: "a" } },
      { id: "agent-default-model", config: { provider: "other", model: "b" } },
      {
        id: "llm-pi-ai",
        config: { providers: { fake: { baseURL: "http://127.0.0.1:9/v1" } } },
      },
    ],
    {},
  );
  results.push({
    case: "默认模型与 ACP 路由不同（不冲突）",
    expected: "local",
    actual: differing.route?.category,
    pass: differing.ok === true && differing.route.category === "local",
  });
  const duplicated = resolveRoute(
    [
      { id: "acp", config: { provider: "fake", model: "a" } },
      { id: "acp", config: { provider: "fake", model: "b" } },
      {
        id: "llm-pi-ai",
        config: { providers: { fake: { baseURL: "http://127.0.0.1:9/v1" } } },
      },
    ],
    {},
  );
  results.push({
    case: "重复 ACP 路由",
    expected: false,
    actual: duplicated.ok,
    pass: duplicated.ok === false,
  });
  // OD-ARCH-022：私有地址不等于单位网络。类别只能说明地址范围，说明文字
  // 不得出现“单位内部/组织内部”之类的归属断言，并必须显式声明不代表所属组织。
  const privateCases = ["http://192.168.1.20:8000/v1", "http://10.1.2.3/v1", "http://model.local/v1"];
  const orgClaimPattern = /单位内部|组织内部|内网|单位网络/;
  const privateResults = privateCases.map((url) => {
    const c = classifyUrl(url);
    return {
      case: "私有地址不声称组织归属 " + url,
      expected: "private 且无组织归属断言",
      actual: c.category + "｜" + c.reason,
      pass:
        c.category === "private" &&
        !orgClaimPattern.test(c.reason) &&
        c.reason.includes("不代表"),
    };
  });
  results.push(...privateResults);
  const failed = results.filter((r) => !r.pass);
  process.stdout.write(JSON.stringify({ ok: failed.length === 0, total: results.length, results }, null, 2) + "\n");
  return failed.length === 0 ? 0 : 1;
}

function main() {
  // 桌面宿主以 `node --input-type=module` 从 stdin 运行本脚本（不落临时文件），
  // 此时 argv 为空，参数改由 OMNIDESK_ROUTE_ARGS 传入（只含路径与 Profile 名）。
  let argv = process.argv.slice(2);
  if (argv.length === 0 && process.env.OMNIDESK_ROUTE_ARGS) {
    let parsed;
    try {
      parsed = JSON.parse(process.env.OMNIDESK_ROUTE_ARGS);
    } catch {
      process.stdout.write(JSON.stringify(fail("bad_arguments", "OMNIDESK_ROUTE_ARGS 不是合法 JSON")) + "\n");
      process.exit(2);
    }
    argv = [];
    if (parsed.dshCli) argv.push("--dsh-cli", String(parsed.dshCli));
    if (parsed.dshHome) argv.push("--dsh-home", String(parsed.dshHome));
    if (parsed.profile) argv.push("--profile", String(parsed.profile));
    for (const patch of Array.isArray(parsed.patches) ? parsed.patches : []) {
      argv.push("--patch", String(patch));
    }
  }
  const args = parseArgs(argv);
  if (args.selfTest) process.exit(selfTest());
  if (!args.dshCli || !args.dshHome) {
    process.stdout.write(JSON.stringify(fail("bad_arguments", "缺少 --dsh-cli 或 --dsh-home")) + "\n");
    process.exit(2);
  }
  try {
    const env = {
      PATH: process.env.PATH,
      HOME: process.env.HOME,
      DEEPSEEK_BASE_URL: process.env.DEEPSEEK_BASE_URL,
      DSH_AGENTS_HOME: process.env.DSH_AGENTS_HOME,
    };
    const text = dumpConfig({ ...args, env });
    const entries = parseDump(text, args.dshCli);
    const result = resolveRoute(entries, env);
    process.stdout.write(JSON.stringify(result) + "\n");
    process.exit(result.ok ? 0 : 3);
  } catch (error) {
    process.stdout.write(
      JSON.stringify(fail("classifier_error", String(error && error.message ? error.message : error))) + "\n",
    );
    process.exit(2);
  }
}

main();
