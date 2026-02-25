use crate::config::Config;
use crate::proxy::ProxyNode;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaUnlockResult {
    pub youtube: bool,
    pub netflix: bool,
    pub disney: bool,
    pub openai: bool,
    pub google: bool,
    pub cloudflare: bool,
    pub tiktok: bool,
    pub gemini: bool,
}

impl Default for MediaUnlockResult {
    fn default() -> Self {
        Self {
            youtube: false,
            netflix: false,
            disney: false,
            openai: false,
            google: false,
            cloudflare: false,
            tiktok: false,
            gemini: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub proxy: ProxyNode,
    pub is_alive: bool,
    pub latency: Option<Duration>,
    pub speed: Option<f64>, // KB/s
    pub media_unlock: MediaUnlockResult,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub ip: Option<String>,
    pub ip_risk: Option<String>,
    pub is_cf_accessible: bool,
    pub cf_location: Option<String>,
    pub cf_ip: Option<String>,
}

pub struct Stats {
    pub total_nodes: AtomicU64,
    pub alive_nodes: AtomicU64,
    pub checked_nodes: AtomicU64,
    pub failed_nodes: AtomicU64,
    pub total_bytes: AtomicU64,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            total_nodes: AtomicU64::new(0),
            alive_nodes: AtomicU64::new(0),
            checked_nodes: AtomicU64::new(0),
            failed_nodes: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
        }
    }

