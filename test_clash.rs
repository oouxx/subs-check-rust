//! Clash代理功能测试程序
//!
//! 这个程序用于测试clash_proxy模块的功能，包括：
//! 1. 配置文件解析
//! 2. 代理节点健康检查
//! 3. 代理管理器功能

use anyhow::Result;
use std::path::PathBuf;
use subs_check_rust::clash_proxy::{ClashProxyManager, ConfigParser};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Clash代理功能测试程序");
    println!("{:=<60}", "");

    // 1. 测试配置文件解析
    test_config_parser()?;

    // 2. 测试代理管理器
    test_proxy_manager().await?;

    println!("\n🎉 所有测试完成！");
    Ok(())
}

/// 测试配置文件解析功能
fn test_config_parser() -> Result<()> {
    println!("\n📄 测试配置文件解析...");

    // 创建配置文件路径
    let config_path = PathBuf::from("./sample-tiny.yaml");

    if !config_path.exists() {
        println!("⚠️  配置文件不存在: {:?}", config_path);
        println!("📝 使用内置示例数据测试...");

        // 创建示例配置
        let example_config = r#"
proxies:
  - name: "测试节点1"
    type: "vless"
    server: "example.com"
    port: 443
    uuid: "12345678-1234-1234-1234-123456789012"
    network: "ws"
    tls: true
    sni: "example.com"
    udp: true

  - name: "测试节点2"
    type: "trojan"
    server: "trojan.example.com"
    port: 443
    password: "password123"
    sni: "trojan.example.com"
    udp: false
"#;

        // 创建配置解析器
        let mut parser = ConfigParser::new("test_config.yaml");

        // 解析配置
        match parser.parse_config(example_config) {
            Ok(_) => {
                println!("✅ 配置文件解析成功");

                // 获取解析结果
                let proxies = parser.get_proxies();
                println!("📊 解析到 {} 个代理节点", proxies.len());

                for (i, proxy) in proxies.iter().enumerate() {
                    println!(
                        "  {}. {} ({}) - {}:{}",
                        i + 1,
                        proxy.name,
                        proxy.proto,
                        proxy.server,
                        proxy.port
                    );

                    // 显示额外信息
                    if let Some(extra_info) = &proxy.extra_info {
                        println!("     额外信息: {:?}", extra_info);
                    }

                    // 显示UDP支持
                    println!(
                        "     UDP支持: {}",
                        if proxy.support_udp { "✅" } else { "❌" }
                    );
                }
            }
            Err(e) => {
                println!("❌ 配置文件解析失败: {}", e);
            }
        }
    } else {
        println!("📁 使用配置文件: {:?}", config_path);

        // 创建配置解析器
        let mut parser = ConfigParser::new(config_path.to_str().unwrap());

        // 加载配置文件
        match parser.load_from_file() {
            Ok(_) => {
                println!("✅ 配置文件加载成功");

                // 获取解析结果
                let proxies = parser.get_proxies();
                println!("📊 解析到 {} 个代理节点", proxies.len());

                for (i, proxy) in proxies.iter().enumerate() {
                    println!(
                        "  {}. {} ({}) - {}:{}",
                        i + 1,
                        proxy.name,
                        proxy.proto,
                        proxy.server,
                        proxy.port
                    );
                }
            }
            Err(e) => {
                println!("❌ 配置文件加载失败: {}", e);
            }
        }
    }

    Ok(())
}

