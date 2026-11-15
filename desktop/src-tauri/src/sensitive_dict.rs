//! 出站敏感词典与硬阻断词（OD-MASK-160d/e）。
//!
//! 文件固定在 `<数据根>/config/sensitive-dict.v1.json`（文件名沿用 v1，内容 schema 为 v2）。
//! 启动时文件不存在则写入空模板；设置页保存时校验字段、数量与大小，旧文件先备份，
//! 再走与模型配置同一套原子写（临时文件 → fsync → rename）。
//!
//! 本模块只维护用户自己的词表，不内置真实领导人姓名或真实色情词；出厂样例里的
//! 阻断词全部是虚构占位（「示例阻断词甲」等）。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::app_paths::AppPaths;
use crate::model_settings::write_atomic;

pub const SCHEMA_V1: &str = "omnidesk.sensitive-dict.v1";
pub const SCHEMA_V2: &str = "omnidesk.sensitive-dict.v2";
pub const FILE_NAME: &str = "sensitive-dict.v1.json";
pub const MAX_BYTES: usize = 256 * 1024;
pub const MAX_ITEMS: usize = 2000;
pub const MAX_TERM_CHARS: usize = 120;
pub const ALLOWED_TYPES: &[&str] = &["PERSON", "BRANCH", "ORG", "CUSTOM"];
pub const ALLOWED_CATEGORIES: &[&str] = &["political", "porn", "leader", "custom"];

/// 出厂样例（含虚构示例词）。启动模板会清空 entries/blocklist 后写出。
const SAMPLE: &str = include_str!("../../../config/sensitive-dict.sample.v1.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DictEntry {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTerm {
    pub term: String,
    #[serde(default = "default_category")]
    pub category: String,
}

fn default_category() -> String {
    "custom".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitiveDict {
    pub schema: String,
    #[serde(default)]
    pub entries: Vec<DictEntry>,
    #[serde(default)]
    pub blocklist: Vec<BlockTerm>,
}

/// 设置页读取结果。文件损坏时 `error` 有值，界面必须显式提示，不能当作空词表静默覆盖。
#[derive(Debug, Clone, Serialize)]
pub struct DictView {
    pub path: String,
    pub exists: bool,
    pub schema: String,
    pub entries: Vec<DictEntry>,
    pub blocklist: Vec<BlockTerm>,
    pub bytes: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaveReceipt {
    pub ok: bool,
    pub path: String,
    pub backup: Option<String>,
    pub bytes: usize,
    pub note: String,
}

pub fn path(config_dir: &Path) -> PathBuf {
    config_dir.join(FILE_NAME)
}

fn parsed_sample() -> SensitiveDict {
    serde_json::from_str::<SensitiveDict>(SAMPLE).unwrap_or_else(|_| SensitiveDict {
        schema: SCHEMA_V2.to_string(),
        entries: Vec::new(),
        blocklist: Vec::new(),
    })
}

/// 启动期默认模板：schema v2、entries:[]、blocklist:[]。
pub fn default_template() -> SensitiveDict {
    let mut dict = parsed_sample();
    dict.schema = SCHEMA_V2.to_string();
    dict.entries.clear();
    dict.blocklist.clear();
    dict
}

fn template_bytes() -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(&default_template())
        .map_err(|e| format!("序列化默认敏感词典失败：{}", e))
}

/// 文件不存在时写入默认模板；已存在时一个字节都不动。
pub fn ensure(paths: &AppPaths) -> Result<bool, String> {
    let target = path(&paths.config);
    if target.exists() {
        return Ok(false);
    }
    let bytes = template_bytes()?;
    write_atomic(&target, &bytes)?;
    Ok(true)
}

pub fn load(config_dir: &Path) -> DictView {
    let target = path(config_dir);
    let raw = match fs::read(&target) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let template = default_template();
            return DictView {
                path: target.display().to_string(),
                exists: false,
                schema: template.schema,
                entries: template.entries,
                blocklist: template.blocklist,
                bytes: 0,
                error: None,
            };
        }
        Err(error) => {
            return DictView {
                path: target.display().to_string(),
                exists: true,
                schema: String::new(),
                entries: Vec::new(),
                blocklist: Vec::new(),
                bytes: 0,
                error: Some(format!("读取敏感词典失败：{}", error)),
            };
        }
    };
    let bytes = raw.len();
    let mut dict = match serde_json::from_slice::<SensitiveDict>(&raw) {
        Ok(dict) => dict,
        Err(error) => {
            return DictView {
                path: target.display().to_string(),
                exists: true,
                schema: String::new(),
                entries: Vec::new(),
                blocklist: Vec::new(),
                bytes,
                error: Some(format!("敏感词典解析失败：{}", error)),
            };
        }
    };
    let mut error = None;
    if dict.schema != SCHEMA_V1 && dict.schema != SCHEMA_V2 {
        error = Some(format!(
            "schema 必须是 {} 或 {}，收到 {}",
            SCHEMA_V1, SCHEMA_V2, dict.schema
        ));
    }
    if dict.schema == SCHEMA_V1 {
        dict.blocklist.clear();
    }
    DictView {
        path: target.display().to_string(),
        exists: true,
        schema: dict.schema,
        entries: dict.entries,
        blocklist: dict.blocklist,
        bytes,
        error,
    }
}

