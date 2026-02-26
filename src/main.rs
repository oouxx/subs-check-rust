use anyhow::Result;
use check::{CheckResult, ProxyChecker};
use clap::Parser;
// 修正后可直接编译的引用
use clash_lib::{
    Options, // 启动参数（根模块）
    // OutboundManager 需包含 manager 子模块
    app::outbound::manager::OutboundManager,
    // 线程安全版本（更常用）
    // app::outbound::manager::ThreadSafeOutboundManager,

    // 配置相关（根据实际用途选择）
    config::Config, // 核心配置解析
    // proxy::options::HandlerCommonOptions, // 代理处理器通用配置

    // 出站代理相关
    proxy::{AnyOutboundHandler, OutboundHandler}, // 替换 OutboundProxy 为 OutboundHandler
};

use anyhow::{Result, anyhow};
use clash_lib::{
    app::dns::DNSResolver,
    app::outbound::manager::{OutboundManager, ThreadSafeOutboundManager},
    common::http::client::new_http_client,
    config::{Config, Options},
    proxy::utils::healthcheck::HealthCheckResult,
    proxy::{AnyOutboundHandler, OutboundType},
};
use config::Config;
use proxy::ProxyNode;
use serde_yaml;
use serde_yaml::Value;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::{path::PathBuf, sync::Arc};
use tokio;
use tokio;
use ui::progress::ProgressTracker;
mod check;
mod config;
mod proxy;
mod ui;

/// Rust 代理检测工具
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 配置文件路径
    #[arg(short = 'f', long, default_value = "config/config.yaml")]
    config: String,

    /// 订阅链接（多个，用逗号分隔）
    #[arg(short = 's', long)]
    subscriptions: Option<String>,

    /// 输出目录
    #[arg(short = 'o', long, default_value = "./output")]
    output: String,

    /// 日志级别
    #[arg(long, default_value = "info")]
    log_level: String,

    /// 并发数
    #[arg(long)]
    concurrent: Option<usize>,

    /// 超时时间（毫秒）
    #[arg(long)]
    timeout: Option<u64>,

    /// 成功节点数量限制
    #[arg(long)]
    limit: Option<usize>,

    /// 是否启用测速
    #[arg(long)]
    speed_test: Option<bool>,

    /// 是否启用媒体检测
    #[arg(long)]
    media_check: Option<bool>,

    /// 是否显示进度条
    #[arg(long)]
    progress: Option<bool>,

    /// 测速地址
    #[arg(long)]
    speed_url: Option<String>,

    /// 输出格式：json, yaml, both
    #[arg(long, default_value = "both")]
    format: String,

    /// 生成 Clash 配置文件
    #[arg(long)]
    clash: Option<bool>,

    /// 生成 Sing-box 配置文件
    #[arg(long)]
    singbox: Option<bool>,

    /// 详细输出
    #[arg(short, long)]
    verbose: bool,
}

fn create_sample_proxies() -> Vec<ProxyNode> {
    vec![
        ProxyNode::new("本地代理 1".to_string(), "127.0.0.1".to_string(), 7890),
        ProxyNode::new("本地代理 2".to_string(), "127.0.0.1".to_string(), 7891),
        ProxyNode::new("本地代理 3".to_string(), "127.0.0.1".to_string(), 7892),
        ProxyNode::new("SSH 隧道".to_string(), "localhost".to_string(), 1080),
        ProxyNode::new("VMess 节点".to_string(), "example.com".to_string(), 443)
            .with_uuid("12345678-1234-1234-1234-123456789012".to_string()),
    ]
}

fn read_sample_proxies() -> Vec<ProxyNode> {
    // 读取文件
    let content = match fs::read_to_string("sample.yaml") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to read sample.yaml: {}", e);
            return vec![];
        }
    };

    // 先解析成通用 YAML Value
    let yaml: Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse YAML: {}", e);
            return vec![];
        }
    };

    // 获取 "proxies" key
    let proxies_value = match yaml.get("proxies") {
        Some(v) => v,
        None => {
            eprintln!("No 'proxies' key found in YAML");
            return vec![];
        }
    };

    // 反序列化成 Vec<ProxyNode>
    let proxies: Vec<ProxyNode> = match serde_yaml::from_value(proxies_value.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse 'proxies': {}", e);
            return vec![];
        }
    };

    proxies
}

