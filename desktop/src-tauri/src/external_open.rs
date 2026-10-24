//! 用系统默认程序打开外部链接（OD-ARCH-081）。
//!
//! 为什么需要：Markdown 正文里的 `http(s)` 链接如果交给 WebView 自己导航，
//! 会把应用界面整个跳走，用户回不到工作台；Tauri 的能力集也没有开放外部打开插件。
//! 这里只放行 `http`/`https` 的绝对地址，交给系统浏览器，其余一律拒绝。
//!
//! 硬边界（由代码强制）：
//!   1. 只接受 http / https；
//!   2. 拒绝带用户名/密码的地址（避免把凭据交给浏览器历史与系统日志）；
//!   3. 拒绝控制字符与空白，长度上限 2048；
//!   4. 不拼接 shell：`xdg-open` / `gio open` 以地址作为独立参数执行。

use std::process::{Command, Stdio};

const MAX_URL_LEN: usize = 2048;

/// 校验外部链接是否允许打开；返回规范化后的地址或错误说明。
pub fn validate_external_url(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err("链接为空".to_string());
    }
    if url.len() > MAX_URL_LEN {
        return Err("链接过长".to_string());
    }
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("链接包含空白或控制字符".to_string());
    }
    let lower = url.to_ascii_lowercase();
    let rest = if let Some(r) = lower.strip_prefix("https://") {
        r
    } else if let Some(r) = lower.strip_prefix("http://") {
        r
    } else {
        return Err("只允许 http / https 链接".to_string());
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err("链接缺少主机名".to_string());
    }
    if authority.contains('@') {
        return Err("链接不允许携带用户名或密码".to_string());
    }
    Ok(url.to_string())
}

/// 交给系统默认浏览器打开；失败时返回可读原因，不吞错误。
pub fn open_in_browser(raw: &str) -> Result<String, String> {
    let url = validate_external_url(raw)?;
    let mut last_error = String::new();
    for (program, args) in [
        ("xdg-open", Vec::<&str>::new()),
        ("gio", vec!["open"]),
    ] {
        match Command::new(program)
            .args(&args)
            .arg(&url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(_) => return Ok(url),
            Err(e) => last_error = format!("{}：{}", program, e),
        }
    }
    Err(format!("系统没有可用的打开器（{}）", last_error))
}

#[cfg(test)]
mod tests {
    use super::validate_external_url;

    #[test]
    fn accepts_plain_http_and_https() {
        for ok in [
            "https://example.com",
            "https://example.com/a/b?x=1#frag",
            "http://127.0.0.1:8080/docs",
            "  https://example.com/x  ",
        ] {
            assert!(validate_external_url(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn rejects_other_schemes_and_credentials() {
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,<script>",
            "vbscript:msgbox",
            "https://user:pw@example.com/x",
            "https://exa mple.com",
            "https://example.com/\u{0}x",
            "example.com",
            "https://",
        ] {
            assert!(validate_external_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn rejects_overlong_url() {
        let long = format!("https://example.com/{}", "a".repeat(3000));
        assert!(validate_external_url(&long).is_err());
    }
}
