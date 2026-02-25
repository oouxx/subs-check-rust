use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub name: String,
    pub url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // 进度显示
    pub print_progress: bool,
    pub progress_mode: String,

    // 计划任务
    pub check_interval: u32,

    // 检测参数
    pub alive_concurrent: usize,
    pub speed_concurrent: usize,
    pub media_concurrent: usize,
    pub ipv6: bool,
    pub timeout: u64,
    pub concurrent: usize,
    pub success_limit: usize,
    pub keep_success_proxies: bool,

    // 下载参数
    pub min_speed: f64,
    pub download_timeout: u64,
    pub download_mb: u64,
    pub total_speed_limit: u64,
    pub speed_test_url: Option<String>,
    pub threshold: f64,

    // 媒体解锁检测
    pub media_check: bool,

    // 订阅配置
    pub subscriptions: Vec<Subscription>,

    // 输出配置
    pub output_dir: String,
    pub output_format: String,
    pub generate_clash_config: bool,
    pub generate_singbox_config: bool,

    // 日志配置
    pub log_level: String,
    pub log_file: Option<String>,

    // 代理配置
    pub system_proxy: Option<String>,
    pub github_proxy: Option<String>,

    // 高级配置
    pub drop_bad_cf_nodes: bool,
    pub sub_urls_stats: bool,
    pub gc_threshold: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            print_progress: true,
            progress_mode: "auto".to_string(),
            check_interval: 720,
            alive_concurrent: 10,
            speed_concurrent: 4,
            media_concurrent: 10,
            ipv6: false,
            timeout: 6000,
            concurrent: 20,
            success_limit: 200,
            keep_success_proxies: true,
            min_speed: 128.0,
            download_timeout: 10,
            download_mb: 20,
            total_speed_limit: 0,
            speed_test_url: Some(
                "https://github.com/2dust/v2rayN/releases/download/7.16.2/v2rayN-windows-64-SelfContained.zip".to_string()
            ),
            threshold: 0.75,
            media_check: true,
            subscriptions: vec![],
            output_dir: "./output".to_string(),
            output_format: "both".to_string(),
            generate_clash_config: true,
            generate_singbox_config: true,
            log_level: "info".to_string(),
            log_file: None,
            system_proxy: None,
            github_proxy: None,
            drop_bad_cf_nodes: false,
            sub_urls_stats: true,
            gc_threshold: 5000,
        }
    }
}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn is_speed_test_enabled(&self) -> bool {
        self.speed_test_url.is_some()
    }

    pub fn is_media_check_enabled(&self) -> bool {
        self.media_check
    }

    pub fn get_timeout_duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.timeout)
    }
}
