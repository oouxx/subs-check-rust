//! 基于clash-rs理念的代理配置解析器
//!
//! 提供完整的代理配置解析功能，支持：
//! - Clash YAML配置文件解析
//! - 多种代理协议配置
//! - 代理组配置
//! - 健康检查配置
//! - 订阅链接解析

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_yaml::{self, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use url::Url;

use super::types::ProxyNodeInfo;

/// Clash配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClashConfig {
    /// 代理列表
    pub proxies: Vec<ProxyNodeInfo>,
    /// 代理组
    pub proxy_groups: Option<Vec<ProxyGroup>>,
    /// DNS配置
    pub dns: Option<DnsConfig>,
    /// 规则列表
    pub rules: Option<Vec<String>>,
    /// 端口设置
    pub port: Option<u16>,
    /// 混合端口设置
    pub mixed_port: Option<u16>,
    /// 允许局域网连接
    pub allow_lan: Option<bool>,
    /// 日志级别
    pub log_level: Option<String>,
    /// 外部控制器
    pub external_controller: Option<String>,
    /// 外部UI
    pub external_ui: Option<String>,
    /// 模式
    pub mode: Option<String>,
    /// 健康检查配置
    pub health_check: Option<HealthCheckConfig>,
}

/// DNS配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    /// 是否启用
    pub enable: Option<bool>,
    /// 监听地址
    pub listen: Option<String>,
    /// 默认DNS服务器
    pub default_nameserver: Option<Vec<String>>,
    /// 增强模式
    pub enhanced_mode: Option<String>,
    /// 使用主机名
    pub use_hosts: Option<bool>,
    /// 域名服务器
    pub nameserver: Option<Vec<String>>,
    /// Fallback服务器
    pub fallback: Option<Vec<String>>,
    /// Fallback过滤器
    pub fallback_filter: Option<DnsFallbackFilter>,
}

/// DNS Fallback过滤器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsFallbackFilter {
    /// 地理位置数据库
    pub geoip: Option<bool>,
    /// IP CIDR
    pub ipcidr: Option<Vec<String>>,
    /// 域名
    pub domain: Option<Vec<String>>,
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// 是否启用
    pub enable: Option<bool>,
    /// 检查间隔（秒）
    pub interval: Option<u64>,
    /// 超时时间（秒）
    pub timeout: Option<u64>,
    /// 延迟阈值（毫秒）
    pub delay_threshold: Option<u64>,
    /// 测试URL
    pub url: Option<String>,
    /// 懒检查模式
    pub lazy: Option<bool>,
}

/// 代理组类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyGroupType {
    /// 选择器
    Select,
    /// URL测试
    UrlTest,
    /// 故障转移
    Fallback,
    /// 负载均衡
    LoadBalance,
    /// 延迟测试
    DelayTest,
    /// 手动选择
    Manual,
}

/// 代理组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyGroup {
    /// 组名称
    pub name: String,
    /// 组类型
    pub r#type: ProxyGroupType,
    /// 代理列表
    pub proxies: Vec<String>,
    /// URL（用于URL测试）
    pub url: Option<String>,
    /// 间隔时间（秒）
    pub interval: Option<u64>,
    /// 延迟容差（毫秒）
    pub tolerance: Option<u64>,
    /// 使用的代理提供者
    pub r#use: Option<Vec<String>>,
}

/// 代理提供者
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyProvider {
    /// 提供者类型
    pub r#type: String,
    /// 订阅URL
    pub url: String,
    /// 更新间隔（秒）
    pub interval: Option<u64>,
    /// 健康检查配置
    pub health_check: Option<HealthCheckConfig>,
    /// 过滤器
    pub filter: Option<String>,
    /// 排除过滤器
    pub exclude_filter: Option<String>,
}

/// 配置解析器
#[derive(Debug, Clone)]
pub struct ConfigParser {
    /// 配置文件路径
    config_path: String,
    /// 解析后的配置
    config: Option<ClashConfig>,
}

impl ConfigParser {
    /// 创建新的配置解析器
    pub fn new(config_path: &str) -> Self {
        Self {
            config_path: config_path.to_string(),
            config: None,
        }
    }

    /// 从文件加载配置
    pub fn load_from_file(&mut self) -> Result<()> {
        let path = Path::new(&self.config_path);
        if !path.exists() {
            return Err(anyhow!("配置文件不存在: {}", self.config_path));
        }

        let content = fs::read_to_string(path)?;
        self.parse_config(&content)
    }

