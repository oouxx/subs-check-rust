//! Clash 代理管理器核心逻辑
//! 使用 clash-lib 实现真实的代理健康检查

use super::health_check::{check_proxies_health, check_proxies_health_with_config};
use super::types::ProxyNodeInfo;
use anyhow::{Result, anyhow};
use serde_yaml;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Clash 代理管理器（简化版本）
#[derive(Debug, Clone)]
pub struct ClashProxyManager {
    config_path: String,
    proxies: Vec<ProxyNodeInfo>,
}

impl ClashProxyManager {
    /// 从配置文件初始化 Clash 代理管理器
    ///
    /// # 参数
    /// - config_path: Clash 配置文件路径
    ///
    /// # 返回
    /// 初始化后的 ClashProxyManager 实例
    pub async fn from_config_file<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_path_str = config_path.as_ref().to_string_lossy().to_string();

        // 1. 检查配置文件是否存在
        if !config_path.as_ref().exists() {
            return Err(anyhow!("配置文件不存在: {}", config_path_str));
        }

        // 2. 读取配置文件
        let content = fs::read_to_string(&config_path)?;

        // 3. 解析 YAML 配置
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

        // 4. 提取代理节点信息
        let proxies = Self::extract_proxies_from_yaml(&yaml)?;

        // 5. 执行真实的健康检查（使用 clash-lib）
        let checked_proxies = Self::check_proxies_health(&proxies).await;