    pub fn increment_alive(&self) {
        self.alive_nodes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_checked(&self) {
        self.checked_nodes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_failed(&self) {
        self.failed_nodes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_bytes(&self, bytes: u64) {
        self.total_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn get_success_rate(&self) -> f64 {
        let total = self.total_nodes.load(Ordering::Relaxed);
        let alive = self.alive_nodes.load(Ordering::Relaxed);

        if total > 0 {
            (alive as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    }
}

pub struct ProxyChecker {
    config: Config,
    stats: Arc<Stats>,
}

impl ProxyChecker {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            stats: Arc::new(Stats::new()),
        }
    }

    pub async fn check_proxies(&self, proxies: Vec<ProxyNode>) -> Vec<CheckResult> {
        let stats = self.stats.clone();
        let config = self.config.clone();

        stats
            .total_nodes
            .store(proxies.len() as u64, Ordering::Relaxed);

        // 创建通道用于收集结果
        let (tx, mut rx) = mpsc::channel(100);

        // 创建任务处理代理检测
        let mut tasks = Vec::new();

        for proxy in proxies {
            let tx = tx.clone();
            let config = config.clone();
            let stats = stats.clone();

            let task = task::spawn(async move {
                match check_single_proxy(proxy, config, stats).await {
                    Ok(result) => {
                        let _ = tx.send(result).await;
                    }
                    Err(e) => {
                        eprintln!("检测代理失败: {}", e);
                    }
                }
            });

            tasks.push(task);
        }

        // 收集结果
        let mut results = Vec::new();
        drop(tx); // 关闭发送端，这样接收端在收完所有消息后会结束

        while let Some(result) = rx.recv().await {
            results.push(result);

            // 如果达到成功限制，停止收集
            if self.config.success_limit > 0 {
                let alive_count = results.iter().filter(|r| r.is_alive).count();
                if alive_count >= self.config.success_limit {
                    break;
                }
            }
        }

        // 等待所有任务完成
        for task in tasks {
            let _ = task.await;
        }

        results
    }

    pub fn print_stats(&self) {
        let total = self.stats.total_nodes.load(Ordering::Relaxed);
        let alive = self.stats.alive_nodes.load(Ordering::Relaxed);
        let checked = self.stats.checked_nodes.load(Ordering::Relaxed);
        let failed = self.stats.failed_nodes.load(Ordering::Relaxed);
        let total_bytes = self.stats.total_bytes.load(Ordering::Relaxed);

        println!("检测统计:");
        println!("  总节点数: {}", total);
        println!("  已检测数: {}", checked);
        println!("  存活节点: {}", alive);
        println!("  失败节点: {}", failed);
        println!(
            "  总消耗流量: {:.3} GB",
            total_bytes as f64 / 1024.0 / 1024.0 / 1024.0
        );

        if total > 0 {
            let success_rate = self.stats.get_success_rate();
            println!("  成功率: {:.2}%", success_rate);
        }
    }

    pub fn get_stats(&self) -> Arc<Stats> {
        self.stats.clone()
    }
}

async fn check_single_proxy(
    proxy: ProxyNode,
    config: Config,
    stats: Arc<Stats>,
) -> anyhow::Result<CheckResult> {
    // 构建代理 URL
    let proxy_url = proxy.to_proxy_url();

    // 创建 HTTP 客户端
    let client = create_http_client(&proxy_url, config.timeout)?;

    // 检查存活
    let start = Instant::now();
    let is_alive = check_alive(&client).await.unwrap_or(false);
    let latency = Some(start.elapsed());

    if !is_alive {
        stats.increment_failed();
        stats.increment_checked();
        return Ok(CheckResult {
            proxy,
            is_alive: false,
            latency,
            speed: None,
            media_unlock: MediaUnlockResult::default(),
            country: None,
            country_code: None,
            ip: None,
            ip_risk: None,
            is_cf_accessible: false,
            cf_location: None,
            cf_ip: None,
        });
    }

    stats.increment_alive();

    // 检查 Cloudflare
    let (is_cf_accessible, cf_location, cf_ip) = check_cloudflare(&client)
        .await
        .unwrap_or((false, None, None));

    // 如果配置了丢弃无法访问 Cloudflare 的节点
    if config.drop_bad_cf_nodes && !is_cf_accessible {
        stats.increment_failed();
        stats.increment_checked();
        return Ok(CheckResult {
            proxy,
            is_alive: false,
            latency,
            speed: None,
            media_unlock: MediaUnlockResult::default(),
            country: None,
            country_code: None,
            ip: None,
            ip_risk: None,
            is_cf_accessible: false,
            cf_location: None,
            cf_ip: None,
        });
    }

    // 测速
    let speed = if config.is_speed_test_enabled() {
        if let Some(test_url) = &config.speed_test_url {
            match check_speed(&client, test_url, &stats).await {
                Ok(speed) => {
                    if speed >= config.min_speed {
                        Some(speed)
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        }
    } else {
        None
    };

    // 媒体解锁检测
    let media_unlock = if config.is_media_check_enabled() {
        MediaUnlockResult {
            youtube: check_youtube(&client).await.unwrap_or(false),
            netflix: check_netflix(&client).await.unwrap_or(false),
            disney: check_disney(&client).await.unwrap_or(false),
            openai: check_openai(&client).await.unwrap_or(false),
            google: check_google(&client).await.unwrap_or(false),
            cloudflare: is_cf_accessible,
            tiktok: check_tiktok(&client).await.unwrap_or(false),
            gemini: check_gemini(&client).await.unwrap_or(false),
        }
    } else {
        MediaUnlockResult::default()
    };

    stats.increment_checked();

    Ok(CheckResult {
        proxy,
        is_alive: true,
        latency,
        speed,
        media_unlock,
        country: cf_location.clone(),
        country_code: None,
        ip: cf_ip.clone(),
        ip_risk: None,
        is_cf_accessible,
        cf_location,
        cf_ip,
    })
}

fn create_http_client(proxy_url: &str, timeout_ms: u64) -> anyhow::Result<Client> {
    let proxy = reqwest::Proxy::all(proxy_url)?;

    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_millis(timeout_ms))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    Ok(client)
}

async fn check_alive(client: &Client) -> anyhow::Result<bool> {
    let response = client
        .head("https://gstatic.com/generate_204")
        .send()
        .await?;

    Ok(response.status().as_u16() == 204)
}

async fn check_google(client: &Client) -> anyhow::Result<bool> {
    let response = client
        .head("https://www.google.com/generate_204")
        .send()
        .await?;

    Ok(response.status().as_u16() == 204)
}

async fn check_cloudflare(
    client: &Client,
) -> anyhow::Result<(bool, Option<String>, Option<String>)> {
    let response = client
        .get("https://cloudflare.com/cdn-cgi/trace")
        .send()
        .await?;

    let is_accessible = response.status().is_success();
    let body = response.text().await?;

    // 解析 trace 信息获取位置和 IP
    let mut loc = None;
    let mut ip = None;

    for line in body.lines() {
        if line.starts_with("loc=") {
            loc = Some(line[4..].to_string());
        } else if line.starts_with("ip=") {
            ip = Some(line[3..].to_string());
        }
    }

    Ok((is_accessible, loc, ip))
}

async fn check_youtube(client: &Client) -> anyhow::Result<bool> {
    let response = client.get("https://www.youtube.com/premium").send().await?;

    let text = response.text().await?;
    // 简单检查是否包含特定关键词
    Ok(!text.contains("YouTube Premium is not available in your country"))
}

async fn check_netflix(client: &Client) -> anyhow::Result<bool> {
    let response = client
        .get("https://www.netflix.com/title/81280792") // 一个特定的剧集ID
        .send()
        .await?;

    Ok(response.status().is_success())
}

async fn check_disney(client: &Client) -> anyhow::Result<bool> {
    let response = client.get("https://www.disneyplus.com").send().await?;

    Ok(response.status().is_success())
}

async fn check_openai(client: &Client) -> anyhow::Result<bool> {
    let response = client.get("https://chat.openai.com").send().await?;

    Ok(response.status().is_success())
}

async fn check_tiktok(client: &Client) -> anyhow::Result<bool> {
    let response = client.get("https://www.tiktok.com").send().await?;

    Ok(response.status().is_success())
}

async fn check_gemini(client: &Client) -> anyhow::Result<bool> {
    let response = client.get("https://gemini.google.com").send().await?;

    Ok(response.status().is_success())
}

async fn check_speed(client: &Client, test_url: &str, stats: &Stats) -> anyhow::Result<f64> {
    let start = Instant::now();

    let response = client.get(test_url).send().await?;

    let content = response.bytes().await?;
    let elapsed = start.elapsed();

    // 记录流量
    stats.add_bytes(content.len() as u64);

    // 计算速度 (KB/s)
    let speed = content.len() as f64 / 1024.0 / elapsed.as_secs_f64();

    Ok(speed)
}

pub mod platform {
    use super::*;

    pub async fn check_all_platforms(client: &Client) -> MediaUnlockResult {
        MediaUnlockResult {
            youtube: check_youtube(client).await.unwrap_or(false),
            netflix: check_netflix(client).await.unwrap_or(false),
            disney: check_disney(client).await.unwrap_or(false),
            openai: check_openai(client).await.unwrap_or(false),
            google: check_google(client).await.unwrap_or(false),
            cloudflare: check_cloudflare(client)
                .await
                .map(|(ok, _, _)| ok)
                .unwrap_or(false),
            tiktok: check_tiktok(client).await.unwrap_or(false),
            gemini: check_gemini(client).await.unwrap_or(false),
        }
    }
}
