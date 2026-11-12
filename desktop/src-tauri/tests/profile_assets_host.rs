//! Profile 资产装配的宿主级集成测试（OD-ARCH-109）。
//!
//! 只用 /tmp 下的合成隔离根，绝不触碰 ~/OmniDesk：
//!   * 基础标准零写入（连目录都不创建）；
//!   * 出厂 Profile 在空根上一次装配齐配置、场景声明、规则与 8 个 Skill；
//!   * 再跑一次没有任何改写（幂等）；
//!   * 用户改过的文件不被覆盖，状态是 drift；
//!   * 删掉的文件下次装配会补回；
//!   * `inspect` 只判定、不写盘。

use std::fs;
use std::path::{Path, PathBuf};

use omnidesk_desktop_lib::profile_assets::{self, Scope};

struct Fixture {
    base: PathBuf,
    dsh_home: PathBuf,
    app_root: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-profile-assets-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        Self {
            dsh_home: base.join("dsh-home"),
            app_root: base.join("app"),
            base,
        }
    }

    fn repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn scope<'a>(&'a self, profile: &'a str, repo: Option<&'a Path>) -> Scope<'a> {
        Scope {
            dsh_home: &self.dsh_home,
            app_root: &self.app_root,
            profile,
            repo_root: repo,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn status_of(view: &profile_assets::MaterializeReport, key: &str) -> String {
    view.items
        .iter()
        .find(|i| i.key == key)
        .map(|i| i.status.clone())
        .unwrap_or_else(|| format!("（清单里没有 {}）", key))
}

#[test]
fn standard_profile_creates_nothing_at_all() {
    let fx = Fixture::new("standard");
    let repo = Fixture::repo();
    let report = profile_assets::materialize(&fx.scope("omnidesk-base", Some(&repo)), "omnidesk-base")
        .expect("标准模式装配应为空操作");
    assert!(report.standard);
    assert!(report.items.is_empty());
    assert!(!fx.dsh_home.exists(), "标准模式不能创建 DSH_HOME");
    assert!(!fx.app_root.exists(), "标准模式不能创建应用数据根");

    // 巡检同样零写入。
    let view = profile_assets::inspect(&fx.scope("omnidesk-base", Some(&repo)), "omnidesk-base")
        .expect("标准模式巡检应成功");
    assert!(view.standard);
    assert!(!fx.dsh_home.exists());
}

#[test]
fn discipline_materializes_full_asset_set_on_empty_root() {
    let fx = Fixture::new("full");
    let repo = Fixture::repo();
    let report = profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base")
        .expect("discipline 装配应成功");
    assert!(!report.standard);
    assert_eq!(report.label, "纪检专业");

    // 配置与补丁落在 DSH_HOME。
    let profile_dir = fx.dsh_home.join("profiles/discipline");
    assert!(profile_dir.join("package.json").is_file());
    assert!(profile_dir.join("cordis.patch.yml").is_file());
    // 场景声明与规则包落在应用数据根。
    let app_profile = fx.app_root.join("profiles/discipline");
    assert!(app_profile.join("decl.yml").is_file());
    assert!(app_profile.join("rules.json").is_file());
    assert!(app_profile.join(profile_assets::ASSET_STATE_FILE).is_file());

    // 8 个 Skill 都带 SKILL.md。
    for name in [
        "anysearch",
        "document-review",
        "knowledge-agent",
        "knowledge-search",
        "material-writing",
        "memory-agent",
        "memory-report",
        "project-agent",
    ] {
        let dst = fx.dsh_home.join("skills").join(name).join("SKILL.md");
        assert!(dst.is_file(), "缺少 Skill：{}", dst.display());
        assert_eq!(status_of(&report, &format!("skill:{}", name)), "created");
    }
    // 写作类 Skill 的 references 说明文件也要物化。
    for name in ["material-writing", "document-review"] {
        let readme = fx
            .dsh_home
            .join("skills")
            .join(name)
            .join("references/README.md");
        if repo.join("templates/discipline").is_dir() {
            assert!(readme.is_file(), "缺少 references/README.md：{}", readme.display());
        }
    }

    // 插件只探测，不在装配里联网：状态要么就绪要么缺失，绝不出现安装回执。
    let plugin = status_of(&report, "plugin:@deepseek-ai/dsh-skill-office");
    assert!(
        plugin == "ready" || plugin == "missing",
        "插件状态异常：{}",
        plugin
    );
}

#[test]
fn second_run_rewrites_nothing() {
    let fx = Fixture::new("idem");
    let repo = Fixture::repo();
    profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    let rules = fx.app_root.join("profiles/discipline/rules.json");
    let before = fs::read(&rules).unwrap();
    let mtime_before = fs::metadata(&rules).unwrap().modified().unwrap();

    let second =
        profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    assert_eq!(status_of(&second, "rules.json"), "unchanged");
    assert_eq!(status_of(&second, "package.json"), "unchanged");
    assert_eq!(fs::read(&rules).unwrap(), before);
    assert_eq!(fs::metadata(&rules).unwrap().modified().unwrap(), mtime_before);
    // 没有产生任何 .bak。
    let backups: Vec<_> = fs::read_dir(rules.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".bak-"))
        .collect();
    assert!(backups.is_empty(), "幂等重跑不应产生备份：{:?}", backups);
}

