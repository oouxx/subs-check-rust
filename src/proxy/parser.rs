//! 代理链接解析器模块
//! 支持多种代理协议链接的解析

use anyhow::{Result, anyhow};
use libsubconverter::models::Proxy;
use libsubconverter::parser::parse_settings::ParseSettings;
use libsubconverter::parser::subparser::add_nodes;
use regex::Regex;
use std::collections::HashMap;
use url::Url;
/// 代理节点信息
#[derive(Debug, Clone)]
pub struct ParsedProxy {
    /// 节点名称
    pub name: String,
    /// 协议类型
    pub protocol: String,
    /// 服务器地址
    pub server: String,
    /// 端口
    pub port: u16,
    /// 用户名（可选）
    pub username: Option<String>,
    /// 密码（可选）
    pub password: Option<String>,
    /// UUID（用于vmess/vless等协议）
    pub uuid: Option<String>,
    /// 加密方式
    pub cipher: Option<String>,
    /// 是否启用TLS
    pub tls: Option<bool>,
    /// SNI（服务器名称指示）
    pub sni: Option<String>,
    /// 网络类型（ws、tcp、kcp等）
    pub network: Option<String>,
    /// WebSocket路径
    pub ws_path: Option<String>,
    /// 额外参数
    pub extra_params: HashMap<String, String>,
}

impl Default for ParsedProxy {
    fn default() -> Self {
        Self {
            name: String::new(),
            protocol: String::new(),
            server: String::new(),
            port: 0,
            username: None,
            password: None,
            uuid: None,
            cipher: None,
            tls: None,
            sni: None,
            network: None,
            ws_path: None,
            extra_params: HashMap::new(),
        }
    }
}

/// 解析代理链接
pub fn parse_proxy_link(link: &str) -> Result<ParsedProxy> {
    let link = link.trim();

    if link.is_empty() {
        return Err(anyhow!("链接为空"));
    }

    // 尝试解析为URL格式
    if let Ok(mut parsed) = parse_as_url(link) {
        // 从fragment中提取名称
        if let Some(name) = extract_name_from_fragment(link) {
            parsed.name = name;
        }

        // 如果名称仍然为空，生成一个默认名称
        if parsed.name.is_empty() {
            parsed.name = generate_default_name(&parsed);
        }

        return Ok(parsed);
    }

    // 尝试解析为IP:PORT格式
    if let Ok(parsed) = parse_as_ip_port(link) {
        return Ok(parsed);
    }

    // 尝试解析为base64编码的vmess链接
    if let Ok(parsed) = parse_base64_vmess(link) {
        return Ok(parsed);
    }

    Err(anyhow!("无法解析代理链接: {}", link))
}

/// 解析为URL格式
fn parse_as_url(link: &str) -> Result<ParsedProxy> {
    let url = Url::parse(link)?;

    let mut parsed = ParsedProxy::default();

    // 提取协议
    parsed.protocol = url.scheme().to_string();

    // 提取服务器和端口
    if let Some(host) = url.host_str() {
        parsed.server = host.to_string();
    }

    if let Some(port) = url.port() {
        parsed.port = port;
    }

    // 提取认证信息
    if !url.username().is_empty() {
        parsed.username = Some(url.username().to_string());
        if let Some(password) = url.password() {
            parsed.password = Some(password.to_string());
        }
    }

    // 解析查询参数
    let query_params: HashMap<_, _> = url.query_pairs().into_owned().collect();

    // 根据协议类型处理不同的参数
    match parsed.protocol.as_str() {
        "vmess" | "vless" => {
            // 从用户信息中提取UUID
            let userinfo = url.username();
            if !userinfo.is_empty() {
                parsed.uuid = Some(userinfo.to_string());
            }

            // 从查询参数中提取其他信息
            if let Some(security) = query_params.get("security") {
                parsed.cipher = Some(security.to_string());
            }

            if let Some(sni) = query_params.get("sni") {
                parsed.sni = Some(sni.to_string());
            }

            if let Some(type_) = query_params.get("type") {
                parsed.network = Some(type_.to_string());
            }

            if let Some(path) = query_params.get("path") {
                parsed.ws_path = Some(path.to_string());
            }

            // 检查是否启用TLS
            if let Some(security) = query_params.get("security") {
                parsed.tls = Some(security == "tls");
            }
        }

        "trojan" => {
            // Trojan协议使用密码作为认证
            if let Some(password) = url.password() {
                parsed.password = Some(password.to_string());
            }

            if let Some(sni) = query_params.get("sni") {
                parsed.sni = Some(sni.to_string());
            }

            parsed.tls = Some(true); // Trojan默认使用TLS
        }

        "ss" | "ssr" => {
            // Shadowsocks协议
            if let Some(method) = query_params.get("method") {
                parsed.cipher = Some(method.to_string());
            }

            if let Some(password) = url.password() {
                parsed.password = Some(password.to_string());
            }
        }

        "http" | "https" | "socks5" | "socks4" => {
            // HTTP/SOCKS代理
            let username = url.username();
            if !username.is_empty() {
                parsed.username = Some(username.to_string());
            }

            if let Some(password) = url.password() {
                parsed.password = Some(password.to_string());
            }
        }

        "hysteria" | "hysteria2" => {
            // Hysteria协议
            if let Some(auth) = query_params.get("auth") {
                parsed.password = Some(auth.to_string());
            }

            if let Some(sni) = query_params.get("sni") {
                parsed.sni = Some(sni.to_string());
            }

            if let Some(insecure) = query_params.get("insecure") {
                parsed.tls = Some(insecure != "1");
            } else {
                parsed.tls = Some(true);
            }
        }

        _ => {
            // 其他协议，保存所有查询参数
            parsed.extra_params = query_params;
        }
    }

    Ok(parsed)
}

