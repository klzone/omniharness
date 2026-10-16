// 通过绝对路径动态导入 DSH 真实的 Skill 提供者，避免裸模块解析依赖 cwd。
const entry = process.env.OMNI_SKILL_FS_ENTRY;
if (!entry) { console.error('OMNI_SKILL_FS_ENTRY not set'); process.exit(2); }
const mod = await import(entry);
const FileSystemSkillProvider = mod.FileSystemSkillProvider;
const cfg = JSON.parse(process.argv[2]);
const logger = { warn: (...a)=>console.error('[warn]', ...a), info(){}, debug(){}, error(...a){console.error('[err]',...a)} };
const ctx = { logger, get(){ return undefined; }, on(){}, effect(){ return undefined; } };
const control = { invalidate(){}, signal: new AbortController().signal };
const provider = new FileSystemSkillProvider(ctx, control, {
  dshHome: cfg.dshHome, agentsHome: cfg.agentsHome,
  customSkillDirs: cfg.customSkillDirs || [], includeDefaultRoots: cfg.includeDefaultRoots !== false,
  watch: false, bundledSkillDir: undefined,
});
const list = await provider.list({ cwd: cfg.cwd });
const arr = Array.isArray(list) ? list : (list.candidates || []);
const out = [];
for (const c of arr) {
  const full = await provider.get(c, {});
  out.push({ name: c.name, source: c.source, rank: c.rank, path: c.locator && c.locator.path, invocation: c.invocation, hasBody: !!(full && full.content), bodyLen: full && full.content ? full.content.length : 0, bodyHasMarker: cfg.marker ? !!(full && full.content && full.content.includes(cfg.marker)) : undefined });
}
console.log(JSON.stringify({ cwd: cfg.cwd, count: out.length, skills: out }, null, 2));
await provider.dispose();
process.exit(0);