#[test]
fn user_edit_is_preserved_and_reported_as_drift() {
    let fx = Fixture::new("drift");
    let repo = Fixture::repo();
    profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    let rules = fx.app_root.join("profiles/discipline/rules.json");
    fs::write(&rules, "{\"mine\": true}\n").unwrap();

    let report =
        profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    assert_eq!(status_of(&report, "rules.json"), "drift");
    assert_eq!(fs::read_to_string(&rules).unwrap(), "{\"mine\": true}\n");
    assert!(report
        .notes
        .iter()
        .any(|n| n.contains("没有覆盖") || n.contains("未覆盖")));
}

#[test]
fn deleted_skill_is_healed_on_next_run() {
    let fx = Fixture::new("heal");
    let repo = Fixture::repo();
    profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    let skill = fx.dsh_home.join("skills/memory-agent");
    fs::remove_dir_all(&skill).unwrap();

    let report =
        profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    assert!(skill.join("SKILL.md").is_file(), "被删的 Skill 应补回");
    assert_eq!(status_of(&report, "skill:memory-agent"), "created");
}

#[test]
fn inspect_reports_without_writing() {
    let fx = Fixture::new("inspect");
    let repo = Fixture::repo();
    let view = profile_assets::inspect(&fx.scope("discipline", Some(&repo)), "omnidesk-base")
        .expect("巡检应成功");
    assert!(!view.standard);
    assert!(!view.items.is_empty());
    // 巡检一个空根：全部缺失，但目录一个都不建。
    assert_eq!(status_of(&view, "package.json"), "missing");
    assert!(!fx.dsh_home.exists(), "巡检不能创建 DSH_HOME");
    assert!(!fx.app_root.exists(), "巡检不能创建应用数据根");
}

#[test]
fn custom_profile_is_not_given_factory_assets() {
    let fx = Fixture::new("custom");
    let repo = Fixture::repo();
    let report =
        profile_assets::materialize(&fx.scope("my-own", Some(&repo)), "omnidesk-base").unwrap();
    assert!(!report.standard);
    assert!(report.items.is_empty());
    assert!(report.notes.iter().any(|n| n.contains("自定义")));
    assert!(!fx.dsh_home.exists(), "自定义 Profile 不应被动写盘");
}