/// 测试代理管理器功能
async fn test_proxy_manager() -> Result<()> {
    println!("\n🔧 测试代理管理器...");

    // 创建配置文件路径
    let config_path = PathBuf::from("./sample-tiny.yaml");

    if !config_path.exists() {
        println!("⚠️  配置文件不存在，跳过代理管理器测试");
        return Ok(());
    }

    println!("📁 使用配置文件: {:?}", config_path);

    // 创建代理管理器
    match ClashProxyManager::from_config_file(&config_path).await {
        Ok(manager) => {
            println!("✅ 代理管理器初始化成功");

            // 获取所有代理节点
            match manager.get_all_proxy_nodes().await {
                Ok(proxies) => {
                    println!("📊 共加载 {} 个代理节点", proxies.len());

                    // 显示代理节点信息
                    for (i, proxy) in proxies.iter().enumerate() {
                        println!("\n  {}. {}", i + 1, proxy.name);
                        println!("     ├── 协议: {}", proxy.proto);
                        println!("     ├── 地址: {}:{}", proxy.server, proxy.port);
                        println!(
                            "     ├── UDP支持: {}",
                            if proxy.support_udp { "✅" } else { "❌" }
                        );
                        println!("     └── 延迟: {}", proxy.get_delay_description());

                        // 显示额外信息
                        if let Some(extra_info) = &proxy.extra_info {
                            if let serde_json::Value::Object(obj) = extra_info {
                                for (key, value) in obj {
                                    println!("        {}: {}", key, value);
                                }
                            }
                        }
                    }

                    // 获取健康检查统计
                    let (total, available, success_rate) = manager.get_health_stats();
                    println!("\n📈 健康检查统计:");
                    println!("  ├── 总节点数: {}", total);
                    println!("  ├── 可用节点: {}", available);
                    println!("  └── 成功率: {:.1}%", success_rate);

                    // 获取可用代理节点
                    let available_proxies = manager.get_available_proxies();
                    if !available_proxies.is_empty() {
                        println!("\n🏆 可用节点列表:");
                        for (i, proxy) in available_proxies.iter().enumerate() {
                            println!("  {}. {} - {}ms", i + 1, proxy.name, proxy.delay_ms);
                        }
                    } else {
                        println!("\n⚠️  没有可用节点");
                    }

                    // 获取按延迟排序的节点
                    let sorted_proxies = manager.get_sorted_by_delay();
                    if !sorted_proxies.is_empty() {
                        println!("\n📊 按延迟排序:");
                        for (i, proxy) in sorted_proxies.iter().enumerate() {
                            if proxy.is_available() {
                                println!("  {}. {} - {}ms", i + 1, proxy.name, proxy.delay_ms);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("❌ 获取代理节点失败: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ 代理管理器初始化失败: {}", e);
        }
    }

    Ok(())
}

/// 测试健康检查功能
async fn test_health_check() -> Result<()> {
    println!("\n🏥 测试健康检查功能...");

    // 创建示例代理节点
    use subs_check_rust::clash_proxy::types::ProxyNodeInfo;

    let test_proxies = vec![
        ProxyNodeInfo::new(
            "测试HTTP代理".to_string(),
            "http".to_string(),
            "proxy.example.com".to_string(),
            8080,
        ),
        ProxyNodeInfo::new(
            "测试SOCKS5代理".to_string(),
            "socks5".to_string(),
            "socks.example.com".to_string(),
            1080,
        ),
    ];

    println!("📊 创建了 {} 个测试代理节点", test_proxies.len());

    // 注意：实际健康检查需要网络连接，这里只显示结构
    for proxy in &test_proxies {
        println!(
            "  - {} ({}) - {}:{}",
            proxy.name, proxy.proto, proxy.server, proxy.port
        );
    }

    println!("ℹ️  实际健康检查需要网络连接，这里只显示测试结构");

    Ok(())
}

// 运行测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parser_basic() {
        // 测试基本配置解析
        let config = r#"
proxies:
  - name: "测试节点"
    type: "vless"
    server: "example.com"
    port: 443
"#;

        let mut parser = ConfigParser::new("test.yaml");
        let result = parser.parse_config(config);
        assert!(result.is_ok(), "配置解析应该成功");

        let proxies = parser.get_proxies();
        assert_eq!(proxies.len(), 1, "应该解析到1个代理节点");

        let proxy = &proxies[0];
        assert_eq!(proxy.name, "测试节点");
        assert_eq!(proxy.proto, "vless");
        assert_eq!(proxy.server, "example.com");
        assert_eq!(proxy.port, 443);
    }

    #[test]
    fn test_proxy_node_info() {
        // 测试代理节点信息结构
        let proxy = ProxyNodeInfo::new(
            "测试节点".to_string(),
            "vless".to_string(),
            "example.com".to_string(),
            443,
        );

        assert_eq!(proxy.name, "测试节点");
        assert_eq!(proxy.proto, "vless");
        assert_eq!(proxy.server, "example.com");
        assert_eq!(proxy.port, 443);
        assert!(!proxy.support_udp); // 默认不支持UDP
        assert_eq!(proxy.delay_ms, 0); // 默认延迟为0

        // 测试地址获取
        assert_eq!(proxy.get_address(), "example.com:443");

        // 测试延迟描述
        assert_eq!(proxy.get_delay_description(), "未检测");

        // 测试可用性检查
        assert!(!proxy.is_available()); // 延迟为0，不可用

        // 测试带延迟的节点
        let proxy_with_delay = proxy.with_delay(100);
        assert_eq!(proxy_with_delay.delay_ms, 100);
        assert!(proxy_with_delay.is_available()); // 延迟100ms，可用

        // 测试带UDP支持的节点
        let proxy_with_udp = proxy.with_udp_support(true);
        assert!(proxy_with_udp.support_udp);
    }
}
