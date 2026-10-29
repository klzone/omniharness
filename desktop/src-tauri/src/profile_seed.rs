//! 出厂资产的内嵌副本（OD-ARCH-111）。
//!
//! 为什么需要这一层：materialize 原来只从仓库目录复制 Skill 与写作资产，而
//! 仓库根的推导依赖 env!("CARGO_MANIFEST_DIR") —— 那是编译期的源码路径。
//! deb / AppImage 装到用户机器上时该目录不存在，于是「干净环境 + 切换 Profile」
//! 能装出配置、场景、规则，却一个 Skill 都补不上。
//!
//! 这里把 8 个 Skill 正文与写作资产在编译期嵌入二进制（约 110K），
//! 仓库源不可用时用它照样装出完整的专属工作台。
//!
//! 一致性由 embedded_seed_matches_repo 集成测试逐字节比对兜住：两边漂移会红。

/// 一个内嵌文件：rel 是相对所属根目录的路径（统一用 / 分隔）。
pub struct SeedFile {
    pub rel: &'static str,
    pub body: &'static str,
}

/// 一个内嵌 Skill 目录。
pub struct SeedSkill {
    pub name: &'static str,
    pub files: &'static [SeedFile],
}

/// 写作资产：Skill 的 references/ 下的三个子目录。
pub struct SeedAssets {
    /// references/ 下的子目录名。
    pub sub: &'static str,
    pub files: &'static [SeedFile],
}

macro_rules! skill_md {
    ($name:literal) => {
        &[SeedFile {
            rel: "SKILL.md",
            body: include_str!(concat!("../../../skills/", $name, "/SKILL.md")),
        }]
    };
}

macro_rules! asset_file {
    ($rel:literal, $path:literal) => {
        SeedFile {
            rel: $rel,
            body: include_str!($path),
        }
    };
}

pub static SKILLS: &[SeedSkill] = &[
    SeedSkill { name: "anysearch", files: skill_md!("anysearch") },
    SeedSkill { name: "document-review", files: skill_md!("document-review") },
    SeedSkill { name: "knowledge-agent", files: skill_md!("knowledge-agent") },
    SeedSkill { name: "knowledge-search", files: skill_md!("knowledge-search") },
    SeedSkill { name: "material-writing", files: skill_md!("material-writing") },
    SeedSkill { name: "memory-agent", files: skill_md!("memory-agent") },
    SeedSkill { name: "memory-report", files: skill_md!("memory-report") },
    SeedSkill { name: "project-agent", files: skill_md!("project-agent") },
];

pub static ASSETS: &[SeedAssets] = &[
    SeedAssets {
        sub: "templates",
        files: &[
            asset_file!("report-template.md", "../../../templates/discipline/report-template.md"),
            asset_file!("speech-template.md", "../../../templates/discipline/speech-template.md"),
            asset_file!("work-summary-template.md", "../../../templates/discipline/work-summary-template.md"),
        ],
    },
    SeedAssets {
        sub: "writing-style",
        files: &[
            asset_file!("README.md", "../../../knowledge/writing-style/README.md"),
            asset_file!("avoid-list.md", "../../../knowledge/writing-style/avoid-list.md"),
            asset_file!("common-phrases.md", "../../../knowledge/writing-style/common-phrases.md"),
            asset_file!("structure-patterns.md", "../../../knowledge/writing-style/structure-patterns.md"),
            asset_file!("style-guide.md", "../../../knowledge/writing-style/style-guide.md"),
        ],
    },
    SeedAssets {
        sub: "excellent-cases",
        files: &[asset_file!("README.md", "../../../knowledge/excellent-cases/README.md")],
    },
];

/// 按名字取内嵌 Skill；没有就返回 None（自定义 Skill 不在内嵌范围）。
pub fn skill(name: &str) -> Option<&'static SeedSkill> {
    SKILLS.iter().find(|s| s.name == name.trim())
}