#[test]
fn custom_profile_inspect_lists_inherited_missing_plugins_only() {
    let fx = Fixture::new("custom-plugins");
    let repo = Fixture::repo();
    // 造一个带依赖的源 Profile：一个本地装不到、一个随运行时就在。
    let src = fx.dsh_home.join("profiles/source");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("package.json"),
        r#"{"name":"dsh-profile-source","dependencies":{"@acme/not-installed":"1.0.0"}}"#,
    )
    .unwrap();
    fs::write(src.join("cordis.patch.yml"), "[]\n").unwrap();

    let receipt = omnidesk_desktop_lib::profile_lifecycle::create(
        &fx.dsh_home,
        &fx.app_root,
        "my-custom",
        "我的自定义",
        Some("source"),
    )
    .expect("新建自定义 Profile 应成功");
    assert!(receipt.profile.plugins.contains(&"@acme/not-installed".to_string()));

    let view = profile_assets::inspect(&fx.scope("my-custom", Some(&repo)), "omnidesk-base")
        .expect("巡检应成功");
    assert!(!view.standard);
    assert_eq!(
        status_of(&view, "plugin:@acme/not-installed"),
        "missing",
        "没装的依赖要显形：{:?}",
        view.items
    );
    assert_eq!(view.items.len(), 1, "只因缺插件列出，不含出厂资产清单：{:?}", view.items);
    assert!(view.summary_note.unwrap_or_default().contains("自建 Profile"));
}

#[test]
fn custom_profile_inspect_is_read_only() {
    let fx = Fixture::new("custom-readonly");
    let repo = Fixture::repo();
    fs::create_dir_all(fx.app_root.join("profiles")).unwrap();
    fs::write(
        fx.app_root.join("profiles/index.v1.json"),
        r#"{"schema":"omnidesk.profiles.v1","profiles":[{"name":"ghost","label":"幽灵","created_at":1,"origin":"standard","plugins":["@acme/ghost"]}]}"#,
    )
    .unwrap();
    let view =
        profile_assets::inspect(&fx.scope("ghost", Some(&repo)), "omnidesk-base").expect("巡检应成功");
    assert_eq!(status_of(&view, "plugin:@acme/ghost"), "missing");
    assert!(!fx.dsh_home.exists(), "巡检不能创建 DSH_HOME");
}

#[test]
fn custom_profile_materializes_bound_skills_only() {
    let fx = Fixture::new("custom-skills");
    let repo = Fixture::repo();
    let src = fx.dsh_home.join("profiles/source");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("package.json"), r#"{"name":"dsh-profile-source"}"#).unwrap();
    fs::write(src.join("cordis.patch.yml"), "[]\n").unwrap();
    fs::create_dir_all(fx.app_root.join("profiles/source")).unwrap();
    fs::write(
        fx.app_root.join("profiles/source/rules.json"),
        r#"{"schema":"omnidesk.profile-rules.v2","profile":"source","skill_bindings":{"weekly-note":{"skills":["memory-agent"]}}}"#,
    )
    .unwrap();

    omnidesk_desktop_lib::profile_lifecycle::create(
        &fx.dsh_home,
        &fx.app_root,
        "custom",
        "自定义",
        Some("source"),
    )
    .expect("新建应成功");
    // 复制只带规则文件，不带 Skill 目录。
    assert!(!fx.dsh_home.join("skills/memory-agent").exists());

    let report = profile_assets::materialize(&fx.scope("custom", Some(&repo)), "omnidesk-base")
        .expect("装配应成功");
    assert_eq!(status_of(&report, "skill:memory-agent"), "created");
    assert!(fx.dsh_home.join("skills/memory-agent/SKILL.md").is_file());
    // 只补绑定里点名的那一个，不全量灌入。
    assert!(!fx.dsh_home.join("skills/memory-report").exists());

    // 再跑一次：已有目录不覆盖。
    let again = profile_assets::materialize(&fx.scope("custom", Some(&repo)), "omnidesk-base")
        .expect("二次装配应成功");
    assert_eq!(status_of(&again, "skill:memory-agent"), "unchanged");
}



