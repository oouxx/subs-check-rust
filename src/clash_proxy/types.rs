//! Clash 代理节点信息结构体定义

use serde::{Deserialize, Serialize};

/// 代理节点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyNodeInfo {
    /// 节点名称
    pub name: String,
    /// 协议类型
    pub proto: String,
    /// 服务器地址
    pub server: String,
    /// 端口号
    pub port: u16,
    /// 是否支持 UDP
    pub support_udp: bool,
    /// 延迟（毫秒）
    pub delay_ms: u64,
    /// 节点类型（如：vmess, vless, trojan, ss, ssr, http, socks5 等）
    pub node_type: String,
    /// 额外信息（JSON 格式）
    pub extra_info: Option<serde_json::Value>,
}

impl ProxyNodeInfo {
    /// 创建新的代理节点信息
    pub fn new(name: String, proto: String, server: String, port: u16) -> Self {
        Self {
            name,
            proto,
            server,
            port,
            support_udp: false,
            delay_ms: 0,
            node_type: "unknown".to_string(),
            extra_info: None,
        }
    }

    /// 设置是否支持 UDP
    pub fn with_udp_support(mut self, support_udp: bool) -> Self {
        self.support_udp = support_udp;
        self
    }

    /// 设置延迟
    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// 设置节点类型
    pub fn with_node_type(mut self, node_type: String) -> Self {
        self.node_type = node_type;
        self
    }

    /// 设置额外信息
    pub fn with_extra_info(mut self, extra_info: serde_json::Value) -> Self {
        self.extra_info = Some(extra_info);
        self
    }

    /// 获取代理地址（server:port）
    pub fn get_address(&self) -> String {
        format!("{}:{}", self.server, self.port)
    }

    /// 检查节点是否可用（延迟小于 5000ms）
    pub fn is_available(&self) -> bool {
        self.delay_ms > 0 && self.delay_ms < 5000
    }

    /// 获取延迟描述
    pub fn get_delay_description(&self) -> String {
        if self.delay_ms == 0 {
            "未检测".to_string()
        } else if self.delay_ms < 100 {
            format!("{}ms (极快)", self.delay_ms)
        } else if self.delay_ms < 300 {
            format!("{}ms (快速)", self.delay_ms)
        } else if self.delay_ms < 1000 {
            format!("{}ms (一般)", self.delay_ms)
        } else {
            format!("{}ms (较慢)", self.delay_ms)
        }
    }
}
