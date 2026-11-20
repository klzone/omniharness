//! 外联 URL 统一校验（OD-HARDEN-2043 任务 C）。
//!
//! 口径：只放行 http/https、拒绝带用户名/密码的 URL、拒绝空主机；解析出的每个
//! 地址都必须落在允许范围。调用方用 `ClientBuilder::resolve` 把已校验的地址钉住，
//! 避免 DNS 重绑定（TOT/TOU）。
//!
//! 两类策略：
//!   * `NetPolicy::PublicOnly`：正文里的远程图片/媒体，只允许公网地址；
//!   * `NetPolicy::AllowPrivate`：订阅源在设置里打开「允许内网订阅源」时，
//!     额外放行私网与 CGNAT。回环、链路本地、未指定、多播始终拒绝。
//!
//! 残余风险：配置了系统代理时，reqwest 会把请求交给代理，`resolve` 的钉扎不生效；
//! 逐跳校验仍然有效，但解析结果的钉扎失效。记录见 docs/decisions.md（OD-HARDEN-2043-C）。

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};

use reqwest::Url;

/// 订阅源/远程媒体的地址策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetPolicy {
    /// 只允许公网地址（远程图片与媒体固定使用这一档）。
    PublicOnly,
    /// 额外允许私网与 CGNAT（只由「允许内网订阅源」开关触发）。
    AllowPrivate,
}

#[cfg(test)]
thread_local! {
    /// 线程局部：并行跑测试时，只有显式打开它的那个测试线程放行回环。
    static TEST_ALLOW_LOOPBACK: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

/// 测试专用放行入口：只放行回环地址，供本机 127.0.0.1 监听器的既有单测使用。
/// 生产构建里不存在这个开关。
#[cfg(test)]
pub(crate) fn allow_loopback_in_tests(flag: bool) {
    TEST_ALLOW_LOOPBACK.with(|cell| cell.set(flag));
}

/// 只允许公网地址：`assert_public_http_url("https://example.com/feed")`。
pub fn assert_public_http_url(raw: &str) -> Result<Vec<SocketAddr>, String> {
    assert_http_url(raw, NetPolicy::PublicOnly)
}

/// 解析并校验一个外联地址，返回带端口、已校验过的地址列表。
pub fn assert_http_url(raw: &str, policy: NetPolicy) -> Result<Vec<SocketAddr>, String> {
    let url = Url::parse(raw.trim()).map_err(|_| "地址不是有效的网址。".to_string())?;
    assert_url_addresses(&url, policy)
}

/// 校验已经解析好的 URL（逐跳跳转时复用），返回带端口、已校验过的地址列表。
pub fn assert_url_addresses(url: &Url, policy: NetPolicy) -> Result<Vec<SocketAddr>, String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err("只允许 http/https 地址。".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("地址不能带用户名或密码。".to_string());
    }
    let host_raw = url.host_str().ok_or_else(|| "地址缺少主机名。".to_string())?;
    let host = host_raw.trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() {
        return Err("地址缺少主机名。".to_string());
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "地址缺少可用端口。".to_string())?;
    let resolved: Vec<SocketAddr> = match host.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        Err(_) => (host, port)
            .to_socket_addrs()
            .map_err(|e| format!("解析主机失败（{}）：{}", host, e))?
            .collect(),
    };
    if resolved.is_empty() {
        return Err("主机没有解析出任何地址。".to_string());
    }
    for addr in &resolved {
        if let Some(reason) = deny_reason(addr.ip(), policy) {
            return Err(format!("地址 {} 属于禁止范围（{}）。", addr.ip(), reason));
        }
    }
    Ok(resolved)
}

/// 把已校验的地址钉到客户端上，防 DNS 重绑定。域名主机才需要钉；
/// IP 字面量本来就不会再解析。
pub fn pin_client(
    builder: reqwest::blocking::ClientBuilder,
    url: &Url,
    addrs: &[SocketAddr],
) -> reqwest::blocking::ClientBuilder {
    let mut builder = builder;
    if let Some(host_raw) = url.host_str() {
        let host = host_raw.trim_start_matches('[').trim_end_matches(']');
        if host.parse::<IpAddr>().is_err() {
        let port = url.port_or_known_default().unwrap_or(443);
        for addr in addrs {
                builder = builder.resolve(host, SocketAddr::new(addr.ip(), port));
            }
        }
    }
    builder
}

/// 逐跳校验用：解析 Location 后判断是否允许 https -> http 降级。
pub fn rejects_scheme_downgrade(from: &Url, to: &Url) -> bool {
    from.scheme() == "https" && to.scheme() == "http"
}

