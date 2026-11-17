// 配套插件推荐（OD-ARCH-177）：只读元数据清单。
//
// 插件会升级或过期，因此仓库不打包任何插件实体、不做联网版本检查、
// 不自动安装；设置页按这份静态清单展示名称、地址与建议版本，
// 由用户按上游自行安装与升级。

export interface CompanionPlugin {
  /** 插件名（与上游一致）。 */
  name: string;
  /** 上游地址（仓库或安装页）。 */
  url: string;
  /** 建议版本（与当前发布包锁定口径一致）。 */
  version: string;
  /** 用途一句话。 */
  purpose: string;
}

export const COMPANION_PLUGINS: readonly CompanionPlugin[] = [
  {
    name: "AnySearch CLI",
    url: "https://github.com/anysearch-ai/anysearch-skill",
    version: "v2.1.0",
    purpose: "联网检索与网页提取（规范、公开资料与新闻查询）",
  },
  {
    name: "dsh-cost-meter",
    url: "https://github.com/Han-1413141/dsh-cost-meter",
    version: "1.9.6",
    purpose: "模型调用成本计量（DSH Profile 插件）",
  },
];