/// 解析为IP:PORT格式
fn parse_as_ip_port(link: &str) -> Result<ParsedProxy> {
    // 匹配 IP:PORT 或 域名:PORT 格式
    let re = Regex::new(r"^([a-zA-Z0-9\.\-]+):(\d+)$")?;

    if let Some(caps) = re.captures(link) {
        let mut parsed = ParsedProxy::default();

        parsed.server = caps[1].to_string();
        parsed.port = caps[2].parse()?;
        parsed.protocol = "unknown".to_string();
        parsed.name = format!("{}:{}", parsed.server, parsed.port);

        return Ok(parsed);
    }

    Err(anyhow!("不是有效的IP:PORT格式"))
}

/// 解析base64编码的vmess链接
fn parse_base64_vmess(link: &str) -> Result<ParsedProxy> {
    use base64::Engine;
    use base64::engine::general_purpose;
    use serde_json::Value;

    // 检查是否是base64编码的vmess链接
    if !link.starts_with("vmess://") {
        return Err(anyhow!("不是vmess链接"));
    }

    // 移除协议头
    let base64_str = link.trim_start_matches("vmess://");

    // 解码base64
    let decoded = match general_purpose::STANDARD.decode(base64_str) {
        Ok(decoded) => String::from_utf8_lossy(&decoded).to_string(),
        Err(_) => return Err(anyhow!("base64解码失败")),
    };

    // 解析JSON
    let json: Value = match serde_json::from_str(&decoded) {
        Ok(json) => json,
        Err(_) => return Err(anyhow!("JSON解析失败")),
    };

    let mut parsed = ParsedProxy::default();
    parsed.protocol = "vmess".to_string();

    // 提取基本信息
    if let Some(ps) = json.get("ps").and_then(|v| v.as_str()) {
        parsed.name = ps.to_string();
    }

    if let Some(add) = json.get("add").and_then(|v| v.as_str()) {
        parsed.server = add.to_string();
    }

    if let Some(port) = json.get("port").and_then(|v| v.as_u64()) {
        parsed.port = port as u16;
    }

    if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
        parsed.uuid = Some(id.to_string());
    }

    if let Some(aid) = json.get("aid").and_then(|v| v.as_u64()) {
        parsed
            .extra_params
            .insert("alterId".to_string(), aid.to_string());
    }

    if let Some(net) = json.get("net").and_then(|v| v.as_str()) {
        parsed.network = Some(net.to_string());
    }

    if let Some(type_) = json.get("type").and_then(|v| v.as_str()) {
        parsed
            .extra_params
            .insert("type".to_string(), type_.to_string());
    }

    if let Some(host) = json.get("host").and_then(|v| v.as_str()) {
        parsed
            .extra_params
            .insert("host".to_string(), host.to_string());
    }

    if let Some(path) = json.get("path").and_then(|v| v.as_str()) {
        parsed.ws_path = Some(path.to_string());
    }

    if let Some(tls) = json.get("tls").and_then(|v| v.as_str()) {
        parsed.tls = Some(tls == "tls");
    }

    if let Some(sni) = json.get("sni").and_then(|v| v.as_str()) {
        parsed.sni = Some(sni.to_string());
    }

    Ok(parsed)
}

/// 从URL fragment中提取名称
fn extract_name_from_fragment(link: &str) -> Option<String> {
    if let Some(pos) = link.find('#') {
        let fragment = &link[pos + 1..];
        if !fragment.is_empty() {
            // 解码URL编码的名称
            match urlencoding::decode(fragment) {
                Ok(decoded) => return Some(decoded.to_string()),
                Err(_) => return Some(fragment.to_string()),
            }
        }
    }
    None
}

/// 生成默认名称
fn generate_default_name(parsed: &ParsedProxy) -> String {
    if !parsed.server.is_empty() && parsed.port > 0 {
        format!("{}:{}", parsed.server, parsed.port)
    } else if !parsed.protocol.is_empty() {
        format!("{}-proxy", parsed.protocol)
    } else {
        "unknown-proxy".to_string()
    }
}

