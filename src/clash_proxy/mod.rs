//! 代理模块封装
//! 对外提供简洁的代理管理接口，实现代理健康检查功能

// 导入子模块
mod cache;
mod health_check;
mod manager;
mod types;

// 对外暴露公共类型和结构体（仅导出需要外部使用的部分）
pub use health_check::ProxyHealthChecker;
pub use manager::ClashProxyManager;
pub use types::ProxyNodeInfo;

// 可选：导出内部错误类型（若需要对外暴露）
pub type ClashProxyResult<T = ()> = anyhow::Result<T>;