    /// 解析配置内容
    pub fn parse_config(&mut self, content: &str) -> Result<()> {
        let yaml: Value = serde_yaml::from_str(content)?;
        self.config = Some(Self::parse_yaml_config(&yaml)?);
        Ok(())
    }

    /// 解析YAML配置
    fn parse_yaml_config(yaml: &Value) -> Result<ClashConfig> {
        let mut proxies = Vec::new();

        // 解析代理列表
        if let Some(proxies_value) = yaml.get("proxies") {
            if let Value::Sequence(proxy_list) = proxies_value {
                for proxy_value in proxy_list {
                    if let Value::Mapping(proxy_map) = proxy_value {
                        let proxy_info = Self::parse_proxy_config(&proxy_map)?;
                        proxies.push(proxy_info);
                    }
                }
            }
        }

        // 解析代理组
        let proxy_groups = if let Some(groups_value) = yaml.get("proxy-groups") {
            if let Value::Sequence(group_list) = groups_value {
                let mut groups = Vec::new();
                for group_value in group_list {
                    if let Value::Mapping(group_map) = group_value {
                        let group = Self::parse_proxy_group(&group_map)?;
                        groups.push(group);
                    }
                }
                Some(groups)
            } else {
                None
            }
        } else {
            None
        };

        // 解析DNS配置
        let dns = if let Some(dns_value) = yaml.get("dns") {
            if let Value::Mapping(dns_map) = dns_value {
                Some(Self::parse_dns_config(&dns_map)?)
            } else {
                None
            }
        } else {
            None
        };

        // 解析规则
        let rules = if let Some(rules_value) = yaml.get("rules") {
            if let Value::Sequence(rule_list) = rules_value {
                let mut rules = Vec::new();
                for rule_value in rule_list {
                    if let Value::String(rule) = rule_value {
                        rules.push(rule.clone());
                    }
                }
                Some(rules)
            } else {
                None
            }
        } else {
            None
        };

        // 解析其他基本配置
        let port = yaml.get("port").and_then(|v| v.as_u64()).map(|v| v as u16);
        let mixed_port = yaml
            .get("mixed-port")
            .and_then(|v| v.as_u64())
            .map(|v| v as u16);
        let allow_lan = yaml.get("allow-lan").and_then(|v| v.as_bool());
        let log_level = yaml
            .get("log-level")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let external_controller = yaml
            .get("external-controller")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let external_ui = yaml
            .get("external-ui")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let mode = yaml
            .get("mode")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // 解析健康检查配置
        let health_check = if let Some(hc_value) = yaml.get("health-check") {
            if let Value::Mapping(hc_map) = hc_value {
                Some(Self::parse_health_check_config(&hc_map)?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(ClashConfig {
            proxies,
            proxy_groups,
            dns,
            rules,
            port,
            mixed_port,
            allow_lan,
            log_level,
            external_controller,
            external_ui,
            mode,
            health_check,
        })
    }

    /// 解析单个代理配置
    fn parse_proxy_config(config: &serde_yaml::Mapping) -> Result<ProxyNodeInfo> {
        // 获取代理类型
        let proxy_type = config
            .get(&Value::String("type".to_string()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("代理配置缺少类型字段"))?
            .to_string();

        // 获取代理名称
        let name = config
            .get(&Value::String("name".to_string()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("代理配置缺少名称字段"))?
            .to_string();

        // 获取服务器地址和端口
        let server = config
            .get(&Value::String("server".to_string()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("代理配置缺少服务器字段"))?
            .to_string();

        let port = config
            .get(&Value::String("port".to_string()))
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow!("代理配置缺少端口字段"))? as u16;

        // 创建基础代理信息
        let mut proxy_info = ProxyNodeInfo::new(name, proxy_type.clone(), server, port);

        // 检查是否支持UDP
        if let Some(udp) = config.get(&Value::String("udp".to_string())) {
            if let Some(udp_bool) = udp.as_bool() {
                proxy_info = proxy_info.with_udp_support(udp_bool);
            }
        }

        // 根据代理类型解析特定字段到extra_info
        let mut extra_info = serde_json::Map::new();

        match proxy_type.as_str() {
            "ss" | "shadowsocks" => {
                if let Some(cipher) = config
                    .get(&Value::String("cipher".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "cipher".to_string(),
                        serde_json::Value::String(cipher.to_string()),
                    );
                }
                if let Some(password) = config
                    .get(&Value::String("password".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "password".to_string(),
                        serde_json::Value::String(password.to_string()),
                    );
                }
                if let Some(plugin) = config
                    .get(&Value::String("plugin".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "plugin".to_string(),
                        serde_json::Value::String(plugin.to_string()),
                    );
                }
            }

            "vmess" => {
                if let Some(uuid) = config
                    .get(&Value::String("uuid".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "uuid".to_string(),
                        serde_json::Value::String(uuid.to_string()),
                    );
                }
                if let Some(cipher) = config
                    .get(&Value::String("cipher".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "cipher".to_string(),
                        serde_json::Value::String(cipher.to_string()),
                    );
                }
                if let Some(tls) = config
                    .get(&Value::String("tls".to_string()))
                    .and_then(|v| v.as_bool())
                {
                    extra_info.insert("tls".to_string(), serde_json::Value::Bool(tls));
                }
                if let Some(sni) = config
                    .get(&Value::String("sni".to_string()))
                    .or_else(|| config.get(&Value::String("servername".to_string())))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "sni".to_string(),
                        serde_json::Value::String(sni.to_string()),
                    );
                }
                if let Some(network) = config
                    .get(&Value::String("network".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "network".to_string(),
                        serde_json::Value::String(network.to_string()),
                    );
                }
            }

            "vless" => {
                if let Some(uuid) = config
                    .get(&Value::String("uuid".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "uuid".to_string(),
                        serde_json::Value::String(uuid.to_string()),
                    );
                }
                if let Some(tls) = config
                    .get(&Value::String("tls".to_string()))
                    .and_then(|v| v.as_bool())
                {
                    extra_info.insert("tls".to_string(), serde_json::Value::Bool(tls));
                }
                if let Some(sni) = config
                    .get(&Value::String("sni".to_string()))
                    .or_else(|| config.get(&Value::String("servername".to_string())))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "sni".to_string(),
                        serde_json::Value::String(sni.to_string()),
                    );
                }
                if let Some(network) = config
                    .get(&Value::String("network".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "network".to_string(),
                        serde_json::Value::String(network.to_string()),
                    );
                }
            }

            "trojan" => {
                if let Some(password) = config
                    .get(&Value::String("password".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "password".to_string(),
                        serde_json::Value::String(password.to_string()),
                    );
                }
                if let Some(tls) = config
                    .get(&Value::String("tls".to_string()))
                    .and_then(|v| v.as_bool())
                {
                    extra_info.insert("tls".to_string(), serde_json::Value::Bool(tls));
                }
                if let Some(sni) = config
                    .get(&Value::String("sni".to_string()))
                    .or_else(|| config.get(&Value::String("servername".to_string())))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "sni".to_string(),
                        serde_json::Value::String(sni.to_string()),
                    );
                }
            }

            "http" | "https" => {
                if let Some(username) = config
                    .get(&Value::String("username".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "username".to_string(),
                        serde_json::Value::String(username.to_string()),
                    );
                }
                if let Some(password) = config
                    .get(&Value::String("password".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "password".to_string(),
                        serde_json::Value::String(password.to_string()),
                    );
                }
                extra_info.insert(
                    "tls".to_string(),
                    serde_json::Value::Bool(proxy_type == "https"),
                );
            }

            "socks5" => {
                if let Some(username) = config
                    .get(&Value::String("username".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "username".to_string(),
                        serde_json::Value::String(username.to_string()),
                    );
                }
                if let Some(password) = config
                    .get(&Value::String("password".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "password".to_string(),
                        serde_json::Value::String(password.to_string()),
                    );
                }
            }

            _ => {
                // 对于其他协议，尝试解析通用字段
                if let Some(password) = config
                    .get(&Value::String("password".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "password".to_string(),
                        serde_json::Value::String(password.to_string()),
                    );
                }
                if let Some(username) = config
                    .get(&Value::String("username".to_string()))
                    .and_then(|v| v.as_str())
                {
                    extra_info.insert(
                        "username".to_string(),
                        serde_json::Value::String(username.to_string()),
                    );
                }
                if let Some(tls) = config
                    .get(&Value::String("tls".to_string()))
                    .and_then(|v| v.as_bool())
                {
                    extra_info.insert("tls".to_string(), serde_json::Value::Bool(tls));
                }
            }
        }

