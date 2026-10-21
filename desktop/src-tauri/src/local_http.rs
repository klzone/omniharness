//! 本机回环的最小 HTTP/1.1 客户端（OD-ARCH-053 S1）。
//!
//! 为什么自己写：桌面宿主只允许依赖 tauri / serde / serde_json（离线构建、锁文件不动）。
//! 需要一个能对 **本机 DSH web 实例** 发一次 JSON 请求的通道，用来调用官方
//! `credentials` RPC（写入模型密钥）。
//!
//! 硬边界：
//!   * 只连 `127.0.0.1`，不接受任意主机名或 URL；
//!   * 无重定向、无代理、无 TLS（DSH 本地实例是明文回环）；
//!   * 响应体与头部有上限，防止异常响应把内存吃满；
//!   * 不记录请求体——调用方把密钥放进去，日志只能出现打码结果。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（本地 TcpListener 回放）。

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    /// 头部键统一小写。
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        let want = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == want)
            .map(|(_, v)| v.as_str())
    }

    /// 同一头部出现多次时全部取出（`Set-Cookie` 可能有多条）。
    pub fn header_all(&self, name: &str) -> Vec<&str> {
        let want = name.to_ascii_lowercase();
        self.headers
            .iter()
            .filter(|(k, _)| *k == want)
            .map(|(_, v)| v.as_str())
            .collect()
    }
}

/// 对 `127.0.0.1:port` 发一次请求。`path` 必须以 `/` 开头且不含换行。
pub fn request(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, String)],
    body: Option<&str>,
    timeout: Duration,
) -> Result<Response, String> {
    if !path.starts_with('/') || path.contains(['\r', '\n']) {
        return Err("拒绝发送：请求路径必须以 / 开头且不能含换行".to_string());
    }
    if !matches!(method, "GET" | "POST") {
        return Err(format!("拒绝发送：本客户端只支持 GET/POST，收到 {}", method));
    }
    let addr = ("127.0.0.1", port)
        .to_socket_addrs()
        .map_err(|e| format!("无法解析本机地址：{}", e))?
        .next()
        .ok_or_else(|| "本机地址解析结果为空".to_string())?;
    let mut stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| format!("连接本机 DSH 实例失败（端口 {}）：{}", port, e))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("设置读取超时失败：{}", e))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| format!("设置写入超时失败：{}", e))?;

    let mut head = format!("{} {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n", method, path, port);
    for (k, v) in headers {
        if k.contains(['\r', '\n']) || v.contains(['\r', '\n']) {
            return Err("拒绝发送：请求头含换行".to_string());
        }
        head.push_str(&format!("{}: {}\r\n", k, v));
    }
    let payload = body.unwrap_or("");
    if body.is_some() {
        head.push_str(&format!("Content-Length: {}\r\n", payload.len()));
    }
    head.push_str("\r\n");
    head.push_str(payload);

    stream
        .write_all(head.as_bytes())
        .and_then(|_| stream.flush())
        .map_err(|e| format!("向本机 DSH 实例发送请求失败：{}", e))?;

    let mut raw: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&buf[..n]);
                if raw.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
                    return Err("拒绝解析：本机响应体超过上限".to_string());
                }
                if let Some(done) = complete(&raw) {
                    raw.truncate(done);
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => break,
            Err(e) => return Err(format!("读取本机 DSH 实例响应失败：{}", e)),
        }
    }
    parse(&raw)
}

/// 判断已经收够整条响应时返回其长度。
fn complete(raw: &[u8]) -> Option<usize> {
    let split = find_header_end(raw)?;
    let head = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    let body_start = split + 4;
    if let Some(len) = content_length(&head) {
        if raw.len() >= body_start + len {
            return Some(body_start + len);
        }
        return None;
    }
    if head.contains("transfer-encoding: chunked") {
        // 分块体结束标志：最后一个 0 长度块后跟空行。
        let body = &raw[body_start..];
        if body.windows(5).any(|w| w == b"\r\n0\r\n") || body.starts_with(b"0\r\n") {
            return Some(raw.len());
        }
        return None;
    }
    // 无长度也无分块：读到连接关闭为止（`Connection: close`）。
    None
}

fn find_header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|w| w == b"\r\n\r\n")
}

fn content_length(head: &str) -> Option<usize> {
    for line in head.split("\r\n") {
        if let Some(rest) = line.strip_prefix("content-length:") {
            return rest.trim().parse::<usize>().ok();
        }
    }
    None
}

fn parse(raw: &[u8]) -> Result<Response, String> {
    let split = find_header_end(raw).ok_or_else(|| "本机响应头部不完整".to_string())?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| format!("本机响应状态行无法解析：{}", status_line))?;
    let mut headers: Vec<(String, String)> = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    let body_raw = &raw[split + 4..];
    let chunked = headers
        .iter()
        .any(|(k, v)| k == "transfer-encoding" && v.to_ascii_lowercase().contains("chunked"));
    let body_bytes = if chunked {
        dechunk(body_raw)?
    } else {
        body_raw.to_vec()
    };
    Ok(Response {
        status,
        headers,
        body: String::from_utf8_lossy(&body_bytes).to_string(),
    })
}

fn dechunk(body: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    loop {
        let line_end = match body[i..].windows(2).position(|w| w == b"\r\n") {
            Some(p) => i + p,
            None => break,
        };
        let size_text = String::from_utf8_lossy(&body[i..line_end]).to_string();
        let size_hex = size_text.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|e| format!("分块长度无法解析（{}）：{}", size_hex, e))?;
        if size == 0 {
            break;
        }
        let start = line_end + 2;
        let end = start + size;
        if end > body.len() {
            return Err("分块响应体被截断".to_string());
        }
        out.extend_from_slice(&body[start..end]);
        i = end + 2;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serve_once(response: &'static str) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf);
                let _ = sock.write_all(response.as_bytes());
            }
        });
        port
    }

    #[test]
    fn reads_content_length_body() {
        let port = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"ok\":true}",
        );
        let r = request(port, "POST", "/api/x", &[], Some("{}"), Duration::from_secs(5)).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.header("content-type"), Some("application/json"));
        assert_eq!(r.body, "{\"ok\":true}");
    }

    #[test]
    fn reads_chunked_body_and_multi_cookie() {
        let port = serve_once(
            "HTTP/1.1 303 See Other\r\nSet-Cookie: dsh_auth=abc; Path=/\r\nSet-Cookie: other=1; Path=/\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
        );
        let r = request(port, "GET", "/?token=t", &[], None, Duration::from_secs(5)).unwrap();
        assert_eq!(r.status, 303);
        assert_eq!(r.body, "hello world");
        assert_eq!(r.header_all("set-cookie").len(), 2);
    }

    #[test]
    fn rejects_bad_path() {
        assert!(request(1, "GET", "nope", &[], None, Duration::from_secs(1)).is_err());
        assert!(request(1, "GET", "/a\r\nX: y", &[], None, Duration::from_secs(1)).is_err());
    }
}