/// OD-ARCH-111：分发到用户机器后没有仓库路径，Skill 与写作资产必须仍能装齐。
#[test]
fn embedded_seed_fills_skills_without_repo() {
    let fx = Fixture::new("embedded");
    let report =
        profile_assets::materialize(&fx.scope("discipline", None), "omnidesk-base")
            .expect("无仓库根时装配也应成功");
    assert!(!report.standard);
    assert!(
        report.repo_missing.is_none(),
        "内嵌副本齐全时不应报源缺失：{:?}",
        report.repo_missing
    );
    for name in [
        "anysearch",
        "document-review",
        "knowledge-agent",
        "knowledge-search",
        "material-writing",
        "memory-agent",
        "memory-report",
        "project-agent",
    ] {
        let dst = fx.dsh_home.join("skills").join(name).join("SKILL.md");
        assert!(dst.is_file(), "内嵌 Skill 未装出：{}", dst.display());
        assert_eq!(status_of(&report, &format!("skill:{}", name)), "created");
    }
    // 写作 references 同样来自内嵌副本。
    for (skill, rel) in [
        ("material-writing", "references/templates/report-template.md"),
        ("material-writing", "references/writing-style/style-guide.md"),
        ("document-review", "references/excellent-cases/README.md"),
    ] {
        let dst = fx.dsh_home.join("skills").join(skill).join(rel);
        assert!(dst.is_file(), "内嵌写作资产未物化：{}", dst.display());
    }
    // 再跑一次：全部 unchanged，且不覆盖。
    let again =
        profile_assets::materialize(&fx.scope("discipline", None), "omnidesk-base").unwrap();
    assert_eq!(status_of(&again, "skill:memory-agent"), "unchanged");
}


/// 内嵌副本必须与仓库权威源逐字节一致，否则「开发机」和「用户机器」会装出两套内容。
#[test]
fn embedded_seed_matches_repo_sources_byte_for_byte() {
    let repo = Fixture::repo();
    for skill in omnidesk_desktop_lib::profile_seed::SKILLS {
        let src = repo.join("skills").join(skill.name).join("SKILL.md");
        let disk = fs::read_to_string(&src)
            .unwrap_or_else(|e| panic!("读不到仓库源 {}：{}", src.display(), e));
        let embedded = skill
            .files
            .iter()
            .find(|f| f.rel == "SKILL.md")
            .map(|f| f.body)
            .expect("内嵌 Skill 必须有 SKILL.md");
        assert_eq!(
            embedded, disk,
            "内嵌副本与仓库源不一致：{}（改 Skill 后需重建二进制）",
            src.display()
        );
    }
    let roots = [
        ("templates", "templates/discipline"),
        ("writing-style", "knowledge/writing-style"),
        ("excellent-cases", "knowledge/excellent-cases"),
    ];
    for assets in omnidesk_desktop_lib::profile_seed::ASSETS {
        let (_, rel_root) = roots
            .iter()
            .find(|(sub, _)| *sub == assets.sub)
            .unwrap_or_else(|| panic!("未知的内嵌资产目录：{}", assets.sub));
        for file in assets.files {
            let src = repo.join(rel_root).join(file.rel);
            let disk = fs::read_to_string(&src)
                .unwrap_or_else(|e| panic!("读不到仓库源 {}：{}", src.display(), e));
            assert_eq!(
                file.body, disk,
                "内嵌写作资产与仓库源不一致：{}",
                src.display()
            );
        }
    }
}


/// 用户在无仓库环境下改过的内嵌 Skill，重启后仍不能被覆盖。
#[test]
fn embedded_skill_user_edit_is_not_overwritten() {
    let fx = Fixture::new("embedded-drift");
    profile_assets::materialize(&fx.scope("discipline", None), "omnidesk-base").unwrap();
    let skill = fx.dsh_home.join("skills/memory-agent/SKILL.md");
    fs::write(&skill, "# 我自己改的\n").unwrap();
    let report = profile_assets::materialize(&fx.scope("discipline", None), "omnidesk-base").unwrap();
    assert_eq!(status_of(&report, "skill:memory-agent"), "drift");
    assert_eq!(fs::read_to_string(&skill).unwrap(), "# 我自己改的\n");
}