fn print_results(results: &[CheckResult]) {
    println!("\n检测结果:");
    println!("{:=<80}", "");

    for (i, result) in results.iter().enumerate() {
        println!(
            "{}. {}: {}",
            i + 1,
            result.proxy.name,
            if result.is_alive {
                "✅ 存活"
            } else {
                "❌ 死亡"
            }
        );

        if result.is_alive {
            if let Some(latency) = result.latency {
                println!("   延迟: {:.2}ms", latency.as_millis());
            }

            if let Some(speed) = result.speed {
                println!("   速度: {:.2} KB/s", speed);
            }

            if let Some(country) = &result.country {
                println!("   位置: {}", country);
            }

            if let Some(ip) = &result.ip {
                println!("   IP: {}", ip);
            }

            println!(
                "   Cloudflare: {}",
                if result.is_cf_accessible {
                    "✅ 可访问"
                } else {
                    "❌ 不可访问"
                }
            );

            if result.media_unlock.youtube
                || result.media_unlock.netflix
                || result.media_unlock.disney
                || result.media_unlock.openai
            {
                println!("   媒体解锁:");
                if result.media_unlock.youtube {
                    println!("     YouTube: ✅");
                }
                if result.media_unlock.netflix {
                    println!("     Netflix: ✅");
                }
                if result.media_unlock.disney {
                    println!("     Disney+: ✅");
                }
                if result.media_unlock.openai {
                    println!("     OpenAI: ✅");
                }
                if result.media_unlock.google {
                    println!("     Google: ✅");
                }
                if result.media_unlock.tiktok {
                    println!("     TikTok: ✅");
                }
                if result.media_unlock.gemini {
                    println!("     Gemini: ✅");
                }
            }
        }
        println!("{:-<80}", "");
    }
}

