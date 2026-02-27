use anyhow::Result;
use std::path::PathBuf;

mod clash_proxy;
use clash_proxy::{ClashProxyManager, ProxyHealthChecker};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 测试 Clash 代理健康检查实现");
    println!("{:=<60}", "");

    // 1. 测试配置文件路径
    let config_path = PathBuf::from("./sample-tiny.yaml");
    if !config_path.exists() {
        eprintln!("❌ 配置文件不存在: {:?}", config_path);
        return Ok(());
    }

    println!("📁 配置文件: {:?}", config_path);

    // 2. 初始化 Clash 代理管理器
    println!("\n🔄 初始化 Clash 代理管理器...");
    let clash_manager = match ClashProxyManager::from_config_file(&config_path).await {
        Ok(manager) => {
            println!("✅ 代理管理器初始化成功");
            manager
        }
        Err(e) => {
            eprintln!("❌ 代理管理器初始化失败: {}", e);
            return Ok(());
        }
    };

    // 3. 获取所有代理节点信息
    println!("\n📄 获取代理节点信息...");
    let proxy_nodes = match clash_manager.get_all_proxy_nodes().await {
        Ok(nodes) => {
            println!("✅ 获取到 {} 个代理节点", nodes.len());
            nodes
        }
        Err(e) => {
            eprintln!("❌ 获取代理节点失败: {}", e);
            return Ok(());
        }
    };

    // 4. 显示代理节点信息
    println!("\n📋 代理节点列表:");
    for (idx, node) in proxy_nodes.iter().enumerate() {
        println!("\n[{}/{}] {}", idx + 1, proxy_nodes.len(), node.name);
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
                "❌ 未检测".to_string()
            }
        );
    }

    // 5. 创建健康检查器
    println!("\n🔬 创建健康检查器...");
    let health_checker = ProxyHealthChecker::new(5000, None);
    println!("  ├── 超时时间: {}ms", health_checker.get_timeout_ms());
    println!("  ├── 测试URL: {}", health_checker.get_test_url());

    // 6. 执行健康检查
    println!("\n⚡ 执行健康检查...");
    let checked_proxies = health_checker.check_proxies_health(&proxy_nodes).await;

    // 7. 显示健康检查结果
    println!("\n📊 健康检查结果:");
    let available_count = checked_proxies.iter().filter(|p| p.delay_ms > 0).count();
    let failed_count = checked_proxies.len() - available_count;

    println!("  ├── 总节点数: {}", checked_proxies.len());
    println!("  ├── 可用节点: {} 个", available_count);
    println!("  ├── 失败节点: {} 个", failed_count);
    println!(
        "  └── 成功率: {:.1}%",
        if checked_proxies.len() > 0 {
            (available_count as f64 / checked_proxies.len() as f64) * 100.0
        } else {
            0.0
        }
    );

    // 8. 显示可用节点（按延迟排序）
    if available_count > 0 {
        let mut sorted_proxies = checked_proxies.clone();
        sorted_proxies.sort_by(|a, b| a.delay_ms.cmp(&b.delay_ms));

        println!("\n🏆 可用节点（按延迟排序）:");
        for (i, proxy) in sorted_proxies.iter().enumerate() {
            if proxy.delay_ms > 0 {
                println!(
                    "  {}. {} - {}ms ({})",
                    i + 1,
                    proxy.name,
                    proxy.delay_ms,
                    proxy.get_delay_description()
                );
            }
        }

        // 显示最快的3个节点
        println!("\n⚡ 最快的3个节点:");
        for (i, proxy) in sorted_proxies.iter().take(3).enumerate() {
            if proxy.delay_ms > 0 {
                println!("  {}. {} - {}ms", i + 1, proxy.name, proxy.delay_ms);
            }
        }
    }

    // 9. 显示健康检查统计
    println!("\n📈 健康检查统计:");
    let (total, available, success_rate) = clash_manager.get_health_stats();
    println!("  ├── 总节点数: {}", total);
    println!("  ├── 可用节点: {}", available);
    println!("  └── 成功率: {:.1}%", success_rate);

    // 10. 测试节点可用性检查
    println!("\n✅ 测试节点可用性检查:");
    for proxy in &proxy_nodes {
        println!(
            "  {} - {}: {}",
            proxy.name,
            proxy.get_address(),
            if proxy.is_available() {
                "✅ 可用"
            } else {
                "❌ 不可用"
            }
        );
    }

    println!("\n🎉 测试完成!");
    println!("{:=<60}", "");

    Ok(())
}
