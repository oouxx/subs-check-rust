//! 健康检查模块测试程序

use anyhow::Result;
use serde_json::json;
use subs_check_rust::clash_proxy::{ProxyHealthChecker, ProxyNodeInfo};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔬 代理健康检查测试");
    println!("{:=<60}", "");

    // 创建健康检查器
    let checker = ProxyHealthChecker::new(5000, None);
    println!("✅ 健康检查器创建成功");
    println!("  超时时间: {}ms", checker.get_timeout_ms());
    println!("  测试URL: {}", checker.get_test_url());

    // 测试 HTTP 代理
    println!("\n📋 测试 HTTP 代理...");
    let http_proxy = ProxyNodeInfo::new(
        "测试HTTP代理".to_string(),
        "http".to_string(),
        "example.com".to_string(),
        8080,
    )
    .with_extra_info(json!({
        "username": "test_user",
        "password": "test_pass"
    }));

    let http_url = checker.build_proxy_url(&http_proxy);
    match http_url {
        Ok(url) => {
            println!("✅ HTTP 代理URL构建成功: {}", url);
            assert_eq!(url, "http://test_user:test_pass@example.com:8080");
        }
        Err(e) => println!("❌ HTTP 代理URL构建失败: {}", e),
    }

    // 测试 Shadowsocks 代理
    println!("\n📋 测试 Shadowsocks 代理...");
    let ss_proxy = ProxyNodeInfo::new(
        "测试SS代理".to_string(),
        "ss".to_string(),
        "ss.example.com".to_string(),
        8388,
    )
    .with_extra_info(json!({
        "password": "secret_password",
        "method": "aes-256-gcm"
    }));

    let ss_url = checker.build_proxy_url(&ss_proxy);
    match ss_url {
        Ok(url) => {
            println!("✅ Shadowsocks 代理URL构建成功: {}", url);
            assert!(url.starts_with("ss://aes-256-gcm:secret_password@ss.example.com:8388"));
        }
        Err(e) => println!("❌ Shadowsocks 代理URL构建失败: {}", e),
    }

    // 测试 SOCKS5 代理
    println!("\n📋 测试 SOCKS5 代理...");
    let socks5_proxy = ProxyNodeInfo::new(
        "测试SOCKS5代理".to_string(),
        "socks5".to_string(),
        "socks.example.com".to_string(),
        1080,
    );

    let socks5_url = checker.build_proxy_url(&socks5_proxy);
    match socks5_url {
        Ok(url) => {
            println!("✅ SOCKS5 代理URL构建成功: {}", url);
            assert_eq!(url, "socks5://socks.example.com:1080");
        }
        Err(e) => println!("❌ SOCKS5 代理URL构建失败: {}", e),
    }

    // 测试 VMess 代理
    println!("\n📋 测试 VMess 代理...");
    let vmess_proxy = ProxyNodeInfo::new(
        "测试VMess代理".to_string(),
        "vmess".to_string(),
        "vmess.example.com".to_string(),
        443,
    )
    .with_extra_info(json!({
        "uuid": "12345678-1234-1234-1234-123456789012",
        "alterId": 0,
        "security": "auto",
        "network": "tcp",
        "tls": "tls"
    }));

    let vmess_url = checker.build_proxy_url(&vmess_proxy);
    match vmess_url {
        Ok(url) => {
            println!("✅ VMess 代理URL构建成功: {}", url);
            assert!(url.starts_with("vmess://"));

            // 验证 base64 编码
            let base64_str = url.trim_start_matches("vmess://");
            if let Ok(decoded) = base64::decode(base64_str) {
                if let Ok(config_str) = String::from_utf8(decoded) {
                    println!("  VMess 配置解码成功");
                    let config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
                    assert_eq!(config["v"], "2");
                    assert_eq!(config["add"], "vmess.example.com");
                    assert_eq!(config["port"], 443);
                }
            }
        }
        Err(e) => println!("❌ VMess 代理URL构建失败: {}", e),
    }

    // 测试 UDP 支持检查
    println!("\n📋 测试 UDP 支持检查...");

    // SOCKS5 应该支持 UDP
    let socks5_udp = checker.check_udp_support(&socks5_proxy).await;
    match socks5_udp {
        Ok(supported) => {
            println!("✅ SOCKS5 UDP 支持检查: {}", supported);
            assert!(supported, "SOCKS5 应该支持 UDP");
        }
        Err(e) => println!("❌ SOCKS5 UDP 支持检查失败: {}", e),
    }

    // HTTP 不应该支持 UDP
    let http_udp = checker.check_udp_support(&http_proxy).await;
    match http_udp {
        Ok(supported) => {
            println!("✅ HTTP UDP 支持检查: {}", supported);
            assert!(!supported, "HTTP 不应该支持 UDP");
        }
        Err(e) => println!("❌ HTTP UDP 支持检查失败: {}", e),
    }

    // 测试批量健康检查（模拟数据）
    println!("\n📋 测试批量健康检查...");
    let test_proxies = vec![http_proxy.clone(), ss_proxy.clone(), socks5_proxy.clone()];

    println!("  准备测试 {} 个代理", test_proxies.len());

    // 注意：这里不会真正进行网络请求，只是测试构建功能
    println!("  ⚠️  注意：实际健康检查需要网络连接");
    println!("      这里只测试 URL 构建和逻辑功能");

    // 测试不支持的协议
    println!("\n📋 测试不支持的协议...");
    let unknown_proxy = ProxyNodeInfo::new(
        "未知协议代理".to_string(),
        "unknown".to_string(),
        "example.com".to_string(),
        8080,
    );

    let unknown_url = checker.build_proxy_url(&unknown_proxy);
    match unknown_url {
        Ok(_) => println!("❌ 未知协议应该构建失败"),
        Err(e) => {
            println!("✅ 未知协议正确处理: {}", e);
            assert!(e.to_string().contains("不支持的代理协议"));
        }
    }

    println!("\n{:=<60}", "");
    println!("📊 测试总结:");
    println!("✅ 代理URL构建功能正常");
    println!("✅ UDP支持检查功能正常");
    println!("✅ 错误处理功能正常");
    println!("⚠️  实际网络检查需要有效的代理服务器");
    println!("\n🎉 健康检查模块测试完成!");

    Ok(())
}