impl SensitiveDict {
    fn normalized(mut self) -> Self {
        for entry in &mut self.entries {
            entry.kind = entry.kind.trim().to_ascii_uppercase();
            entry.value = entry.value.trim().to_string();
            let value = entry.value.clone();
            let mut aliases: Vec<String> = Vec::new();
            for alias in entry.aliases.drain(..) {
                let alias = alias.trim().to_string();
                if !alias.is_empty() && alias != value && !aliases.contains(&alias) {
                    aliases.push(alias);
                }
            }
            entry.aliases = aliases;
        }
        for term in &mut self.blocklist {
            term.term = term.term.trim().to_string();
            term.category = term.category.trim().to_ascii_lowercase();
        }
        self.entries.retain(|entry| !entry.value.is_empty());
        self.blocklist.retain(|term| !term.term.is_empty());
        self.schema = SCHEMA_V2.to_string();
        self
    }

    fn validate(&self) -> Result<(), String> {
        if self.entries.len() > MAX_ITEMS {
            return Err(format!("敏感实体最多 {} 条", MAX_ITEMS));
        }
        if self.blocklist.len() > MAX_ITEMS {
            return Err(format!("阻断词最多 {} 条", MAX_ITEMS));
        }
        for (index, entry) in self.entries.iter().enumerate() {
            if !ALLOWED_TYPES.contains(&entry.kind.as_str()) {
                return Err(format!("第 {} 条实体类型无效：{}", index + 1, entry.kind));
            }
            if entry.value.is_empty() || entry.value.chars().count() > MAX_TERM_CHARS {
                return Err(format!("第 {} 条实体 value 不能为空且不超过 {} 字", index + 1, MAX_TERM_CHARS));
            }
            for alias in &entry.aliases {
                if alias.is_empty() || alias.chars().count() > MAX_TERM_CHARS {
                    return Err(format!("第 {} 条实体的别名不能为空且不超过 {} 字", index + 1, MAX_TERM_CHARS));
                }
            }
        }
        for (index, term) in self.blocklist.iter().enumerate() {
            if !ALLOWED_CATEGORIES.contains(&term.category.as_str()) {
                return Err(format!("第 {} 条阻断词类别无效：{}", index + 1, term.category));
            }
            if term.term.is_empty() || term.term.chars().count() > MAX_TERM_CHARS {
                return Err(format!("第 {} 条阻断词不能为空且不超过 {} 字", index + 1, MAX_TERM_CHARS));
            }
        }
        let bytes = serde_json::to_vec(self).map_err(|e| format!("序列化敏感词典失败：{}", e))?;
        if bytes.len() > MAX_BYTES {
            return Err(format!("敏感词典不能超过 {} KB", MAX_BYTES / 1024));
        }
        Ok(())
    }
}