/// 无仓库时巡检要如实说「内嵌可补」，而不是报源缺失。
#[test]
fn inspect_without_repo_expects_embedded_seed() {
    let fx = Fixture::new("embedded-inspect");
    let view = profile_assets::inspect(&fx.scope("discipline", None), "omnidesk-base").unwrap();
    assert!(view.repo_missing.is_none(), "{:?}", view.repo_missing);
    assert_eq!(status_of(&view, "skill:memory-agent"), "missing");
    assert!(!fx.dsh_home.exists(), "巡检不能写盘");
}

/// OD-ARCH-113： Skill 正文被改过，巡检必须报 drift，而不是「已就位」。
#[test]
fn inspect_detects_edited_skill_body() {
    let fx = Fixture::new("inspect-skill-drift");
    let repo = Fixture::repo();
    profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    let skill = fx.dsh_home.join("skills/memory-agent/SKILL.md");
    assert_eq!(
        status_of(
            &profile_assets::inspect(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap(),
            "skill:memory-agent"
        ),
        "ready"
    );
    fs::write(&skill, "# 我自己改的\n").unwrap();
    let view = profile_assets::inspect(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    assert_eq!(status_of(&view, "skill:memory-agent"), "drift");
    // 巡检仍然不写盘：内容原样。
    assert_eq!(fs::read_to_string(&skill).unwrap(), "# 我自己改的\n");
}

/// OD-ARCH-113：显式修复补齐缺失项，并把被改过的文件先备份再写回。
#[test]
fn repair_fills_missing_and_backs_up_drift() {
    let fx = Fixture::new("repair-missing");
    let repo = Fixture::repo();
    profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    fs::remove_dir_all(fx.dsh_home.join("skills/memory-agent")).unwrap();
    let rules = fx.app_root.join("profiles/discipline/rules.json");
    fs::write(&rules, "{\"mine\": true}\n").unwrap();

    let report =
        profile_assets::materialize_with(&fx.scope("discipline", Some(&repo)), "omnidesk-base", true)
            .unwrap();
    assert_eq!(
        status_of(&report, "skill:memory-agent"),
        "created",
        "缺失的 Skill 应被补齐"
    );
    assert_eq!(status_of(&report, "rules.json"), "updated");
    // 被覆盖的文件先备份，备份里是用户原文。
    let backups: Vec<_> = fs::read_dir(rules.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".bak-"))
        .map(|e| e.path())
        .collect();
    assert_eq!(backups.len(), 1, "覆盖前必须留一份备份：{:?}", backups);
    assert_eq!(fs::read_to_string(&backups[0]).unwrap(), "{\"mine\": true}\n");
    assert_ne!(fs::read_to_string(&rules).unwrap(), "{\"mine\": true}\n");
}

/// OD-ARCH-113：标准 Profile 的修复仍然是零写入。
#[test]
fn repair_standard_profile_still_writes_nothing() {
    let fx = Fixture::new("repair-standard");
    let repo = Fixture::repo();
    let report =
        profile_assets::materialize_with(&fx.scope("omnidesk-base", Some(&repo)), "omnidesk-base", true)
            .unwrap();
    assert!(report.standard);
    assert!(report.items.is_empty());
    assert!(!fx.dsh_home.exists(), "标准模式不能创建 DSH_HOME");
    assert!(!fx.app_root.exists(), "标准模式不能创建应用数据根");
}

/// OD-ARCH-115：切回基础标准时，OmniDesk 写的 Skill 必须真的离开全局 skills/。
#[test]
fn switching_back_to_standard_parks_managed_skills() {
    let fx = Fixture::new("park-on-standard");
    let repo = Fixture::repo();
    profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();

    let skills_root = fx.dsh_home.join("skills");
    let registry = skills_root.join(profile_assets::MANAGED_SKILLS_FILE);
    assert!(registry.is_file(), "装配后必须有登记表：{}", registry.display());
    let registered = fs::read_to_string(&registry).unwrap();
    assert_eq!(
        registered.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).count(),
        10,
        "{} 应登记 10 个 Skill：{}",
        registry.display(),
        registered
    );

    let report = profile_assets::reconcile_managed_skills(
        &fx.scope("omnidesk-base", Some(&repo)),
        "omnidesk-base",
    )
    .unwrap();
    assert_eq!(report.parked.len(), 10, "{:?}", report.parked);
    assert!(report.returned.is_empty());

    // 全局目录里不该再有这些 Skill，也不该再留登记表。
    for name in &report.parked {
        assert!(!skills_root.join(name).exists(), "{} 仍留在 skills/", name);
        assert!(
            fx.app_root
                .join(profile_assets::PARKED_SKILLS_DIR)
                .join(name)
                .join("SKILL.md")
                .is_file(),
            "{} 没有停放到停放区",
            name
        );
    }
    let leftovers: Vec<String> = fs::read_dir(&skills_root)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(leftovers.is_empty(), "skills/ 应为空（含登记表）：{:?}", leftovers);
}