/// 批量解析代理链接
pub fn parse_proxy_links(links: &[String]) -> Vec<Result<ParsedProxy>> {
    links.iter().map(|link| parse_proxy_link(link)).collect()
}

/// 将ParsedProxy转换为ProxyNode
pub fn to_proxy_node(parsed: ParsedProxy) -> crate::proxy::ProxyNode {
    use crate::proxy::ProxyNode;

    let mut node = ProxyNode {
        name: parsed.name,
        server: parsed.server,
        port: parsed.port,
        protocol: Some(parsed.protocol),
        username: parsed.username,
        password: parsed.password,
        uuid: parsed.uuid,
        cipher: parsed.cipher,
        tls: parsed.tls,
        sni: parsed.sni,
        network: parsed.network,
        ws_path: parsed.ws_path,
        ..Default::default()
    };

    // 设置alter_id（如果有）
    if let Some(alter_id) = parsed.extra_params.get("alterId") {
        if let Ok(alter_id) = alter_id.parse::<u16>() {
            node.alter_id = Some(alter_id);
        }
    }

    // 设置servername（与sni相同）
    if node.sni.is_some() && node.servername.is_none() {
        node.servername = node.sni.clone();
    }

    node
}
pub async fn parse_by_subsubconverter() -> Result<Vec<Proxy>, String> {
    let link = String::from(
        "https://198.16.63.200/xbsub?token=cb4ecb008923efba39841b2eba8b35ea
",
    );
    let mut all_nodes = Vec::new();
    let group_id = 0;
    let mut parse_settings = ParseSettings::default();

    add_nodes(
        link.to_string(),
        &mut all_nodes,
        group_id,
        &mut parse_settings,
    )
    .await?;
    Ok(all_nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_parse_by_subsubconverter() {
        let result = parse_by_subsubconverter().await;
        assert!(result.is_ok());
    }
    #[test]
    fn test_parse_vmess_link() {
        let link = "vmess://eyJ2IjoiMiIsInBzIjoi5paw5Yqg5Z2hIiwicG9ydCI6NDQzLCJpZCI6ImE1YTEzYjY1LTYyYjItNDUwOS04ZDU4LTUzYjE5Y2QxM2Q4YiIsImFpZCI6MCwic2N5IjoiYXV0byIsIm5ldCI6IndzIiwidHlwZSI6Im5vbmUiLCJob3N0IjoiIiwicGF0aCI6Ii8iLCJ0bHMiOiJ0bHMiLCJzbmkiOiIiLCJhbGwiOiJhbGwifQ==";

        let result = parse_proxy_link(link);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.protocol, "vmess");
        assert_eq!(parsed.name, "测试节点");
        assert_eq!(parsed.port, 443);
        assert!(parsed.uuid.is_some());
        assert_eq!(parsed.network, Some("ws".to_string()));
        assert_eq!(parsed.tls, Some(true));
    }

    #[test]
    fn test_parse_vless_link() {
        let link =
            "vless://uuid@server.com:443?security=tls&sni=example.com&type=ws&path=/path#节点名称";

        let result = parse_proxy_link(link);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.protocol, "vless");
        assert_eq!(parsed.name, "节点名称");
        assert_eq!(parsed.server, "server.com");
        assert_eq!(parsed.port, 443);
        assert_eq!(parsed.uuid, Some("uuid".to_string()));
        assert_eq!(parsed.tls, Some(true));
        assert_eq!(parsed.sni, Some("example.com".to_string()));
        assert_eq!(parsed.network, Some("ws".to_string()));
        assert_eq!(parsed.ws_path, Some("/path".to_string()));
    }

    #[test]
    fn test_parse_trojan_link() {
        let link = "trojan://password@server.com:443?sni=example.com#Trojan节点";

        let result = parse_proxy_link(link);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.protocol, "trojan");
        assert_eq!(parsed.name, "Trojan节点");
        assert_eq!(parsed.server, "server.com");
        assert_eq!(parsed.port, 443);
        assert_eq!(parsed.password, Some("password".to_string()));
        assert_eq!(parsed.tls, Some(true));
        assert_eq!(parsed.sni, Some("example.com".to_string()));
    }

    #[test]
    fn test_parse_ip_port() {
        let link = "192.168.1.1:1080";

        let result = parse_proxy_link(link);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.server, "192.168.1.1");
        assert_eq!(parsed.port, 1080);
        assert_eq!(parsed.protocol, "unknown");
        assert_eq!(parsed.name, "192.168.1.1:1080");
    }

    #[test]
    fn test_parse_http_proxy() {
        let link = "http://user:pass@proxy.com:8080#HTTP代理";

        let result = parse_proxy_link(link);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.protocol, "http");
        assert_eq!(parsed.name, "HTTP代理");
        assert_eq!(parsed.server, "proxy.com");
        assert_eq!(parsed.port, 8080);
        assert_eq!(parsed.username, Some("user".to_string()));
        assert_eq!(parsed.password, Some("pass".to_string()));
    }
}