fn deny_reason(ip: IpAddr, policy: NetPolicy) -> Option<&'static str> {
    #[cfg(test)]
    {
        if TEST_ALLOW_LOOPBACK.with(|cell| cell.get()) && ip.is_loopback() {
            return None;
        }
    }
    let ip = match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => {
                if v6.is_loopback() {
                    return Some("IPv6 回环");
                }
                let seg = v6.segments()[0];
                if v6.is_unspecified() {
                    return Some("IPv6 未指定地址");
                }
                if seg & 0xffc0 == 0xfe80 {
                    return Some("IPv6 链路本地");
                }
                if seg & 0xfe00 == 0xfc00 {
                    if policy == NetPolicy::AllowPrivate {
                        return None;
                    }
                    return Some("IPv6 唯一本地地址");
                }
                if v6.is_multicast() {
                    return Some("IPv6 多播");
                }
                return None;
            }
        },
        other => other,
    };
    let IpAddr::V4(v4) = ip else {
        return None;
    };
    if v4.is_unspecified() {
        return Some("未指定地址");
    }
    if v4.is_loopback() {
        return Some("回环地址");
    }
    if v4 == Ipv4Addr::BROADCAST {
        return Some("广播地址");
    }
    if v4.is_link_local() {
        return Some("链路本地地址");
    }
    if v4.is_multicast() {
        return Some("多播地址");
    }
    let octets = v4.octets();
    if octets[0] == 100 && (64..=127).contains(&octets[1]) {
        if policy == NetPolicy::AllowPrivate {
            return None;
        }
        return Some("CGNAT 地址");
    }
    if v4.is_private() {
        if policy == NetPolicy::AllowPrivate {
            return None;
        }
        return Some("私网地址");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(ip: &str, policy: NetPolicy) -> Option<&'static str> {
        deny_reason(ip.parse::<IpAddr>().expect("测试地址应能解析"), policy)
    }

    #[test]
    fn denies_always_blocked_ranges() {
        for ip in [
            "0.0.0.0",
            "127.0.0.1",
            "127.1.2.3",
            "169.254.169.254",
            "169.254.0.1",
            "224.0.0.1",
            "239.255.255.250",
            "255.255.255.255",
            "::",
            "::1",
            "fe80::1",
            "febf::1",
            "ff02::1",
        ] {
            assert!(
                verdict(ip, NetPolicy::AllowPrivate).is_some(),
                "{ip} 在允许内网的策略下也必须拒绝"
            );
        }
    }

    #[test]
    fn denies_private_ranges_by_policy() {
        for ip in [
            "10.0.0.1",
            "10.255.255.254",
            "172.16.0.1",
            "172.31.255.254",
            "192.168.1.1",
            "100.64.0.1",
            "100.127.255.254",
            "fd00::1",
            "fc00::1",
        ] {
            assert!(verdict(ip, NetPolicy::PublicOnly).is_some(), "{ip} 默认应拒绝");
            assert!(verdict(ip, NetPolicy::AllowPrivate).is_none(), "{ip} 开关打开后应放行");
        }
    }

    #[test]
    fn allows_public_addresses() {
        for ip in ["8.8.8.8", "1.1.1.1", "93.184.216.34", "2606:4700::1111", "2001:4860::8888"] {
            assert!(verdict(ip, NetPolicy::PublicOnly).is_none(), "{ip} 应放行");
        }
    }

    #[test]
    fn unwraps_ipv4_mapped_ipv6_before_judging() {
        for ip in [
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "::ffff:10.0.0.1",
            "::ffff:100.64.0.1",
        ] {
            assert!(verdict(ip, NetPolicy::PublicOnly).is_some(), "{ip} 应拒绝");
        }
        assert!(verdict("::ffff:8.8.8.8", NetPolicy::PublicOnly).is_none());
    }

    #[test]
    fn url_shape_rules() {
        assert!(assert_public_http_url("http://user:pass@example.com/feed").is_err());
        assert!(assert_public_http_url("http://user@example.com/feed").is_err());
        assert!(assert_public_http_url("ftp://example.com/feed").is_err());
        assert!(assert_public_http_url("file:///etc/passwd").is_err());
        assert!(assert_public_http_url("http:///feed").is_err());
        assert!(assert_public_http_url("http://127.0.0.1:8080/feed").is_err());
        assert!(assert_public_http_url("http://169.254.169.254/latest/meta-data").is_err());
        assert!(assert_public_http_url("http://[::ffff:127.0.0.1]/").is_err());
        assert!(assert_public_http_url("http://[fd00::1]/").is_err());
        assert!(assert_public_http_url("http://[::1]/").is_err());
    }

    #[test]
    fn private_policy_still_denies_loopback() {
        assert!(assert_http_url("http://127.0.0.1:9/feed", NetPolicy::AllowPrivate).is_err());
        assert!(assert_http_url("http://192.168.1.2:9/feed", NetPolicy::AllowPrivate).is_ok());
    }

    #[test]
    fn detects_scheme_downgrade() {
        let https = Url::parse("https://example.com/feed").expect("https url");
        let http = Url::parse("http://example.com/feed").expect("http url");
        assert!(rejects_scheme_downgrade(&https, &http));
        assert!(!rejects_scheme_downgrade(&http, &https));
        assert!(!rejects_scheme_downgrade(&https, &https));
    }
}