/// OD-ARCH-115：切回专业 Profile 时，停放的 Skill 要归位，且不再重复写盘。
#[test]
fn switching_back_to_profile_returns_parked_skills() {
    let fx = Fixture::new("return-parked");
    let repo = Fixture::repo();
    profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    profile_assets::reconcile_managed_skills(
        &fx.scope("omnidesk-base", Some(&repo)),
        "omnidesk-base",
    )
    .unwrap();

    let report = profile_assets::reconcile_managed_skills(
        &fx.scope("discipline", Some(&repo)),
        "omnidesk-base",
    )
    .unwrap();
    assert_eq!(report.returned.len(), 10, "{:?}", report.returned);
    assert!(report.parked.is_empty());
    for name in &report.returned {
        assert!(
            fx.dsh_home.join("skills").join(name).join("SKILL.md").is_file(),
            "{} 没有归位",
            name
        );
    }

    // 归位后装配是幂等的：不再改写用户文件。
    let again = profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base")
        .unwrap();
    assert!(
        again
            .items
            .iter()
            .filter(|i| i.kind == "skill")
            .all(|i| i.status == "unchanged" || i.status == "ready"),
        "{:?}",
        again.items.iter().map(|i| (&i.key, &i.status)).collect::<Vec<_>>()
    );
}

/// OD-ARCH-115：停放再归位后，用户改过的 Skill 正文仍然是用户版。
#[test]
fn parked_skill_keeps_user_edit_after_return() {
    let fx = Fixture::new("park-keeps-edit");
    let repo = Fixture::repo();
    profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    let skill = fx.dsh_home.join("skills/memory-agent/SKILL.md");
    fs::write(&skill, "# 用户自己的版本\n").unwrap();

    profile_assets::reconcile_managed_skills(
        &fx.scope("omnidesk-base", Some(&repo)),
        "omnidesk-base",
    )
    .unwrap();
    assert!(!skill.exists(), "应已移入停放区");

    profile_assets::reconcile_managed_skills(
        &fx.scope("discipline", Some(&repo)),
        "omnidesk-base",
    )
    .unwrap();
    profile_assets::materialize(&fx.scope("discipline", Some(&repo)), "omnidesk-base").unwrap();
    assert_eq!(
        fs::read_to_string(&skill).unwrap(),
        "# 用户自己的版本\n",
        "归位后不得用出厂内容覆盖用户改动"
    );
}