fn print_summary(results: &[CheckResult]) {
    let total = results.len();
    let alive: Vec<&CheckResult> = results.iter().filter(|r| r.is_alive).collect();
    let dead: Vec<&CheckResult> = results.iter().filter(|r| !r.is_alive).collect();

    println!("\n检测摘要:");
    println!("{:=<80}", "");
    println!("总节点数: {}", total);
    println!(
        "存活节点: {} ({:.1}%)",
        alive.len(),
        (alive.len() as f64 / total as f64) * 100.0
    );
    println!(
        "死亡节点: {} ({:.1}%)",
        dead.len(),
        (dead.len() as f64 / total as f64) * 100.0
    );

    if !alive.is_empty() {
        println!("\n存活节点详情:");

        // 按速度排序
        let mut fast_nodes: Vec<&CheckResult> = alive
            .iter()
            .filter(|r| r.speed.is_some())
            .copied()
            .collect();
        fast_nodes.sort_by(|a, b| b.speed.partial_cmp(&a.speed).unwrap());

        if !fast_nodes.is_empty() {
            println!("  最快节点:");
            for (i, node) in fast_nodes.iter().take(3).enumerate() {
                if let Some(speed) = node.speed {
                    println!("    {}. {}: {:.2} KB/s", i + 1, node.proxy.name, speed);
                }
            }
        }

        // 检查媒体解锁情况
        let youtube_unlock = alive.iter().filter(|r| r.media_unlock.youtube).count();
        let netflix_unlock = alive.iter().filter(|r| r.media_unlock.netflix).count();
        let disney_unlock = alive.iter().filter(|r| r.media_unlock.disney).count();
        let openai_unlock = alive.iter().filter(|r| r.media_unlock.openai).count();

        println!("\n  媒体解锁统计:");
        println!("    YouTube: {}/{}", youtube_unlock, alive.len());
        println!("    Netflix: {}/{}", netflix_unlock, alive.len());
        println!("    Disney+: {}/{}", disney_unlock, alive.len());
        println!("    OpenAI: {}/{}", openai_unlock, alive.len());
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let args = Args::parse();

    // 设置日志级别
    unsafe {
        if args.verbose {
            std::env::set_var("RUST_LOG", "debug");
        } else {
            std::env::set_var("RUST_LOG", &args.log_level);
        }
    }
    env_logger::init();

    println!("🚀 Rust 代理检测工具 v{}", env!("CARGO_PKG_VERSION"));
    println!("{:=<80}", "");

    // 尝试加载配置文件
    let mut config = if Path::new(&args.config).exists() {
        println!("📁 从配置文件加载设置: {}", args.config);
        match Config::load_from_file(&args.config) {
            Ok(config) => {
                println!("✅ 配置文件加载成功");
                config
            }
            Err(e) => {
                println!("⚠️  配置文件加载失败: {}", e);
                println!("📝 使用默认配置");
                Config::default()
            }
        }
    } else {
        println!("📝 使用默认配置 (配置文件不存在: {})", args.config);
        Config::default()
    };

    // 覆盖命令行参数
    if let Some(concurrent) = args.concurrent {
        config.concurrent = concurrent;
    }
    if let Some(timeout) = args.timeout {
        config.timeout = timeout;
    }
    if let Some(limit) = args.limit {
        config.success_limit = limit;
    }
    if let Some(speed_test) = args.speed_test {
        config.media_check = speed_test;
    }
    if let Some(media_check) = args.media_check {
        config.media_check = media_check;
    }
    if let Some(progress) = args.progress {
        config.print_progress = progress;
    }
    if let Some(speed_url) = args.speed_url {
        config.speed_test_url = Some(speed_url);
    }
    if let Some(clash) = args.clash {
        config.generate_clash_config = clash;
    }
    if let Some(singbox) = args.singbox {
        config.generate_singbox_config = singbox;
    }

    // 处理订阅链接
    if let Some(subscriptions) = args.subscriptions {
        let urls: Vec<&str> = subscriptions.split(',').collect();
        for url in urls {
            config.subscriptions.push(config::Subscription {
                name: format!("订阅-{}", url),
                url: url.to_string(),
                enabled: true,
            });
        }
    }

    // 设置输出目录
    config.output_dir = args.output;

    // 创建进度跟踪器
    let progress_tracker = ProgressTracker::new(&config);

    // 打印配置信息
    println!("\n⚙️  当前配置:");
    println!("  配置文件: {}", args.config);
    println!("  输出目录: {}", config.output_dir);
    println!("  并发数: {}", config.concurrent);
    println!("  超时时间: {}ms", config.timeout);
    println!("  成功限制: {}", config.success_limit);
    println!(
        "  测速: {}",
        if config.is_speed_test_enabled() {
            "✅ 启用"
        } else {
            "❌ 禁用"
        }
    );
    println!(
        "  媒体检测: {}",
        if config.is_media_check_enabled() {
            "✅ 启用"
        } else {
            "❌ 禁用"
        }
    );
    println!(
        "  进度显示: {}",
        if config.print_progress {
            "✅ 启用"
        } else {
            "❌ 禁用"
        }
    );
    println!("  输出格式: {}", args.format);
    println!(
        "  Clash 配置: {}",
        if config.generate_clash_config {
            "✅ 生成"
        } else {
            "❌ 不生成"
        }
    );
    println!(
        "  Sing-box 配置: {}",
        if config.generate_singbox_config {
            "✅ 生成"
        } else {
            "❌ 不生成"
        }
    );

    // 创建检测器
    let config_clone = config.clone();
    let checker = ProxyChecker::new(config_clone);

    // 获取代理列表（这里使用示例数据）
    println!("\n📡 获取代理节点...");
    let mut proxies = read_sample_proxies();
    println!("✅ 获取到 {} 个代理节点", proxies.len());

    // 智能乱序（模拟原项目的功能）
    if config.threshold > 0.0 {
        println!("🔄 对代理节点进行智能乱序...");
        proxy::smart_shuffle_proxies(&mut proxies, config.threshold, config.concurrent * 5);
        println!("✅ 节点乱序完成");
    }

    // 设置进度跟踪器
    progress_tracker.set_total_nodes(proxies.len() as u64);

    // 执行检测
    println!("\n🔍 开始检测代理节点...");
    println!("{:=<80}", "");

    let results = checker.check_proxies(proxies).await;

    // 完成进度显示
    if config.print_progress {
        progress_tracker.finalize();
    }

    // 打印统计信息
    checker.print_stats();

    // 打印详细结果
    print_results(&results);

    // 打印摘要
    print_summary(&results);

    // 保存结果（如果配置了输出目录）
    if !config.output_dir.is_empty() {
        println!("\n💾 保存检测结果到: {}", config.output_dir);
        // 这里可以添加保存结果的逻辑
        println!("✅ 结果保存完成");
    }

    println!("\n🎉 检测完成!");

    Ok(())
}

async fn use_clash_rs() -> Result<()> {
    // 1. 初始化 Clash 启动参数（加载配置文件）
    let config_path = PathBuf::from("./sample.yaml");
    if !config_path.exists() {
        return Err(anyhow!("配置文件不存在: {:?}", config_path));
    }

    // 构建启动 Options（与 clash-rs 测试代码保持一致）
    let options = Options {
        config: Config::File(config_path.to_string_lossy().to_string()),
        cwd: Some(std::env::current_dir()?.to_string_lossy().to_string()),
        rt: None,
        log_file: None,
    };

    // 解析配置（clash-rs 内部通过 Options 解析完整配置）
    let resolved_config = options.resolve_config().await?;

    // 2. 初始化依赖组件（DNS 解析器、HTTP 客户端等）
    // 初始化 DNS 解析器（OutboundManager 必需）
    let dns_resolver = DNSResolver::new(&resolved_config.dns, None)
        .await
        .map_err(|e| anyhow!("DNS 解析器初始化失败: {}", e))?;
    let dns_resolver = Arc::new(dns_resolver);

    // 初始化 HTTP 客户端（健康检查必需）
    let http_client = new_http_client(dns_resolver.clone(), None)?;

    // 3. 初始化出站管理器（核心：管理所有代理节点）
    let outbound_manager = OutboundManager::new(
        resolved_config.proxies.unwrap_or_default(),
        resolved_config.proxy_groups.unwrap_or_default(),
        resolved_config.proxy_providers.unwrap_or_default(),
        Some(dns_resolver),
        resolved_config.interface.clone(),
        http_client,
    )
    .await
    .map_err(|e| anyhow!("OutboundManager 初始化失败: {}", e))?;

    // 包装为线程安全版本（clash-rs 标准用法）
    let outbound_manager: ThreadSafeOutboundManager =
        Arc::new(tokio::sync::RwLock::new(outbound_manager));

    // 4. 获取所有代理节点的处理器
    let all_proxies: Vec<AnyOutboundHandler> = outbound_manager.read().await.get_proxies().await;
    if all_proxies.is_empty() {
        return Err(anyhow!("配置中未找到任何代理节点"));
    }

    // 5. 遍历 & 使用代理节点
    for proxy in all_proxies {
        println!("\n=== 代理信息 ===");
        println!("代理名称: {}", proxy.name());
        println!("代理类型: {:?}", proxy.proto()); // 输出：Ss / Vmess / Socks5 等

        // 示例1：检查 UDP 支持
        let support_udp = proxy.support_udp().await;
        println!("支持 UDP: {}", support_udp);

        // 示例2：健康检查（URL 测试，clash-rs 标准健康检查方式）
        let health_check_url = "http://www.gstatic.com/generate_204"; // 通用健康检查 URL
        let timeout = std::time::Duration::from_secs(5);

        let check_result: HealthCheckResult = outbound_manager
            .read()
            .await
            .url_test(&[proxy.clone()], health_check_url, timeout)
            .await
            .into_iter()
            .next()
            .ok_or(anyhow!("健康检查无结果"))??;

        println!(
            "代理 {} 延迟: {}ms (可用性: {})",
            proxy.name(),
            check_result.actual.as_millis(),
            check_result.actual.as_millis() < 5000 // 延迟 <5s 视为可用
        );
    }

    Ok(())
}