        Ok(Self {
            config_path: config_path_str,
            proxies: checked_proxies,
        })
    }

    /// 获取所有代理节点信息
    pub async fn get_all_proxy_nodes(&self) -> Result<Vec<ProxyNodeInfo>> {
        Ok(self.proxies.clone())
    }

    /// 根据名称查找代理节点
    pub fn find_proxy_by_name(&self, name: &str) -> Option<&ProxyNodeInfo> {
        self.proxies.iter().find(|p| p.name == name)
    }

    /// 获取可用代理节点（延迟小于 5000ms）
    pub fn get_available_proxies(&self) -> Vec<&ProxyNodeInfo> {
        self.proxies.iter().filter(|p| p.is_available()).collect()
    }

    /// 按延迟排序代理节点
    pub fn get_sorted_by_delay(&self) -> Vec<&ProxyNodeInfo> {
        let mut proxies: Vec<&ProxyNodeInfo> = self.proxies.iter().collect();
        proxies.sort_by(|a, b| a.delay_ms.cmp(&b.delay_ms));
        proxies
    }

    /// 从 YAML 配置中提取代理节点信息
    fn extract_proxies_from_yaml(yaml: &serde_yaml::Value) -> Result<Vec<ProxyNodeInfo>> {
        let mut proxies = Vec::new();

        // 尝试从 "proxies" 字段获取代理列表
        if let Some(proxies_value) = yaml.get("proxies") {
            if let serde_yaml::Value::Sequence(proxy_list) = proxies_value {
                for proxy in proxy_list {
                    if let serde_yaml::Value::Mapping(proxy_map) = proxy {
                        let name = proxy_map
                            .get(&serde_yaml::Value::String("name".to_string()))
                            .and_then(|v| v.as_str())
                            .unwrap_or("未知节点")
                            .to_string();

                        let server = proxy_map
                            .get(&serde_yaml::Value::String("server".to_string()))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let port = proxy_map
                            .get(&serde_yaml::Value::String("port".to_string()))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u16;

                        let proto = proxy_map
                            .get(&serde_yaml::Value::String("type".to_string()))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();

                        // 检查是否支持 UDP
                        let support_udp = proxy_map
                            .get(&serde_yaml::Value::String("udp".to_string()))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        // 创建代理节点信息
                        let proto_clone = proto.clone();
                        let proxy_info = ProxyNodeInfo::new(name, proto, server, port)
                            .with_udp_support(support_udp)
                            .with_node_type(proto_clone);

                        proxies.push(proxy_info);
                    }
                }
            }
        }

        if proxies.is_empty() {
            return Err(anyhow!("配置文件中未找到任何代理节点"));
        }

        Ok(proxies)
    }

    /// 执行真实的健康检查（使用 clash-lib）
    async fn check_proxies_health(proxies: &[ProxyNodeInfo]) -> Vec<ProxyNodeInfo> {
        // 使用默认配置进行健康检查
        check_proxies_health(proxies).await
    }

    /// 执行带自定义配置的健康检查
    async fn check_proxies_health_with_timeout(
        proxies: &[ProxyNodeInfo],
        timeout_ms: u64,
    ) -> Vec<ProxyNodeInfo> {
        check_proxies_health_with_config(proxies, timeout_ms, None).await
    }

    /// 生成 Clash 配置文件
    pub fn generate_clash_config(&self, output_path: &str) -> Result<()> {
        let mut config = HashMap::new();

        // 构建代理列表
        let proxies: Vec<serde_yaml::Value> = self
            .proxies
            .iter()
            .map(|proxy| {
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

                // 添加延迟信息作为注释
                if proxy.delay_ms > 0 {
                    proxy_map.insert(
                        serde_yaml::Value::String("delay".to_string()),
                        serde_yaml::Value::String(format!("{}ms", proxy.delay_ms)),
                    );
                }

                // 添加额外信息
                if let Some(extra_info) = &proxy.extra_info {
                    if let Some(obj) = extra_info.as_object() {
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

                serde_yaml::Value::Mapping(proxy_map)
            })
            .collect();

        config.insert("proxies".to_string(), serde_yaml::Value::Sequence(proxies));

        // 添加代理组
        let mut proxy_groups = Vec::new();

        // 自动选择组（基于延迟）
        let mut auto_group = serde_yaml::Mapping::new();
        auto_group.insert(
            serde_yaml::Value::String("name".to_string()),
            serde_yaml::Value::String("自动选择".to_string()),
        );
        auto_group.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String("url-test".to_string()),
        );

        let proxy_names: Vec<serde_yaml::Value> = self
            .proxies
            .iter()
            .filter(|p| p.is_available())
            .map(|p| serde_yaml::Value::String(p.name.clone()))
            .collect();

        auto_group.insert(
            serde_yaml::Value::String("proxies".to_string()),
            serde_yaml::Value::Sequence(proxy_names),
        );
        auto_group.insert(
            serde_yaml::Value::String("url".to_string()),
            serde_yaml::Value::String("http://www.gstatic.com/generate_204".to_string()),
        );
        auto_group.insert(
            serde_yaml::Value::String("interval".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(300)),
        );

        proxy_groups.push(serde_yaml::Value::Mapping(auto_group));

        // 手动选择组（所有可用节点）
        let mut manual_group = serde_yaml::Mapping::new();
        manual_group.insert(
            serde_yaml::Value::String("name".to_string()),
            serde_yaml::Value::String("手动选择".to_string()),
        );
        manual_group.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String("select".to_string()),
        );

        let all_proxy_names: Vec<serde_yaml::Value> = self
            .proxies
            .iter()
            .filter(|p| p.is_available())
            .map(|p| serde_yaml::Value::String(p.name.clone()))
            .collect();

        manual_group.insert(
            serde_yaml::Value::String("proxies".to_string()),
            serde_yaml::Value::Sequence(all_proxy_names),
        );

        proxy_groups.push(serde_yaml::Value::Mapping(manual_group));

        config.insert(
            "proxy-groups".to_string(),
            serde_yaml::Value::Sequence(proxy_groups),
        );

        // 写入文件
        let yaml = serde_yaml::to_string(&serde_yaml::Value::Mapping(
            config
                .into_iter()
                .map(|(k, v)| (serde_yaml::Value::String(k), v))
                .collect(),
        ))?;

        fs::write(output_path, yaml)?;

        Ok(())
    }

    /// 重新检查代理健康状态
    pub async fn recheck_proxies_health(&mut self, timeout_ms: Option<u64>) -> Result<()> {
        let timeout = timeout_ms.unwrap_or(5000);
        let checked_proxies = Self::check_proxies_health_with_timeout(&self.proxies, timeout).await;
        self.proxies = checked_proxies;
        Ok(())
    }

    /// 获取健康检查统计信息
    pub fn get_health_stats(&self) -> (usize, usize, f64) {
        let total = self.proxies.len();
        let available = self.get_available_proxies().len();
        let success_rate = if total > 0 {
            (available as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        (total, available, success_rate)
    }
}