/// 造一个「只带一个版本号」的假安装锚，把真实 CLI 转发出去（OD-ARCH-120）。
///
/// 这样探测只看得到隔离根里的包，不会因为运行时自带同名包而直接判就绪，
/// 而安装命令仍然是真实的 DSH CLI 通道。
fn forwarding_cli(root: &Path, real: &Path) -> PathBuf {
    let pkg = root.join("node_modules/@deepseek-ai/dsh");
    fs::create_dir_all(pkg.join("lib")).unwrap();
    fs::write(
        pkg.join("package.json"),
        r#"{"name":"@deepseek-ai/dsh","version":"0.2.1-alpha.2"}"#,
    )
    .unwrap();
    let wrapper = format!(
        "const {{ spawnSync }} = require('node:child_process');\n\
         const r = spawnSync(process.execPath, [{real:?}, ...process.argv.slice(2)], {{ stdio: 'inherit', env: process.env }});\n\
         process.exit(r.status === null ? 1 : r.status);\n",
        real = real.display().to_string()
    );
    let bin = pkg.join("lib/bin.js");
    fs::write(&bin, wrapper).unwrap();
    bin
}

/// 真实联网安装：只有显式 `OMNIDESK_NET_TESTS=1` 才跑（OD-ARCH-120）。
///
/// 走的是应用真实入口 `ensure_plugins` → `install_plugin`，不是裸 CLI，
/// 因此能证明「版本钉到运行时 + 构建脚本拦截自动放行」这条链路真的装得上。
#[test]
fn ensure_plugins_installs_over_network_when_enabled() {
    if std::env::var("OMNIDESK_NET_TESTS").ok().as_deref() != Some("1") {
        return;
    }
    let fx = Fixture::new("net-install");
    let repo = Fixture::repo();
    let real = profile_assets::default_dsh_cli_for(Some(&repo), None);
    assert!(real.is_file(), "找不到 DSH CLI：{}", real.display());
    let cli = forwarding_cli(&fx.base.join("anchor"), &real);

    fs::create_dir_all(fx.app_root.join("profiles")).unwrap();
    fs::write(
        fx.app_root.join("profiles/index.v1.json"),
        r#"{"schema":"omnidesk.profiles.v1","profiles":[{"name":"netprobe","label":"联网探针","created_at":1,"origin":"standard","plugins":["@deepseek-ai/dsh-skill-office"]}]}"#,
    )
    .unwrap();

    let scope = fx.scope("netprobe", Some(&repo));
    let receipt = profile_assets::ensure_plugins(&scope, "omnidesk-base", &cli)
        .expect("安装入口应返回回执");
    assert!(
        receipt.ok,
        "真实安装失败：{:?}\n仍缺：{:?}",
        receipt
            .results
            .iter()
            .map(|r| (&r.package, r.exit_code, &r.note, &r.stderr))
            .collect::<Vec<_>>(),
        receipt.still_missing
    );
    assert_eq!(
        receipt.attempted,
        vec!["@deepseek-ai/dsh-skill-office".to_string()]
    );
    let profile = fx.dsh_home.join("profiles/netprobe");
    assert!(
        profile
            .join("node_modules/@deepseek-ai/dsh-skill-office/package.json")
            .is_file(),
        "装完应在 Profile 的 node_modules 里"
    );
    let workspace = fs::read_to_string(profile.join("pnpm-workspace.yaml")).unwrap_or_default();
    // 只有第一次真的被 pnpm 拦住时才需要写这个开关；已装好的机器上直接成功。
    if receipt
        .results
        .iter()
        .any(|r| format!("{}{}", r.stdout, r.stderr).contains("Ignored build scripts"))
        || receipt.results.iter().any(|r| r.note.contains("构建脚本"))
    {
        assert!(
            workspace.contains("strictDepBuilds: false"),
            "pnpm 的构建脚本拦截应被自动放行：{workspace}"
        );
    }

    let again = profile_assets::ensure_plugins(&scope, "omnidesk-base", &cli).unwrap();
    assert!(again.attempted.is_empty(), "第二次不该再装：{:?}", again.attempted);
}

