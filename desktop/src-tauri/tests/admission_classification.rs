//! OD-ARCH-021：按实际 DSH 配置做端点分类的集成测试。
//!
//! 这里运行真实的 `dsh --dump-config`（不联网、不读凭证、不启动模型），
//! 把临时 Profile 补丁指向本机回环地址，验证宿主能否从配置本身得出端点类别。

use std::fs;
use std::path::{Path, PathBuf};

use omnidesk_desktop_lib::admission::{self, AdmissionFacts};
use omnidesk_desktop_lib::engine_supervisor::EngineConfig;
use omnidesk_desktop_lib::settings::ModelPermissions;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("仓库根")
        .to_path_buf()
}

fn dsh_cli() -> PathBuf {
    repo_root().join("runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js")
}

fn temp_root(tag: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "omnidesk-admission-classify-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(base.join("dsh-home")).unwrap();
    fs::create_dir_all(base.join("agents")).unwrap();
    fs::create_dir_all(base.join("config")).unwrap();
    base
}

fn overlay(base: &Path, provider: &str, base_url: Option<&str>) -> PathBuf {
    // OD-ARCH-048：宿主默认通道是 web-api，实际生效路由条目为
    // `agent-default-model`（web app 不加载 dsh-acp）。夹具同时声明两条同源
    // 路由，使两种通道都指向同一端点；只写 acp 会落到 DSH 内置 DeepSeek
    // 默认路由，测不到夹具意图。
    // 条目的 plugin name 必须与 id 匹配，否则 DSH 只打印 "name mismatch ... skipping"
    // 并静默回落到内置 DeepSeek 默认路由（OD-ARCH-048 实测）。
    let mut lines = Vec::new();
    for (id, name) in [
        ("acp", "@deepseek-ai/dsh-acp"),
        ("agent-default-model", "@deepseek-ai/dsh-agent-default-model"),
    ] {
        lines.extend([
            format!("- id: {}", id),
            format!("  name: \"{}\"", name),
            "  config:".to_string(),
            format!("    provider: {}", provider),
            "    model: synthetic-model".to_string(),
        ]);
    }
    if let Some(url) = base_url {
        lines.extend([
            "- id: llm-pi-ai".to_string(),
            "  name: \"@deepseek-ai/dsh-llm-pi-ai\"".to_string(),
            "  config:".to_string(),
            "    providers:".to_string(),
            format!("      {}:", provider),
            "        apiKeyEnv: OMNIDESK_SYNTHETIC_KEY".to_string(),
            "        api: openai-completions".to_string(),
            format!("        baseURL: {}", url),
            "        models:".to_string(),
            "          - id: synthetic-model".to_string(),
            "            name: synthetic-model".to_string(),
        ]);
    }
    let path = base.join("overlay.yml");
    fs::write(&path, lines.join("\n") + "\n").unwrap();
    path
}

fn config(base: &Path, patch: PathBuf) -> EngineConfig {
    EngineConfig {
        python: "python3".to_string(),
        engine_dir: repo_root().join("engine"),
        dsh_cli: dsh_cli(),
        dsh_home: base.join("dsh-home"),
        agents_home: base.join("agents"),
        engine_state_dir: base.join("state"),
        result_dir: base.join("state/results"),
        // OD-ARCH-048：该夹具只测端点分类，通道与宿主默认一致（web-api）。
        channel: "web-api".to_string(),
        profile: "acp".to_string(),
        patch_files: vec![patch],
    }
}

#[test]
fn classify_loopback_route_and_enforce_permissions() {
    if !dsh_cli().exists() {
        eprintln!("跳过：本机没有仓库内 DSH CLI（{}）", dsh_cli().display());
        return;
    }
    let base = temp_root("local");
    let patch = overlay(&base, "synthetic", Some("http://127.0.0.1:9/v1"));
    let config = config(&base, patch);

    let facts = admission::read_facts(&config, &base.join("config"))
        .expect("本机回环端点应能分类");
    assert_eq!(facts.route.category, "local");
    assert_eq!(facts.route.provider, "synthetic");
    assert_eq!(facts.route.leaves_device, Some(false));
    assert!(facts.route.is_configurable());
    // 首次（无设置文件）必须全部关闭。
    assert_eq!(facts.permissions, ModelPermissions::default());
    let denied = admission::require_allowed(&facts, admission::SUBMIT_REFUSAL_MESSAGE)
        .expect_err("默认权限下必须拒绝");
    assert_eq!(denied["engine_error_code"], admission::REFUSAL_CODE);

    // 开启本机类别后放行；其他类别仍关闭。
    let allowed = AdmissionFacts {
        route: facts.route.clone(),
        permissions: ModelPermissions { local: true, private: false, public: false },
    };
    assert!(admission::require_allowed(&allowed, admission::SUBMIT_REFUSAL_MESSAGE).is_ok());

    // 撤销全部后再次拒绝。
    let mut revoked = allowed.clone();
    revoked.permissions.revoke_all();
    assert!(admission::require_allowed(&revoked, admission::SUBMIT_REFUSAL_MESSAGE).is_err());

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn unresolvable_provider_is_unknown_and_never_allowed() {
    if !dsh_cli().exists() {
        eprintln!("跳过：本机没有仓库内 DSH CLI（{}）", dsh_cli().display());
        return;
    }
    let base = temp_root("unknown");
    // 没有 llm-pi-ai 配置的自定义 provider：端点无法定位。
    let patch = overlay(&base, "mystery", None);
    let config = config(&base, patch);
    let err = admission::read_facts(&config, &base.join("config"))
        .expect_err("无法解析的端点不得默认为本机");
    assert_eq!(err["engine_error_code"], admission::CLASSIFIER_FAILED_CODE);
    let text = err.to_string();
    assert!(!text.contains("127.0.0.1"), "拒绝信息不应泄露端点：{}", text);

    let _ = fs::remove_dir_all(&base);
}
