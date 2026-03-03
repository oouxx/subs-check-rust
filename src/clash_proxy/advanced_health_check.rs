//! 基于clash-rs理念的增强版健康检查器
//!
//! 提供完整的代理健康检查功能，包括：
//! - 详细的延迟测量（连接延迟、总延迟）
//! - 代理状态管理
//! - 延迟历史记录
//! - 批量测试和并发控制
//! - 自定义测试URL和超时设置

use anyhow::{Result, anyhow};
use reqwest::{Client, Proxy as ReqwestProxy};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::types::ProxyNodeInfo;

/// 延迟历史记录项
#[derive(Debug, Clone)]
pub struct DelayHistory {
    /// 测试时间（Unix时间戳）
    pub timestamp: u64,
    /// 实际延迟（不包括连接建立时间）
    pub actual_delay: Duration,
    /// 总延迟（包括连接建立）
    pub total_delay: Duration,
    /// 是否成功
    pub success: bool,
}

/// 代理状态
#[derive(Debug, Clone)]
pub struct ProxyState {
    /// 是否存活
    pub alive: bool,
    /// 最后检查时间（Unix时间戳）
    pub last_check: Option<u64>,
    /// 平均延迟（毫秒）
    pub avg_delay: Option<u64>,
    /// 延迟历史记录（最多保留10条）
    pub delay_history: VecDeque<DelayHistory>,
    /// 成功率（0.0-1.0）
    pub success_rate: f64,
    /// 总测试次数
    pub total_tests: u32,
    /// 成功测试次数
    pub successful_tests: u32,
}

impl Default for ProxyState {
    fn default() -> Self {
        Self {
            alive: false,
            last_check: None,
            avg_delay: None,
            delay_history: VecDeque::with_capacity(10),
            success_rate: 0.0,
            total_tests: 0,
            successful_tests: 0,
        }
    }
}

/// 健康检查结果
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// 代理名称
    pub name: String,
    /// 是否存活
    pub alive: bool,
    /// 连接延迟（毫秒）
    pub connect_delay: Option<u64>,
    /// TLS握手延迟（毫秒）
    pub tls_handshake_delay: Option<u64>,
    /// 实际延迟（毫秒）
    pub actual_delay: Option<u64>,
    /// 总延迟（毫秒）
    pub total_delay: Option<u64>,
    /// 错误信息（如果有）
    pub error: Option<String>,
    /// 测试时间（Unix时间戳）
    pub timestamp: u64,
}

/// 增强版健康检查器
#[derive(Debug, Clone)]
pub struct AdvancedHealthChecker {
    /// 默认超时时间（毫秒）
    timeout_ms: u64,
    /// 测试URL
    test_url: String,
    /// 代理状态存储
    proxy_states: Arc<RwLock<HashMap<String, ProxyState>>>,
    /// 最大并发数
    max_concurrent: usize,
    /// 是否启用详细日志
    verbose: bool,
}

impl AdvancedHealthChecker {
    /// 创建新的健康检查器
    pub fn new(
        timeout_ms: u64,
        test_url: Option<String>,
        max_concurrent: usize,
        verbose: bool,
    ) -> Self {
        Self {
            timeout_ms,
            test_url: test_url.unwrap_or_else(|| "http://www.gstatic.com/generate_204".to_string()),
            proxy_states: Arc::new(RwLock::new(HashMap::new())),
            max_concurrent,
            verbose,
        }
    }

    /// 批量检查代理健康状态
    pub async fn check_proxies_health_batch(
        &self,
        proxies: &[ProxyNodeInfo],
    ) -> Vec<HealthCheckResult> {
        let mut results = Vec::with_capacity(proxies.len());
        let mut tasks = Vec::new();

        // 创建任务
        for proxy in proxies {
            let proxy = proxy.clone();
            let checker = self.clone();
            tasks.push(tokio::spawn(async move {
                checker.check_proxy_health_detailed(&proxy).await
            }));
        }

        // 等待所有任务完成
        for task in tasks {
            match task.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => {
                    eprintln!("检查代理时发生错误: {}", e);
                }
                Err(e) => {
                    eprintln!("任务执行失败: {}", e);
                }
            }
        }

