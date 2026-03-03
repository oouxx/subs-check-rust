//! 代理模块封装
//! 基于clash-rs理念的代理管理接口，实现代理健康检查和配置解析功能

// 导入子模块
mod advanced_health_check;
mod cache;
mod config_parser;
mod health_check;
mod manager;
mod types;

// 对外暴露公共类型和结构体（仅导出需要外部使用的部分）
pub use advanced_health_check::{
    AdvancedHealthChecker, DelayHistory, HealthCheckConfig, HealthCheckResult, ProxyState,
    check_proxies_health_batch, check_proxies_health_with_config,
};
pub use config_parser::{
    ClashConfig, ConfigParser, DnsConfig, HealthCheckConfig as ParserHealthCheckConfig, ProxyGroup,
    ProxyGroupType, ProxyProvider,
};
pub use health_check::ProxyHealthChecker;
pub use manager::ClashProxyManager;
pub use types::ProxyNodeInfo;

// 可选：导出内部错误类型（若需要对外暴露）
pub type ClashProxyResult<T = ()> = anyhow::Result<T>;
