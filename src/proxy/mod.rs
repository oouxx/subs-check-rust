use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyNode {
    pub name: String,
    pub server: String,
    pub port: u16,
    pub protocol: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub uuid: Option<String>,
    pub alter_id: Option<u16>,
    pub cipher: Option<String>,
    pub tls: Option<bool>,
    pub sni: Option<String>,
    pub network: Option<String>,
    pub ws_path: Option<String>,
    pub ws_headers: Option<serde_json::Value>,
    pub skip_cert_verify: Option<bool>,
    pub server_name: Option<String>,
    pub fingerprint: Option<String>,
    pub alpn: Option<Vec<String>>,
    pub servername: Option<String>,
    pub flow: Option<String>,
    pub reality_opts: Option<serde_json::Value>,
}

impl ProxyNode {
    pub fn new(name: String, server: String, port: u16, protocol: String) -> Self {
        Self {
            name,
            server,
            port,
            protocol,
            username: None,
            password: None,
            uuid: None,
            alter_id: None,
            cipher: None,
            tls: None,
            sni: None,
            network: None,
            ws_path: None,
            ws_headers: None,
            skip_cert_verify: None,
            server_name: None,
            fingerprint: None,
            alpn: None,
            servername: None,
            flow: None,
            reality_opts: None,
        }
    }

    pub fn with_auth(mut self, username: String, password: String) -> Self {
        self.username = Some(username);
        self.password = Some(password);
        self
    }

    pub fn with_uuid(mut self, uuid: String) -> Self {
        self.uuid = Some(uuid);
        self
    }

    pub fn to_proxy_url(&self) -> String {
        match (&self.username, &self.password) {
            (Some(username), Some(password)) => {
                format!(
                    "{}://{}:{}@{}:{}",
                    self.protocol, username, password, self.server, self.port
                )
            }
            _ => {
                format!("{}://{}:{}", self.protocol, self.server, self.port)
            }
        }
    }

    pub fn get_ip_address(&self) -> Option<IpAddr> {
        self.server.parse().ok()
    }

    pub fn is_same_cidr(&self, other: &ProxyNode, threshold: f64) -> bool {
        if let (Some(ip1), Some(ip2)) = (self.get_ip_address(), other.get_ip_address()) {
            match (ip1, ip2) {
                (IpAddr::V4(ip1), IpAddr::V4(ip2)) => {
                    let octets1 = ip1.octets();
                    let octets2 = ip2.octets();

                    if threshold >= 1.0 {
                        // /32 - 完全相同
                        octets1 == octets2
                    } else if threshold >= 0.75 {
                        // /24 - 前三段相同
                        octets1[0] == octets2[0]
                            && octets1[1] == octets2[1]
                            && octets1[2] == octets2[2]
                    } else if threshold >= 0.5 {
                        // /16 - 前两段相同
                        octets1[0] == octets2[0] && octets1[1] == octets2[1]
                    } else if threshold >= 0.25 {
                        // /8 - 第一段相同
                        octets1[0] == octets2[0]
                    } else {
                        false
                    }
                }
                (IpAddr::V6(_), IpAddr::V6(_)) => {
                    // IPv6 简化处理
                    self.server == other.server
                }
                _ => false,
            }
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProxyGroup {
    pub name: String,
    pub proxies: Vec<ProxyNode>,
    pub group_type: String,
    pub url: Option<String>,
    pub interval: Option<u64>,
}

impl ProxyGroup {
    pub fn new(name: String, group_type: String) -> Self {
        Self {
            name,
            proxies: vec![],
            group_type,
            url: None,
            interval: None,
        }
    }

    pub fn add_proxy(&mut self, proxy: ProxyNode) {
        self.proxies.push(proxy);
    }

    pub fn size(&self) -> usize {
        self.proxies.len()
    }
}

pub fn parse_subscription_url(url: &str) -> anyhow::Result<Url> {
    Url::parse(url).map_err(|e| anyhow::anyhow!("解析订阅URL失败: {}", e))
}

pub fn validate_proxy_node(proxy: &ProxyNode) -> anyhow::Result<()> {
    if proxy.server.is_empty() {
        return Err(anyhow::anyhow!("服务器地址不能为空"));
    }

    if proxy.port == 0 {
        return Err(anyhow::anyhow!("端口号不能为0"));
    }

    if proxy.protocol.is_empty() {
        return Err(anyhow::anyhow!("协议不能为空"));
    }

    // 验证协议是否支持
    let supported_protocols = [
        "http", "https", "socks5", "socks5h", "ss", "ssr", "vmess", "vless", "trojan", "hysteria",
    ];

    if !supported_protocols.contains(&proxy.protocol.as_str()) {
        return Err(anyhow::anyhow!("不支持的协议: {}", proxy.protocol));
    }

    Ok(())
}

pub fn smart_shuffle_proxies(proxies: &mut [ProxyNode], threshold: f64, min_spacing: usize) {
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    if proxies.len() <= min_spacing {
        return;
    }

    let mut rng = thread_rng();

    // 多次迭代改善分布
    for _ in 0..3 {
        proxies.shuffle(&mut rng);

        // 检查并调整相同CIDR的节点间距
        for i in 0..proxies.len() {
            for j in (i + 1)..proxies.len().min(i + min_spacing) {
                if proxies[i].is_same_cidr(&proxies[j], threshold) {
                    // 找到可以交换的位置
                    if let Some(k) = (j + 1..proxies.len()).find(|&k| {
                        !proxies[i].is_same_cidr(&proxies[k], threshold)
                            && !proxies[j].is_same_cidr(&proxies[k], threshold)
                    }) {
                        proxies.swap(j, k);
                    }
                }
            }
        }
    }
}