        // 如果有额外信息，设置到proxy_info中
        if !extra_info.is_empty() {
            proxy_info = proxy_info.with_extra_info(serde_json::Value::Object(extra_info));
        }

        Ok(proxy_info)
    }

    /// 解析代理组配置
    fn parse_proxy_group(config: &serde_yaml::Mapping) -> Result<ProxyGroup> {
        let name = config
            .get(&Value::String("name".to_string()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("代理组配置缺少名称字段"))?
            .to_string();

        let type_str = config
            .get(&Value::String("type".to_string()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("代理组配置缺少类型字段"))?
            .to_string();

        let r#type = match type_str.to_lowercase().as_str() {
            "select" => ProxyGroupType::Select,
            "url-test" => ProxyGroupType::UrlTest,
            "fallback" => ProxyGroupType::Fallback,
            "load-balance" => ProxyGroupType::LoadBalance,
            "delay-test" => ProxyGroupType::DelayTest,
            "manual" => ProxyGroupType::Manual,
            _ => return Err(anyhow!("不支持的代理组类型: {}", type_str)),
        };

        let proxies = config
            .get(&Value::String("proxies".to_string()))
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        let url = config
            .get(&Value::String("url".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let interval = config
            .get(&Value::String("interval".to_string()))
            .and_then(|v| v.as_u64());

        let tolerance = config
            .get(&Value::String("tolerance".to_string()))
            .and_then(|v| v.as_u64());

        let r#use = config
            .get(&Value::String("use".to_string()))
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            });

        Ok(ProxyGroup {
            name,
            r#type,
            proxies,
            url,
            interval,
            tolerance,
            r#use,
        })
    }

    /// 解析DNS配置
    fn parse_dns_config(config: &serde_yaml::Mapping) -> Result<DnsConfig> {
        let enable = config
            .get(&Value::String("enable".to_string()))
            .and_then(|v| v.as_bool());

        let listen = config
            .get(&Value::String("listen".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let default_nameserver = config
            .get(&Value::String("default-nameserver".to_string()))
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            });

        let enhanced_mode = config
            .get(&Value::String("enhanced-mode".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let use_hosts = config
            .get(&Value::String("use-hosts".to_string()))
            .and_then(|v| v.as_bool());

        let nameserver = config
            .get(&Value::String("nameserver".to_string()))
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            });

        let fallback = config
            .get(&Value::String("fallback".to_string()))
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            });

        let fallback_filter = config
            .get(&Value::String("fallback-filter".to_string()))
            .and_then(|v| {
                if let Value::Mapping(filter_map) = v {
                    let geoip = filter_map
                        .get(&Value::String("geoip".to_string()))
                        .and_then(|v| v.as_bool());

                    let ipcidr = filter_map
                        .get(&Value::String("ipcidr".to_string()))
                        .and_then(|v| v.as_sequence())
                        .map(|seq| {
                            seq.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        });

                    let domain = filter_map
                        .get(&Value::String("domain".to_string()))
                        .and_then(|v| v.as_sequence())
                        .map(|seq| {
                            seq.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        });

                    Some(DnsFallbackFilter {
                        geoip,
                        ipcidr,
                        domain,
                    })
                } else {
                    None
                }
            });

        Ok(DnsConfig {
            enable,
            listen,
            default_nameserver,
            enhanced_mode,
            use_hosts,
            nameserver,
            fallback,
            fallback_filter,
        })
    }

    /// 解析健康检查配置
    fn parse_health_check_config(config: &serde_yaml::Mapping) -> Result<HealthCheckConfig> {
        let enable = config
            .get(&Value::String("enable".to_string()))
            .and_then(|v| v.as_bool());

        let interval = config
            .get(&Value::String("interval".to_string()))
            .and_then(|v| v.as_u64());

        let timeout = config
            .get(&Value::String("timeout".to_string()))
            .and_then(|v| v.as_u64());

        let delay_threshold = config
            .get(&Value::String("delay-threshold".to_string()))
            .and_then(|v| v.as_u64());

        let url = config
            .get(&Value::String("url".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let lazy = config
            .get(&Value::String("lazy".to_string()))
            .and_then(|v| v.as_bool());

        Ok(HealthCheckConfig {
            enable,
            interval,
            timeout,
            delay_threshold,
            url,
            lazy,
        })
    }

    /// 获取解析后的配置
    pub fn get_config(&self) -> Option<&ClashConfig> {
        self.config.as_ref()
    }

    /// 获取代理列表
    pub fn get_proxies(&self) -> Vec<ProxyNodeInfo> {
        self.config
            .as_ref()
            .map(|config| config.proxies.clone())
            .unwrap_or_default()
    }

    /// 获取代理组
    pub fn get_proxy_groups(&self) -> Vec<ProxyGroup> {
        self.config
            .as_ref()
            .and_then(|config| config.proxy_groups.clone())
            .unwrap_or_default()
    }

    /// 获取DNS配置
    pub fn get_dns_config(&self) -> Option<DnsConfig> {
        self.config.as_ref().and_then(|config| config.dns.clone())
    }

    /// 获取规则列表
    pub fn get_rules(&self) -> Vec<String> {
        self.config
            .as_ref()
            .and_then(|config| config.rules.clone())
            .unwrap_or_default()
    }

    /// 获取端口设置
    pub fn get_port(&self) -> Option<u16> {
        self.config.as_ref().and_then(|config| config.port)
    }

    /// 获取混合端口设置
    pub fn get_mixed_port(&self) -> Option<u16> {
        self.config.as_ref().and_then(|config| config.mixed_port)
    }

    /// 是否允许局域网连接
    pub fn allow_lan(&self) -> bool {
        self.config
            .as_ref()
            .and_then(|config| config.allow_lan)
            .unwrap_or(false)
    }

    /// 获取日志级别
    pub fn get_log_level(&self) -> String {
        self.config
            .as_ref()
            .and_then(|config| config.log_level.clone())
            .unwrap_or_else(|| "info".to_string())
    }

    /// 获取外部控制器地址
    pub fn get_external_controller(&self) -> Option<String> {
        self.config
            .as_ref()
            .and_then(|config| config.external_controller.clone())
    }

    /// 获取外部UI路径
    pub fn get_external_ui(&self) -> Option<String> {
        self.config
            .as_ref()
            .and_then(|config| config.external_ui.clone())
    }

    /// 获取运行模式
    pub fn get_mode(&self) -> String {
        self.config
            .as_ref()
            .and_then(|config| config.mode.clone())
            .unwrap_or_else(|| "rule".to_string())
    }

    /// 获取健康检查配置
    pub fn get_health_check_config(&self) -> Option<HealthCheckConfig> {
        self.config
            .as_ref()
            .and_then(|config| config.health_check.clone())
    }

    /// 生成Clash配置文件
    pub fn generate_config(&self, output_path: &str) -> Result<()> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| anyhow!("没有配置数据"))?;

        let mut yaml_config = serde_yaml::Mapping::new();

        // 添加代理列表
        if !config.proxies.is_empty() {
            let mut proxies_seq = serde_yaml::Sequence::new();
            for proxy in &config.proxies {
                let mut proxy_map = serde_yaml::Mapping::new();
                proxy_map.insert(
                    serde_yaml::Value::String("name".to_string()),
                    serde_yaml::Value::String(proxy.name.clone()),
                );
                proxy_map.insert(
                    serde_yaml::Value::String("type".to_string()),
                    serde_yaml::Value::String(proxy.proto.clone()),
                );
                proxy_map.insert(
                    serde_yaml::Value::String("server".to_string()),
                    serde_yaml::Value::String(proxy.server.clone()),
                );
                proxy_map.insert(
                    serde_yaml::Value::String("port".to_string()),
                    serde_yaml::Value::Number(serde_yaml::Number::from(proxy.port)),
                );

                if proxy.support_udp {
                    proxy_map.insert(
                        serde_yaml::Value::String("udp".to_string()),
                        serde_yaml::Value::Bool(true),
                    );
                }

                if let Some(extra_info) = &proxy.extra_info {
                    if let serde_json::Value::Object(obj) = extra_info {
                        for (key, value) in obj {
                            let yaml_value = match value {
                                serde_json::Value::String(s) => {
                                    serde_yaml::Value::String(s.clone())
                                }
                                serde_json::Value::Number(n) => {
                                    if let Some(i) = n.as_i64() {
                                        serde_yaml::Value::Number(serde_yaml::Number::from(i))
                                    } else if let Some(f) = n.as_f64() {
                                        serde_yaml::Value::Number(serde_yaml::Number::from(f))
                                    } else {
                                        continue;
                                    }
                                }
                                serde_json::Value::Bool(b) => serde_yaml::Value::Bool(*b),
                                _ => continue,
                            };
                            proxy_map.insert(serde_yaml::Value::String(key.clone()), yaml_value);
                        }
                    }
                }

                proxies_seq.push(serde_yaml::Value::Mapping(proxy_map));
            }
            yaml_config.insert(
                serde_yaml::Value::String("proxies".to_string()),
                serde_yaml::Value::Sequence(proxies_seq),
            );
        }

        // 添加代理组
        if let Some(groups) = &config.proxy_groups {
            if !groups.is_empty() {
                let mut groups_seq = serde_yaml::Sequence::new();
                for group in groups {
                    let mut group_map = serde_yaml::Mapping::new();
                    group_map.insert(
                        serde_yaml::Value::String("name".to_string()),
                        serde_yaml::Value::String(group.name.clone()),
                    );

                    let type_str = match group.r#type {
                        ProxyGroupType::Select => "select",
                        ProxyGroupType::UrlTest => "url-test",
                        ProxyGroupType::Fallback => "fallback",
                        ProxyGroupType::LoadBalance => "load-balance",
                        ProxyGroupType::DelayTest => "delay-test",
                        ProxyGroupType::Manual => "manual",
                    };
                    group_map.insert(
                        serde_yaml::Value::String("type".to_string()),
                        serde_yaml::Value::String(type_str.to_string()),
                    );

                    let mut proxies_seq = serde_yaml::Sequence::new();
                    for proxy in &group.proxies {
                        proxies_seq.push(serde_yaml::Value::String(proxy.clone()));
                    }
                    group_map.insert(
                        serde_yaml::Value::String("proxies".to_string()),
                        serde_yaml::Value::Sequence(proxies_seq),
                    );

                    if let Some(url) = &group.url {
                        group_map.insert(
                            serde_yaml::Value::String("url".to_string()),
                            serde_yaml::Value::String(url.clone()),
                        );
                    }

                    if let Some(interval) = group.interval {
                        group_map.insert(
                            serde_yaml::Value::String("interval".to_string()),
                            serde_yaml::Value::Number(serde_yaml::Number::from(interval)),
                        );
                    }

                    if let Some(tolerance) = group.tolerance {
                        group_map.insert(
                            serde_yaml::Value::String("tolerance".to_string()),
                            serde_yaml::Value::Number(serde_yaml::Number::from(tolerance)),
                        );
                    }

                    if let Some(use_list) = &group.r#use {
                        let mut use_seq = serde_yaml::Sequence::new();
                        for use_item in use_list {
                            use_seq.push(serde_yaml::Value::String(use_item.clone()));
                        }
                        group_map.insert(
                            serde_yaml::Value::String("use".to_string()),
                            serde_yaml::Value::Sequence(use_seq),
                        );
                    }

                    groups_seq.push(serde_yaml::Value::Mapping(group_map));
                }
                yaml_config.insert(
                    serde_yaml::Value::String("proxy-groups".to_string()),
                    serde_yaml::Value::Sequence(groups_seq),
                );
            }
        }

        // 添加基本配置
        if let Some(port) = config.port {
            yaml_config.insert(
                serde_yaml::Value::String("port".to_string()),
                serde_yaml::Value::Number(serde_yaml::Number::from(port)),
            );
        }

        if let Some(mixed_port) = config.mixed_port {
            yaml_config.insert(
                serde_yaml::Value::String("mixed-port".to_string()),
                serde_yaml::Value::Number(serde_yaml::Number::from(mixed_port)),
            );
        }

        if config.allow_lan.unwrap_or(false) {
            yaml_config.insert(
                serde_yaml::Value::String("allow-lan".to_string()),
                serde_yaml::Value::Bool(true),
            );
        }

        yaml_config.insert(
            serde_yaml::Value::String("log-level".to_string()),
            serde_yaml::Value::String(
                config
                    .log_level
                    .clone()
                    .unwrap_or_else(|| "info".to_string()),
            ),
        );

        if let Some(external_controller) = &config.external_controller {
            yaml_config.insert(
                serde_yaml::Value::String("external-controller".to_string()),
                serde_yaml::Value::String(external_controller.clone()),
            );
        }

        if let Some(external_ui) = &config.external_ui {
            yaml_config.insert(
                serde_yaml::Value::String("external-ui".to_string()),
                serde_yaml::Value::String(external_ui.clone()),
            );
        }

        yaml_config.insert(
            serde_yaml::Value::String("mode".to_string()),
            serde_yaml::Value::String(config.mode.clone().unwrap_or_else(|| "rule".to_string())),
        );

        // 写入文件
        let yaml = serde_yaml::to_string(&serde_yaml::Value::Mapping(yaml_config))?;
        fs::write(output_path, yaml)?;

        Ok(())
    }
}