pub fn save(
    config_dir: &Path,
    entries: Vec<DictEntry>,
    blocklist: Vec<BlockTerm>,
) -> Result<SaveReceipt, String> {
    let dict = SensitiveDict {
        schema: SCHEMA_V2.to_string(),
        entries,
        blocklist,
    }
    .normalized();
    dict.validate()?;
    let bytes = serde_json::to_vec_pretty(&dict)
        .map_err(|e| format!("序列化敏感词典失败：{}", e))?;
    if bytes.len() > MAX_BYTES {
        return Err(format!("敏感词典不能超过 {} KB", MAX_BYTES / 1024));
    }

    let target = path(config_dir);
    let backup = if target.exists() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut candidate = target.with_file_name(format!("{}.bak-{}", FILE_NAME, stamp));
        let mut suffix = 1;
        while candidate.exists() {
            candidate = target.with_file_name(format!("{}.bak-{}-{}", FILE_NAME, stamp, suffix));
            suffix += 1;
        }
        fs::copy(&target, &candidate)
            .map_err(|e| format!("备份旧敏感词典失败（{}）：{}", candidate.display(), e))?;
        Some(candidate.display().to_string())
    } else {
        None
    };
    write_atomic(&target, &bytes)?;
    Ok(SaveReceipt {
        ok: true,
        path: target.display().to_string(),
        backup,
        bytes: bytes.len(),
        note: "敏感词典已保存；下一轮提交生效".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQ: AtomicUsize = AtomicUsize::new(0);

    struct TempRoot {
        base: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "omnidesk-sensitive-dict-{}-{}-{}",
                tag,
                std::process::id(),
                SEQ.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = fs::remove_dir_all(&base);
            fs::create_dir_all(&base).unwrap();
            Self { base }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn paths_for(tmp: &TempRoot) -> AppPaths {
        AppPaths {
            root: tmp.base.clone(),
            ui_state: tmp.base.join("ui-state"),
            logs: tmp.base.join("logs"),
            engine_state: tmp.base.join("engine-state"),
            results: tmp.base.join("engine-state").join("results"),
            threads: tmp.base.join("threads"),
            config: tmp.base.join("config"),
        }
    }

    #[test]
    fn ensure_creates_empty_v2_when_missing() {
        let tmp = TempRoot::new("create");
        let paths = paths_for(&tmp);
        assert!(ensure(&paths).unwrap(), "首次应创建");
        let target = path(&paths.config);
        let text = fs::read_to_string(&target).unwrap();
        let parsed: SensitiveDict = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.schema, SCHEMA_V2);
        assert!(parsed.entries.is_empty());
        assert!(parsed.blocklist.is_empty());
        assert!(!ensure(&paths).unwrap(), "已存在时不应重复创建");
    }

    #[test]
    fn ensure_keeps_existing_bytes() {
        let tmp = TempRoot::new("keep");
        let paths = paths_for(&tmp);
        fs::create_dir_all(&paths.config).unwrap();
        let target = path(&paths.config);
        let original = r#"{"schema":"omnidesk.sensitive-dict.v2","entries":[{"type":"PERSON","value":"合成甲"}],"blocklist":[]}"#;
        fs::write(&target, original).unwrap();
        assert!(!ensure(&paths).unwrap());
        assert_eq!(fs::read_to_string(&target).unwrap(), original);
    }

    #[test]
    fn save_rejects_empty_and_oversize_fields() {
        let tmp = TempRoot::new("reject");
        let paths = paths_for(&tmp);
        let bad_type = vec![DictEntry {
            kind: "OPS".to_string(),
            value: "合成甲".to_string(),
            aliases: vec![],
        }];
        assert!(save(&paths.config, bad_type, vec![]).is_err());
        let oversized = vec![DictEntry {
            kind: "PERSON".to_string(),
            value: "甲".repeat(MAX_TERM_CHARS + 1),
            aliases: vec![],
        }];
        assert!(save(&paths.config, oversized, vec![]).is_err());
        let bad_category = vec![BlockTerm {
            term: "合成阻断词".to_string(),
            category: "unknown".to_string(),
        }];
        assert!(save(&paths.config, vec![], bad_category).is_err());
    }

    #[test]
    fn save_writes_v2_and_backs_up_previous() {
        let tmp = TempRoot::new("save");
        let paths = paths_for(&tmp);
        assert!(ensure(&paths).unwrap());
        let receipt = save(
            &paths.config,
            vec![DictEntry {
                kind: "PERSON".to_string(),
                value: " 合成甲 ".to_string(),
                aliases: vec![" 合成甲同志 ".to_string(), "".to_string()],
            }],
            vec![BlockTerm {
                term: " 示例阻断词甲 ".to_string(),
                category: "CUSTOM".to_string(),
            }],
        )
        .unwrap();
        assert!(receipt.ok);
        assert!(receipt.backup.is_some(), "旧文件应备份");
        let view = load(&paths.config);
        assert!(view.error.is_none(), "{:?}", view.error);
        assert_eq!(view.entries[0].value, "合成甲");
        assert_eq!(view.entries[0].aliases, vec!["合成甲同志".to_string()]);
        assert_eq!(view.blocklist[0].category, "custom");
    }
}
