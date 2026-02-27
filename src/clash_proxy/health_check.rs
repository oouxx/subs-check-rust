//! 基于 clash-lib 的代理健康检查模块
//! 提供代理连接测试和延迟检测功能

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

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

        // 创建 HTTP 客户端
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(self.timeout_ms))
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        // 设置代理
        let proxy = reqwest::Proxy::all(&proxy_url)
            .map_err(|e| anyhow!("Failed to create proxy: {}", e))?;

        let client_with_proxy = reqwest::Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_millis(self.timeout_ms))
            .build()
            .map_err(|e| anyhow!("Failed to create proxied HTTP client: {}", e))?;

        // 发送测试请求并测量延迟
        let start = Instant::now();
        let response = client_with_proxy.get(&self.test_url).send().await;
        let elapsed = start.elapsed();

        match response {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 204 => {
                Ok(elapsed.as_millis() as u64)
            }
            Ok(resp) => Err(anyhow!(
                "Test request failed with status: {}",
                resp.status()
            )),
            Err(e) => Err(anyhow!("Test request failed: {}", e)),
        }
    }

    /// 构建代理 URL
    fn build_proxy_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        let proto = proxy_info.proto.to_lowercase();

        // 处理不同的代理协议
        let url = match proto.as_str() {
            "http" | "https" | "socks5" | "socks4" | "socks4a" => {
                // 标准代理协议
                let mut url = format!("{}://{}:{}", proto, proxy_info.server, proxy_info.port);

                // 添加认证信息（如果有）
                if let Some(extra_info) = &proxy_info.extra_info {
                    if let Some(username) = extra_info.get("username").and_then(|v| v.as_str()) {
                        if let Some(password) = extra_info.get("password").and_then(|v| v.as_str())
                        {
                            url = format!(
                                "{}://{}:{}@{}:{}",
                                proto, username, password, proxy_info.server, proxy_info.port
                            );
                        }
                    }
                }
                url
            }
            "ss" | "shadowsocks" => {
                // Shadowsocks 协议
                if let Some(extra_info) = &proxy_info.extra_info {
                    if let Some(password) = extra_info.get("password").and_then(|v| v.as_str()) {
                        let method = extra_info
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("aes-256-gcm");

                        format!(
                            "ss://{}:{}@{}:{}",
                            method, password, proxy_info.server, proxy_info.port
                        )
                    } else {
                        return Err(anyhow!("Missing password for Shadowsocks proxy"));
                    }
                } else {
                    return Err(anyhow!("Missing extra info for Shadowsocks proxy"));
                }
            }
            _ => {
                // 其他协议使用标准格式
                format!("{}://{}:{}", proto, proxy_info.server, proxy_info.port)
            }
        };

        Ok(url)
    }

    /// 批量检查代理节点健康状态
    pub async fn check_proxies_health(&self, proxies: &[ProxyNodeInfo]) -> Vec<ProxyNodeInfo> {
        let mut results = Vec::new();

        // 并发检查代理
        let mut tasks = Vec::new();

        for proxy in proxies {
            let proxy_clone = proxy.clone();
            let checker_clone = self.clone();

            let task = tokio::spawn(async move {
                match checker_clone.check_proxy_health(&proxy_clone).await {
                    Ok(delay_ms) => {
                        let mut result = proxy_clone.clone();
                        result.delay_ms = delay_ms;
                        result
                    }
                    Err(e) => {
                        eprintln!("Failed to check proxy {}: {}", proxy_clone.name, e);
                        let mut result = proxy_clone.clone();
                        result.delay_ms = 0;
                        result
                    }
                }
            });

            tasks.push(task);
        }

        // 等待所有任务完成并收集结果
        for task in tasks {
            match task.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    eprintln!("Task failed: {}", e);
                }
            }
        }

        results
    }

    /// 检查代理节点是否支持 UDP
    pub async fn check_udp_support(&self, proxy_info: &ProxyNodeInfo) -> Result<bool> {
        // 对于大多数代理协议，UDP 支持取决于配置
        // 这里返回代理信息中已有的设置
        Ok(proxy_info.support_udp)
    }

    /// 获取测试超时时间
    pub fn get_timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// 设置测试超时时间
    pub fn set_timeout_ms(&mut self, timeout_ms: u64) {
        self.timeout_ms = timeout_ms;
    }

    /// 获取测试 URL
    pub fn get_test_url(&self) -> &str {
        &self.test_url
    }

    /// 设置测试 URL
    pub fn set_test_url(&mut self, test_url: String) {
        self.test_url = test_url;
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

    fn create_test_proxy_info() -> ProxyNodeInfo {
        ProxyNodeInfo::new(
            "测试节点".to_string(),
            "http".to_string(),
            "example.com".to_string(),
            8080,
        )
        .with_extra_info(json!({
            "username": "test_user",
            "password": "test_pass"
        }))
    }

    #[tokio::test]
    async fn test_build_proxy_url() {
        let checker = ProxyHealthChecker::new(5000, None);
        let proxy_info = create_test_proxy_info();

        let url = checker.build_proxy_url(&proxy_info);
        assert!(url.is_ok());
        assert_eq!(url.unwrap(), "http://test_user:test_pass@example.com:8080");
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
        let mut proxy_info = create_test_proxy_info();

        // 测试支持 UDP 的情况
        proxy_info.support_udp = true;
        let result = checker.check_udp_support(&proxy_info).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);

        // 测试不支持 UDP 的情况
        proxy_info.support_udp = false;
        let result = checker.check_udp_support(&proxy_info).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
    }

    #[test]
    fn test_shadowsocks_url_building() {
        let checker = ProxyHealthChecker::new(5000, None);
        let proxy_info = ProxyNodeInfo::new(
            "SS节点".to_string(),
            "ss".to_string(),
            "ss.example.com".to_string(),
            8388,
        )
        .with_extra_info(json!({
            "password": "secret_password",
            "method": "aes-256-gcm"
        }));

        let url = checker.build_proxy_url(&proxy_info);
        assert!(url.is_ok());
        assert_eq!(
            url.unwrap(),
            "ss://aes-256-gcm:secret_password@ss.example.com:8388"
        );
    }
}
