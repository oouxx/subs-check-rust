//! 代理健康检查模块
//! 参考 clash 的实现逻辑，提供代理连接测试和延迟检测功能

use anyhow::{Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::{Client, Proxy as ReqwestProxy};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

use super::types::ProxyNodeInfo;

/// 代理健康检查器
#[derive(Debug, Clone)]
pub struct ProxyHealthChecker {
    /// 测试超时时间（毫秒）
    timeout_ms: u64,
    /// 测试目标 URL
    test_url: String,
}

impl ProxyHealthChecker {
    /// 创建新的健康检查器
    pub fn new(timeout_ms: u64, test_url: Option<String>) -> Self {
        Self {
            timeout_ms,
            test_url: test_url.unwrap_or_else(|| "http://www.gstatic.com/generate_204".to_string()),
        }
    }

    /// 检查单个代理节点的健康状况
    pub async fn check_proxy_health(&self, proxy_info: &ProxyNodeInfo) -> Result<u64> {
        // 构建代理 URL
        let proxy_url = self.build_proxy_url(proxy_info)?;

        // 创建带代理的客户端
        let proxy = ReqwestProxy::all(&proxy_url)
            .map_err(|e| anyhow!("无效的代理URL: {}: {}", proxy_url, e))?;

        let client = Client::builder()
            .timeout(Duration::from_millis(self.timeout_ms))
            .proxy(proxy)
            .build()
            .map_err(|e| anyhow!("创建代理客户端失败: {}", e))?;

        // 发送测试请求并测量延迟
        let start = Instant::now();

        let response = client
            .get(&self.test_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (compatible; Proxy-Health-Check/1.0)",
            )
            .send()
            .await;

        let elapsed = start.elapsed();

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    Ok(elapsed.as_millis() as u64)
                } else {
                    Err(anyhow!("代理返回错误状态码: {}", resp.status()))
                }
            }
            Err(e) => Err(anyhow!("代理连接失败: {}", e)),
        }
    }

    /// 构建代理 URL
    fn build_proxy_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        match proxy_info.proto.as_str() {
            "http" | "https" => self.build_http_proxy_url(proxy_info),
            "socks4" | "socks5" => self.build_socks_proxy_url(proxy_info),
            "ss" => self.build_shadowsocks_url(proxy_info),
            "vmess" => self.build_vmess_url(proxy_info),
            "vless" => self.build_vless_url(proxy_info),
            "trojan" => self.build_trojan_url(proxy_info),
            "ssr" => self.build_ssr_url(proxy_info),
            "hysteria" => self.build_hysteria_url(proxy_info),
            "tuic" => self.build_tuic_url(proxy_info),
            "wireguard" => self.build_wireguard_url(proxy_info),
            _ => Err(anyhow!("不支持的代理协议: {}", proxy_info.proto)),
        }
    }

    /// 构建 HTTP/HTTPS 代理 URL
    fn build_http_proxy_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        let mut url = format!("{}://", proxy_info.proto);

        if let Some(extra_info) = &proxy_info.extra_info {
            if let Some(username) = extra_info.get("username").and_then(|v| v.as_str()) {
                if let Some(password) = extra_info.get("password").and_then(|v| v.as_str()) {
                    url.push_str(&format!("{}:{}@", username, password));
                }
            }
        }

        url.push_str(&format!("{}:{}", proxy_info.server, proxy_info.port));
        Ok(url)
    }

    /// 构建 SOCKS 代理 URL
    fn build_socks_proxy_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        let mut url = format!("{}://", proxy_info.proto);

        if let Some(extra_info) = &proxy_info.extra_info {
            if let Some(username) = extra_info.get("username").and_then(|v| v.as_str()) {
                if let Some(password) = extra_info.get("password").and_then(|v| v.as_str()) {
                    url.push_str(&format!("{}:{}@", username, password));
                }
            }
        }

        url.push_str(&format!("{}:{}", proxy_info.server, proxy_info.port));
        Ok(url)
    }

    /// 构建 Shadowsocks 代理 URL
    fn build_shadowsocks_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        if let Some(extra_info) = &proxy_info.extra_info {
            let password = extra_info
                .get("password")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Shadowsocks 代理缺少密码"))?;

            let method = extra_info
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("aes-256-gcm");

            let mut url = format!(
                "ss://{}:{}@{}:{}",
                method, password, proxy_info.server, proxy_info.port
            );

            // 添加插件参数
            if let Some(plugin) = extra_info.get("plugin").and_then(|v| v.as_str()) {
                let mut plugin_opts = String::new();
                if let Some(opts) = extra_info.get("plugin-opts").and_then(|v| v.as_str()) {
                    plugin_opts = opts.to_string();
                }

                if !plugin_opts.is_empty() {
                    url.push_str(&format!(
                        "/?plugin={}%3B{}",
                        urlencoding::encode(plugin),
                        urlencoding::encode(&plugin_opts)
                    ));
                } else {
                    url.push_str(&format!("/?plugin={}", urlencoding::encode(plugin)));
                }
            }

            Ok(url)
        } else {
            Err(anyhow!("Shadowsocks 代理缺少额外信息"))
        }
    }

    /// 构建 VMess 代理 URL
    fn build_vmess_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        if let Some(extra_info) = &proxy_info.extra_info {
            let uuid = extra_info
                .get("uuid")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("VMess 代理缺少 UUID"))?;

            let alter_id = extra_info
                .get("alterId")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let security = extra_info
                .get("security")
                .and_then(|v| v.as_str())
                .unwrap_or("auto");

            let network = extra_info
                .get("network")
                .and_then(|v| v.as_str())
                .unwrap_or("tcp");

            // 构建 VMess 配置
            let config = json!({
                "v": "2",
                "ps": proxy_info.name,
                "add": proxy_info.server,
                "port": proxy_info.port,
                "id": uuid,
                "aid": alter_id,
                "scy": security,
                "net": network,
                "type": extra_info.get("type").and_then(|v| v.as_str()).unwrap_or("none"),
                "host": extra_info.get("host").and_then(|v| v.as_str()).unwrap_or(""),
                "path": extra_info.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                "tls": extra_info.get("tls").and_then(|v| v.as_str()).unwrap_or(""),
                "sni": extra_info.get("sni").and_then(|v| v.as_str()).unwrap_or(""),
                "alpn": extra_info.get("alpn").and_then(|v| v.as_str()).unwrap_or(""),
            });

            let config_str = serde_json::to_string(&config)?;
            let encoded = STANDARD.encode(config_str);
            Ok(format!("vmess://{}", encoded))
        } else {
            Err(anyhow!("VMess 代理缺少额外信息"))
        }
    }

    /// 构建 VLESS 代理 URL
    fn build_vless_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        if let Some(extra_info) = &proxy_info.extra_info {
            let uuid = extra_info
                .get("uuid")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("VLESS 代理缺少 UUID"))?;

            let mut url = format!("vless://{}@{}:{}", uuid, proxy_info.server, proxy_info.port);

            // 添加参数
            let mut params = Vec::new();

            if let Some(security) = extra_info.get("security").and_then(|v| v.as_str()) {
                if !security.is_empty() {
                    params.push(format!("security={}", security));
                }
            }

            if let Some(network) = extra_info.get("network").and_then(|v| v.as_str()) {
                if !network.is_empty() {
                    params.push(format!("type={}", network));
                }
            }

            if let Some(host) = extra_info.get("host").and_then(|v| v.as_str()) {
                if !host.is_empty() {
                    params.push(format!("host={}", host));
                }
            }

            if let Some(path) = extra_info.get("path").and_then(|v| v.as_str()) {
                if !path.is_empty() {
                    params.push(format!("path={}", path));
                }
            }

            if let Some(tls) = extra_info.get("tls").and_then(|v| v.as_str()) {
                if !tls.is_empty() {
                    params.push(format!("tls={}", tls));
                }
            }

            if let Some(sni) = extra_info.get("sni").and_then(|v| v.as_str()) {
                if !sni.is_empty() {
                    params.push(format!("sni={}", sni));
                }
            }

            if !params.is_empty() {
                url.push_str(&format!("#{}", params.join("&")));
            }

            Ok(url)
        } else {
            Err(anyhow!("VLESS 代理缺少额外信息"))
        }
    }

    /// 构建 Trojan 代理 URL
    fn build_trojan_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        if let Some(extra_info) = &proxy_info.extra_info {
            let password = extra_info
                .get("password")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Trojan 代理缺少密码"))?;

            let mut url = format!(
                "trojan://{}@{}:{}",
                password, proxy_info.server, proxy_info.port
            );

            // 添加参数
            let mut params = Vec::new();

            if let Some(security) = extra_info.get("security").and_then(|v| v.as_str()) {
                if !security.is_empty() {
                    params.push(format!("security={}", security));
                }
            }

            if let Some(host) = extra_info.get("host").and_then(|v| v.as_str()) {
                if !host.is_empty() {
                    params.push(format!("host={}", host));
                }
            }

            if let Some(path) = extra_info.get("path").and_then(|v| v.as_str()) {
                if !path.is_empty() {
                    params.push(format!("path={}", path));
                }
            }

            if let Some(tls) = extra_info.get("tls").and_then(|v| v.as_str()) {
                if !tls.is_empty() {
                    params.push(format!("tls={}", tls));
                }
            }

            if let Some(sni) = extra_info.get("sni").and_then(|v| v.as_str()) {
                if !sni.is_empty() {
                    params.push(format!("sni={}", sni));
                }
            }

            if !params.is_empty() {
                url.push_str(&format!("#{}", params.join("&")));
            }

            Ok(url)
        } else {
            Err(anyhow!("Trojan 代理缺少额外信息"))
        }
    }

    /// 构建 SSR 代理 URL
    fn build_ssr_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        if let Some(extra_info) = &proxy_info.extra_info {
            let password = extra_info
                .get("password")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("SSR 代理缺少密码"))?;

            let method = extra_info
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("aes-256-cfb");

            let protocol = extra_info
                .get("protocol")
                .and_then(|v| v.as_str())
                .unwrap_or("origin");

            let obfs = extra_info
                .get("obfs")
                .and_then(|v| v.as_str())
                .unwrap_or("plain");

            let protocol_param = extra_info
                .get("protocol-param")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let obfs_param = extra_info
                .get("obfs-param")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // SSR URL 格式
            let password_base64 = STANDARD.encode(password);
            let base_str = format!(
                "{}:{}:{}:{}:{}:{}",
                proxy_info.server, proxy_info.port, protocol, method, obfs, password_base64
            );

            let mut params = Vec::new();
            if !obfs_param.is_empty() {
                let obfs_param_base64 = STANDARD.encode(obfs_param);
                params.push(format!("obfsparam={}", obfs_param_base64));
            }
            if !protocol_param.is_empty() {
                let protocol_param_base64 = STANDARD.encode(protocol_param);
                params.push(format!("protoparam={}", protocol_param_base64));
            }

            let full_str = if !params.is_empty() {
                format!("{}/?{}", base_str, params.join("&"))
            } else {
                base_str
            };

            let encoded = STANDARD.encode(&full_str);
            Ok(format!("ssr://{}", encoded))
        } else {
            Err(anyhow!("SSR 代理缺少额外信息"))
        }
    }

    /// 构建 Hysteria 代理 URL
    fn build_hysteria_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        Ok(format!(
            "hysteria://{}:{}#{}",
            proxy_info.server, proxy_info.port, proxy_info.name
        ))
    }

    /// 构建 Tuic 代理 URL
    fn build_tuic_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        if let Some(extra_info) = &proxy_info.extra_info {
            let password = extra_info
                .get("password")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Tuic 代理缺少密码"))?;

            let mut url = format!(
                "tuic://{}@{}:{}",
                password, proxy_info.server, proxy_info.port
            );

            // 添加参数
            let mut params = Vec::new();

            if let Some(uuid) = extra_info.get("uuid").and_then(|v| v.as_str()) {
                if !uuid.is_empty() {
                    params.push(format!("uuid={}", uuid));
                }
            }

            if let Some(host) = extra_info.get("host").and_then(|v| v.as_str()) {
                if !host.is_empty() {
                    params.push(format!("host={}", host));
                }
            }

            if let Some(path) = extra_info.get("path").and_then(|v| v.as_str()) {
                if !path.is_empty() {
                    params.push(format!("path={}", path));
                }
            }

            if !params.is_empty() {
                url.push_str(&format!("#{}", params.join("&")));
            }

            Ok(url)
        } else {
            Err(anyhow!("Tuic 代理缺少额外信息"))
        }
    }

    /// 构建 WireGuard 代理 URL
    fn build_wireguard_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        Ok(format!(
            "wireguard://{}:{}#{}",
            proxy_info.server, proxy_info.port, proxy_info.name
        ))
    }

    /// 批量检查代理节点健康状况
    pub async fn check_proxies_health(&self, proxies: &[ProxyNodeInfo]) -> Vec<ProxyNodeInfo> {
        let mut results = Vec::new();

        for proxy in proxies {
            match self.check_proxy_health(proxy).await {
                Ok(delay_ms) => {
                    let mut result = proxy.clone();
                    result.delay_ms = delay_ms;
                    results.push(result);
                }
                Err(_) => {
                    // 检查失败，保持原始节点信息
                    results.push(proxy.clone());
                }
            }
        }

        results
    }

    /// 检查代理是否支持 UDP
    pub async fn check_udp_support(&self, proxy_info: &ProxyNodeInfo) -> Result<bool> {
        // 根据协议类型判断是否支持 UDP
        match proxy_info.proto.as_str() {
            "socks5" => Ok(true),
            "ss" | "ssr" => {
                if let Some(extra_info) = &proxy_info.extra_info {
                    if let Some(udp) = extra_info.get("udp").and_then(|v| v.as_bool()) {
                        Ok(udp)
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }

    /// 获取测试超时时间
    pub fn get_timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// 获取测试 URL
    pub fn get_test_url(&self) -> &str {
        &self.test_url
    }
}

/// 简化的代理健康检查（兼容旧接口）
pub async fn check_proxies_health(proxies: &[ProxyNodeInfo]) -> Vec<ProxyNodeInfo> {
    let checker = ProxyHealthChecker::new(5000, None);
    checker.check_proxies_health(proxies).await
}

/// 带自定义配置的代理健康检查
pub async fn check_proxies_health_with_config(
    proxies: &[ProxyNodeInfo],
    timeout_ms: u64,
    test_url: Option<String>,
) -> Vec<ProxyNodeInfo> {
    let checker = ProxyHealthChecker::new(timeout_ms, test_url);
    checker.check_proxies_health(proxies).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_http_proxy_info() -> ProxyNodeInfo {
        ProxyNodeInfo::new(
            "测试HTTP节点".to_string(),
            "http".to_string(),
            "example.com".to_string(),
            8080,
        )
        .with_extra_info(json!({
            "username": "test_user",
            "password": "test_pass"
        }))
    }

    fn create_test_shadowsocks_proxy_info() -> ProxyNodeInfo {
        ProxyNodeInfo::new(
            "测试SS节点".to_string(),
            "ss".to_string(),
            "ss.example.com".to_string(),
            8388,
        )
        .with_extra_info(json!({
            "password": "secret_password",
            "method": "aes-256-gcm",
            "plugin": "obfs-local",
            "plugin-opts": "obfs=http;obfs-host=cloudflare.com"
        }))
    }

    fn create_test_vmess_proxy_info() -> ProxyNodeInfo {
        ProxyNodeInfo::new(
            "测试VMess节点".to_string(),
            "vmess".to_string(),
            "vmess.example.com".to_string(),
            443,
        )
        .with_extra_info(json!({
            "uuid": "12345678-1234-1234-1234-123456789012",
            "alterId": 0,
            "security": "auto",
            "network": "tcp",
            "type": "none",
            "host": "",
            "path": "",
            "tls": "tls",
            "sni": "vmess.example.com"
        }))
    }

    #[tokio::test]
    async fn test_build_http_proxy_url() {
        let checker = ProxyHealthChecker::new(5000, None);
        let proxy_info = create_test_http_proxy_info();

        let url = checker.build_proxy_url(&proxy_info);
        assert!(url.is_ok());
        assert_eq!(url.unwrap(), "http://test_user:test_pass@example.com:8080");
    }

    #[tokio::test]
    async fn test_build_shadowsocks_url() {
        let checker = ProxyHealthChecker::new(5000, None);
        let proxy_info = create_test_shadowsocks_proxy_info();

        let url = checker.build_proxy_url(&proxy_info);
        assert!(url.is_ok());
        let url_str = url.unwrap();
        assert!(url_str.starts_with("ss://aes-256-gcm:secret_password@ss.example.com:8388"));
        assert!(url_str.contains("plugin=obfs-local"));
        assert!(url_str.contains("obfs%3Dhttp"));
        assert!(url_str.contains("obfs-host%3Dcloudflare.com"));
    }

    #[tokio::test]
    async fn test_build_vmess_url() {
        let checker = ProxyHealthChecker::new(5000, None);
        let proxy_info = create_test_vmess_proxy_info();

        let url = checker.build_proxy_url(&proxy_info);
        assert!(url.is_ok());
        let url_str = url.unwrap();
        assert!(url_str.starts_with("vmess://"));

        // 解码 base64 验证内容
        let base64_str = url_str.trim_start_matches("vmess://");
        let decoded = STANDARD.decode(base64_str);
        assert!(decoded.is_ok());

        let config_str = String::from_utf8(decoded.unwrap());
        assert!(config_str.is_ok());
        let config: Value = serde_json::from_str(&config_str.unwrap()).unwrap();

        assert_eq!(config["v"], "2");
        assert_eq!(config["add"], "vmess.example.com");
        assert_eq!(config["port"], 443);
        assert_eq!(config["id"], "12345678-1234-1234-1234-123456789012");
        assert_eq!(config["tls"], "tls");
    }

    #[tokio::test]
    async fn test_health_checker_creation() {
        let checker = ProxyHealthChecker::new(5000, None);
        assert_eq!(checker.get_timeout_ms(), 5000);
        assert_eq!(
            checker.get_test_url(),
            "http://www.gstatic.com/generate_204"
        );
    }

    #[tokio::test]
    async fn test_check_udp_support() {
        let checker = ProxyHealthChecker::new(5000, None);

        // 测试 SOCKS5 协议（通常支持 UDP）
        let socks5_proxy = ProxyNodeInfo::new(
            "SOCKS5节点".to_string(),
            "socks5".to_string(),
            "socks.example.com".to_string(),
            1080,
        );
        let result = checker.check_udp_support(&socks5_proxy).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);

        // 测试 HTTP 协议（默认不支持 UDP）
        let http_proxy = create_test_http_proxy_info();
        let result = checker.check_udp_support(&http_proxy).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);

        // 测试带 UDP 配置的 Shadowsocks
        let mut ss_proxy = create_test_shadowsocks_proxy_info();
        ss_proxy.extra_info = Some(json!({
            "password": "secret",
            "method": "aes-256-gcm",
            "udp": true
        }));
        let result = checker.check_udp_support(&ss_proxy).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_unsupported_protocol() {
        let checker = ProxyHealthChecker::new(5000, None);
        let proxy_info = ProxyNodeInfo::new(
            "未知协议节点".to_string(),
            "unknown".to_string(),
            "example.com".to_string(),
            8080,
        );

        let url = checker.build_proxy_url(&proxy_info);
        assert!(url.is_err());
        assert!(url.unwrap_err().to_string().contains("不支持的代理协议"));
    }
}
