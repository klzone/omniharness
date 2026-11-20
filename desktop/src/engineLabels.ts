/** 引擎状态只给人看，不把协议里的英文枚举直接抛到界面上。 */
export function engineKindLabel(kind: string): string {
  if (kind === "dsh-web-api") return "DSH Web API";
  if (kind === "dsh-acp") return "DSH ACP";
  return kind || "引擎";
}

export function engineCompatibilityLabel(value: string): string {
  if (value === "compatible") return "协议兼容";
  if (value === "degraded") return "协议降级";
  if (value === "incompatible") return "协议不兼容";
  return value ? "兼容性：" + value : "兼容性未知";
}

export function engineSummaryLabel(input: {
  engine_kind: string;
  engine_version: string;
  compatibility: string;
}): string {
  const version = input.engine_version.trim();
  const parts = [engineKindLabel(input.engine_kind)];
  // 版本未暴露时索性不显示，不用占位词冒充真实版本号。
  if (version && version !== "unknown") parts.push(version);
  parts.push(engineCompatibilityLabel(input.compatibility));
  return parts.join(" · ");
}
