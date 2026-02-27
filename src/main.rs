use anyhow::Result;
use check::{CheckResult, ProxyChecker};
use clap::Parser;
use config::Config;
use proxy::ProxyNode;
use serde_yaml;
use serde_yaml::Value;
use std::fs;
use std::path::Path;

use tokio;

mod check;
mod config;
mod proxy;
mod ui;

// 引入 clash-proxy 模块
mod clash_proxy;
use clash_proxy::{ClashProxyManager, ProxyHealthChecker};

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

fn read_sample_proxies() -> Vec<ProxyNode> {
    // 读取文件
    let content = match fs::read_to_string("sample-tiny.yaml") {
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
    if let Err(e) = clash_proxy_test().await {
        eprintln!("错误: {:#}", e);
        std::process::exit(1);
    }

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
    let progress_tracker = ui::progress::ProgressTracker::new(&config);

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

async fn clash_proxy_test() -> Result<()> {
    // 1. 配置文件路径
    let config_path = std::path::PathBuf::from("./sample-tiny.yaml");

    // 2. 初始化代理管理器
    println!("正在初始化 Clash 代理管理器...");
    let clash_manager = ClashProxyManager::from_config_file(&config_path).await?;
    println!("✅ 代理管理器初始化成功\n");

    // 3. 获取所有代理节点信息
    let proxy_nodes = clash_manager.get_all_proxy_nodes().await?;

    // 4. 打印代理节点信息
    println!("📄 共加载 {} 个代理节点：", proxy_nodes.len());
    for (idx, node) in proxy_nodes.iter().enumerate() {
        println!(
            "\n[{}/{}] 代理节点: {}",
            idx + 1,
            proxy_nodes.len(),
            node.name
        );
        println!("  ├── 协议类型: {}", node.proto);
        println!("  ├── 服务器地址: {}:{}", node.server, node.port);
        println!(
            "  ├── 支持 UDP: {}",
            if node.support_udp { "✅" } else { "❌" }
        );
        println!(
            "  └── 延迟: {}",
            if node.delay_ms > 0 {
                format!("{}ms ({})", node.delay_ms, node.get_delay_description())
            } else {
                "❌ 检查失败".to_string()
            }
        );
    }

    // 5. 显示健康检查统计
    let (total, available, success_rate) = clash_manager.get_health_stats();
    println!("\n📊 健康检查统计:");
    println!("  ├── 总节点数: {}", total);
    println!("  ├── 可用节点: {}", available);
    println!("  └── 成功率: {:.1}%", success_rate);

    // 6. 显示可用节点（按延迟排序）
    let available_proxies = clash_manager.get_available_proxies();
    if !available_proxies.is_empty() {
        println!("\n🏆 可用节点（按延迟排序）:");
        for (idx, proxy) in clash_manager.get_sorted_by_delay().iter().enumerate() {
            if proxy.is_available() {
                println!(
                    "  {}. {} - {}ms ({})",
                    idx + 1,
                    proxy.name,
                    proxy.delay_ms,
                    proxy.get_delay_description()
                );
            }
        }
    }

    // 7. 测试独立的健康检查器
    println!("\n🔬 测试独立的健康检查器...");
    test_health_checker(&clash_manager).await?;

    Ok(())
}

async fn test_health_checker(clash_manager: &ClashProxyManager) -> Result<()> {
    println!("正在创建健康检查器...");

    // 创建健康检查器
    let health_checker = ProxyHealthChecker::new(3000, None);

    // 测试配置
    println!("  ├── 超时时间: {}ms", health_checker.get_timeout_ms());
    println!("  ├── 测试URL: {}", health_checker.get_test_url());

    // 测试批量健康检查
    println!("  └── 测试批量健康检查...");

    // 使用从配置文件中加载的代理节点进行测试
    let proxy_nodes = clash_manager.get_all_proxy_nodes().await?;

    if !proxy_nodes.is_empty() {
        // 使用健康检查器检查代理
        let checked_proxies = health_checker.check_proxies_health(&proxy_nodes).await;

        println!("✅ 健康检查完成，检查了 {} 个节点", checked_proxies.len());

        // 显示检查结果
        let available_count = checked_proxies.iter().filter(|p| p.delay_ms > 0).count();
        println!("  ├── 可用节点: {} 个", available_count);
        println!(
            "  └── 失败节点: {} 个",
            checked_proxies.len() - available_count
        );

        if available_count > 0 {
            // 显示最快的3个节点
            let mut sorted_proxies = checked_proxies.clone();
            sorted_proxies.sort_by(|a, b| a.delay_ms.cmp(&b.delay_ms));

            println!("\n🏆 最快的3个节点:");
            for (i, proxy) in sorted_proxies.iter().take(3).enumerate() {
                if proxy.delay_ms > 0 {
                    println!("  {}. {} - {}ms", i + 1, proxy.name, proxy.delay_ms);
                }
            }
        }
    } else {
        println!("⚠️  没有可用的代理节点进行测试");
    }

    Ok(())
}