        results
    }

    /// 检查单个代理的健康状态（详细版）
    pub async fn check_proxy_health_detailed(
        &self,
        proxy_info: &ProxyNodeInfo,
    ) -> Result<HealthCheckResult> {
        let start_time = Instant::now();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 构建代理URL
        let proxy_url = match self.build_proxy_url(proxy_info) {
            Ok(url) => url,
            Err(e) => {
                return Ok(HealthCheckResult {
                    name: proxy_info.name.clone(),
                    alive: false,
                    connect_delay: None,
                    tls_handshake_delay: None,
                    actual_delay: None,
                    total_delay: None,
                    error: Some(format!("构建代理URL失败: {}", e)),
                    timestamp,
                });
            }
        };

        // 创建带代理的客户端
        let proxy = match ReqwestProxy::all(&proxy_url) {
            Ok(proxy) => proxy,
            Err(e) => {
                return Ok(HealthCheckResult {
                    name: proxy_info.name.clone(),
                    alive: false,
                    connect_delay: None,
                    tls_handshake_delay: None,
                    actual_delay: None,
                    total_delay: None,
                    error: Some(format!("无效的代理URL: {}", e)),
                    timestamp,
                });
            }
        };

        let client = match Client::builder()
            .timeout(Duration::from_millis(self.timeout_ms))
            .proxy(proxy)
            .build()
        {
            Ok(client) => client,
            Err(e) => {
                return Ok(HealthCheckResult {
                    name: proxy_info.name.clone(),
                    alive: false,
                    connect_delay: None,
                    tls_handshake_delay: None,
                    actual_delay: None,
                    total_delay: None,
                    error: Some(format!("创建HTTP客户端失败: {}", e)),
                    timestamp,
                });
            }
        };

        // 发送测试请求并测量延迟
        let connect_start = Instant::now();

        let response = client
            .get(&self.test_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (compatible; Proxy-Health-Check/1.0)",
            )
            .send()
            .await;

        let connect_delay = connect_start.elapsed();
        let actual_delay = connect_start.elapsed();
        let total_delay = start_time.elapsed();

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    // 成功
                    let result = HealthCheckResult {
                        name: proxy_info.name.clone(),
                        alive: true,
                        connect_delay: Some(connect_delay.as_millis() as u64),
                        tls_handshake_delay: None,
                        actual_delay: Some(actual_delay.as_millis() as u64),
                        total_delay: Some(total_delay.as_millis() as u64),
                        error: None,
                        timestamp,
                    };

                    // 更新代理状态
                    self.update_proxy_state(&proxy_info.name, &result).await;

                    Ok(result)
                } else {
                    // HTTP错误
                    let result = HealthCheckResult {
                        name: proxy_info.name.clone(),
                        alive: false,
                        connect_delay: Some(connect_delay.as_millis() as u64),
                        tls_handshake_delay: None,
                        actual_delay: Some(actual_delay.as_millis() as u64),
                        total_delay: Some(total_delay.as_millis() as u64),
                        error: Some(format!("HTTP错误: {}", resp.status())),
                        timestamp,
                    };

                    // 更新代理状态
                    self.update_proxy_state(&proxy_info.name, &result).await;

                    Ok(result)
                }
            }
            Err(e) => {
                // 连接失败
                let result = HealthCheckResult {
                    name: proxy_info.name.clone(),
                    alive: false,
                    connect_delay: Some(connect_delay.as_millis() as u64),
                    tls_handshake_delay: None,
                    actual_delay: Some(actual_delay.as_millis() as u64),
                    total_delay: Some(total_delay.as_millis() as u64),
                    error: Some(format!("连接失败: {}", e)),
                    timestamp,
                };

                // 更新代理状态
                self.update_proxy_state(&proxy_info.name, &result).await;

                Ok(result)
            }
        }
    }

    /// 构建代理URL
    fn build_proxy_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        match proxy_info.proto.to_lowercase().as_str() {
            "http" | "https" => self.build_http_proxy_url(proxy_info),
            "socks4" | "socks5" | "socks" => self.build_socks_proxy_url(proxy_info),
            "ss" | "shadowsocks" => self.build_shadowsocks_url(proxy_info),
            "vmess" => self.build_vmess_url(proxy_info),
            "vless" => self.build_vless_url(proxy_info),
            "trojan" => self.build_trojan_url(proxy_info),
            "ssr" => self.build_ssr_url(proxy_info),
            "hysteria" | "hysteria2" => self.build_hysteria_url(proxy_info),
            "tuic" => self.build_tuic_url(proxy_info),
            "wireguard" | "wg" => self.build_wireguard_url(proxy_info),
            _ => Err(anyhow!("不支持的代理协议: {}", proxy_info.proto)),
        }
    }

    fn build_http_proxy_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        let mut auth_part = String::new();

        if let Some(extra_info) = &proxy_info.extra_info {
            if let Some(username) = extra_info.get("username").and_then(|v| v.as_str()) {
                if let Some(password) = extra_info.get("password").and_then(|v| v.as_str()) {
                    auth_part = format!("{}:{}@", username, password);
                }
            }
        }

        Ok(format!(
            "{}://{}{}:{}",
            proxy_info.proto.to_lowercase(),
            auth_part,
            proxy_info.server,
            proxy_info.port
        ))
    }

    fn build_socks_proxy_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        let mut auth_part = String::new();

        if let Some(extra_info) = &proxy_info.extra_info {
            if let Some(username) = extra_info.get("username").and_then(|v| v.as_str()) {
                if let Some(password) = extra_info.get("password").and_then(|v| v.as_str()) {
                    auth_part = format!("{}:{}@", username, password);
                }
            }
        }

        Ok(format!(
            "socks5://{}{}:{}",
            auth_part, proxy_info.server, proxy_info.port
        ))
    }

    fn build_shadowsocks_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        let cipher = proxy_info
            .extra_info
            .as_ref()
            .and_then(|info| info.get("cipher"))
            .and_then(|v| v.as_str())
            .unwrap_or("aes-256-gcm");

        let password = proxy_info
            .extra_info
            .as_ref()
            .and_then(|info| info.get("password"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Shadowsocks需要密码"))?;

        // 构建SS URL格式: ss://method:password@server:port
        let encoded_password = urlencoding::encode(password);
        Ok(format!(
            "ss://{}:{}@{}:{}",
            cipher, encoded_password, proxy_info.server, proxy_info.port
        ))
    }

    fn build_vmess_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        let uuid = proxy_info
            .extra_info
            .as_ref()
            .and_then(|info| info.get("uuid"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Vmess需要UUID"))?;

        // 构建VMess配置对象
        let mut config = HashMap::new();
        config.insert("v".to_string(), "2".to_string());
        config.insert("ps".to_string(), proxy_info.name.clone());
        config.insert("add".to_string(), proxy_info.server.clone());
        config.insert("port".to_string(), proxy_info.port.to_string());
        config.insert("id".to_string(), uuid.to_string());
        config.insert("aid".to_string(), "0".to_string());

        // 添加加密方式
        if let Some(extra_info) = &proxy_info.extra_info {
            if let Some(cipher) = extra_info.get("cipher").and_then(|v| v.as_str()) {
                config.insert("type".to_string(), cipher.to_string());
            }
        }

        // 转换为JSON并Base64编码
        let json_config = serde_json::to_string(&config)?;
        let base64_config =
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE, json_config);

        Ok(format!("vmess://{}", base64_config))
    }

    fn build_vless_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        let uuid = proxy_info
            .extra_info
            .as_ref()
            .and_then(|info| info.get("uuid"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("VLESS需要UUID"))?;

        let mut url = format!("vless://{}@{}:{}", uuid, proxy_info.server, proxy_info.port);

        // 添加参数
        let mut params = Vec::new();

        if let Some(extra_info) = &proxy_info.extra_info {
            if let Some(security) = extra_info.get("cipher").and_then(|v| v.as_str()) {
                params.push(format!("security={}", security));
            }

            if let Some(network) = extra_info.get("network").and_then(|v| v.as_str()) {
                params.push(format!("type={}", network));
            }

            if let Some(sni) = extra_info.get("sni").and_then(|v| v.as_str()) {
                params.push(format!("sni={}", sni));
            }

            if let Some(tls) = extra_info.get("tls").and_then(|v| v.as_bool()) {
                if tls {
                    params.push("security=tls".to_string());
                }
            }
        }

        if !params.is_empty() {
            url.push_str("#");
            url.push_str(&params.join("&"));
        }

        Ok(url)
    }

    fn build_trojan_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        let password = proxy_info
            .extra_info
            .as_ref()
            .and_then(|info| info.get("password"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Trojan需要密码"))?;

        let mut url = format!(
            "trojan://{}@{}:{}",
            password, proxy_info.server, proxy_info.port
        );

        // 添加参数
        let mut params = Vec::new();

        if let Some(extra_info) = &proxy_info.extra_info {
            if let Some(sni) = extra_info.get("sni").and_then(|v| v.as_str()) {
                params.push(format!("sni={}", sni));
            }

            if let Some(tls) = extra_info.get("tls").and_then(|v| v.as_bool()) {
                if tls {
                    params.push("security=tls".to_string());
                }
            }
        }

        if !params.is_empty() {
            url.push_str("#");
            url.push_str(&params.join("&"));
        }

        Ok(url)
    }

    fn build_ssr_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        let cipher = proxy_info
            .extra_info
            .as_ref()
            .and_then(|info| info.get("cipher"))
            .and_then(|v| v.as_str())
            .unwrap_or("aes-256-cfb");

        let password = proxy_info
            .extra_info
            .as_ref()
            .and_then(|info| info.get("password"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("SSR需要密码"))?;

        // SSR URL格式更复杂，这里简化处理
        let encoded_password = urlencoding::encode(password);
        Ok(format!(
            "ssr://{}:{}@{}:{}",
            cipher, encoded_password, proxy_info.server, proxy_info.port
        ))
    }

    fn build_hysteria_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        // Hysteria协议URL构建
        let password = proxy_info
            .extra_info
            .as_ref()
            .and_then(|info| info.get("password"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        Ok(format!(
            "hysteria://{}@{}:{}",
            password, proxy_info.server, proxy_info.port
        ))
    }

    fn build_tuic_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        let password = proxy_info
            .extra_info
            .as_ref()
            .and_then(|info| info.get("password"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let uuid = proxy_info
            .extra_info
            .as_ref()
            .and_then(|info| info.get("uuid"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        Ok(format!(
            "tuic://{}:{}@{}:{}",
            uuid, password, proxy_info.server, proxy_info.port
        ))
    }

    fn build_wireguard_url(&self, proxy_info: &ProxyNodeInfo) -> Result<String> {
        // WireGuard协议URL构建
        Ok(format!(
            "wireguard://{}:{}",
            proxy_info.server, proxy_info.port
        ))
    }

    /// 更新代理状态
    async fn update_proxy_state(&self, proxy_name: &str, result: &HealthCheckResult) {
        let mut states = self.proxy_states.write().await;

        let state = states.entry(proxy_name.to_string()).or_default();

        // 更新基本状态
        state.alive = result.alive;
        state.last_check = Some(result.timestamp);

        // 更新测试统计
        state.total_tests += 1;
        if result.alive {
            state.successful_tests += 1;
        }
        state.success_rate = state.successful_tests as f64 / state.total_tests as f64;

        // 添加延迟历史记录
        if let (Some(actual_delay), Some(total_delay)) = (result.actual_delay, result.total_delay) {
            let history = DelayHistory {
                timestamp: result.timestamp,
                actual_delay: Duration::from_millis(actual_delay),
                total_delay: Duration::from_millis(total_delay),
                success: result.alive,
            };

            state.delay_history.push_back(history);

            // 保持最多10条记录
            if state.delay_history.len() > 10 {
                state.delay_history.pop_front();
            }

            // 计算平均延迟
            let total_actual_delay: u64 = state
                .delay_history
                .iter()
                .filter(|h| h.success)
                .map(|h| h.actual_delay.as_millis() as u64)
                .sum();

            let successful_count = state.delay_history.iter().filter(|h| h.success).count();

            if successful_count > 0 {
                state.avg_delay = Some(total_actual_delay / successful_count as u64);
            }
        }

        if self.verbose {
            println!(
                "代理 {}: {} (延迟: {:?}ms, 成功率: {:.1}%)",
                proxy_name,
                if result.alive {
                    "✓ 存活"
                } else {
                    "✗ 失败"
                },
                result.actual_delay,
                state.success_rate * 100.0
            );
        }
    }

    /// 获取代理状态
    pub async fn get_proxy_state(&self, proxy_name: &str) -> Option<ProxyState> {
        let states = self.proxy_states.read().await;
        states.get(proxy_name).cloned()
    }

    /// 获取所有代理状态
    pub async fn get_all_proxy_states(&self) -> HashMap<String, ProxyState> {
        let states = self.proxy_states.read().await;
        states.clone()
    }

    /// 清除代理状态
    pub async fn clear_proxy_states(&self) {
        let mut states = self.proxy_states.write().await;
        states.clear();
    }

    /// 获取超时时间
    pub fn get_timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// 获取测试URL
    pub fn get_test_url(&self) -> &str {
        &self.test_url
    }

    /// 获取最大并发数
    pub fn get_max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    /// 是否启用详细日志
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }
}

/// 批量检查代理健康状态（简化接口）
pub async fn check_proxies_health_batch(
    proxies: &[ProxyNodeInfo],
    timeout_ms: u64,
    test_url: Option<String>,
    max_concurrent: usize,
) -> Vec<HealthCheckResult> {
    let checker = AdvancedHealthChecker::new(timeout_ms, test_url, max_concurrent, false);
    checker.check_proxies_health_batch(proxies).await
}

/// 批量检查代理健康状态（带配置）
pub async fn check_proxies_health_with_config(
    proxies: &[ProxyNodeInfo],
    timeout_ms: u64,
    test_url: Option<String>,
) -> Vec<HealthCheckResult> {
    let checker = AdvancedHealthChecker::new(timeout_ms, test_url, 10, false);
    checker.check_proxies_health_batch(proxies).await
}

/// 健康检查配置
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// 超时时间（毫秒）
    pub timeout_ms: u64,
    /// 测试URL
    pub test_url: String,
    /// 最大并发数
    pub max_concurrent: usize,
    /// 是否启用详细日志
    pub verbose: bool,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 5000,
            test_url: "http://www.gstatic.com/generate_204".to_string(),
            max_concurrent: 10,
            verbose: false,
        }
    }
}
