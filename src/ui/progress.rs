use crate::config::Config;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Clone)]
pub struct ProgressTracker {
    multi_progress: Option<Arc<MultiProgress>>,
    total_progress: Option<ProgressBar>,
    alive_progress: Option<ProgressBar>,
    speed_progress: Option<ProgressBar>,
    media_progress: Option<ProgressBar>,
    total_nodes: Arc<AtomicU64>,
    alive_nodes: Arc<AtomicU64>,
    speed_nodes: Arc<AtomicU64>,
    media_nodes: Arc<AtomicU64>,
    checked_nodes: Arc<AtomicU64>,
}

impl ProgressTracker {
    pub fn new(config: &Config) -> Self {
        if !config.print_progress {
            return Self {
                multi_progress: None,
                total_progress: None,
                alive_progress: None,
                speed_progress: None,
                media_progress: None,
                total_nodes: Arc::new(AtomicU64::new(0)),
                alive_nodes: Arc::new(AtomicU64::new(0)),
                speed_nodes: Arc::new(AtomicU64::new(0)),
                media_nodes: Arc::new(AtomicU64::new(0)),
                checked_nodes: Arc::new(AtomicU64::new(0)),
            };
        }

        let multi_progress = Arc::new(MultiProgress::new());

        // 总进度条
        let total_style = ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}",
        )
        .unwrap()
        .progress_chars("#>-");

        let total_progress = multi_progress.add(ProgressBar::new(0));
        total_progress.set_style(total_style.clone());

        // 存活检测进度
        let alive_progress = multi_progress.add(ProgressBar::new(0));
        alive_progress.set_style(
            ProgressStyle::with_template(
                "  {spinner:.yellow} 存活检测: [{bar:30.yellow}] {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
        );

        // 测速进度（如果启用）
        let speed_progress = multi_progress.add(ProgressBar::new(0));
        speed_progress.set_style(
            ProgressStyle::with_template(
                "  {spinner:.green} 测速检测: [{bar:30.green}] {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
        );

        // 媒体检测进度（如果启用）
        let media_progress = multi_progress.add(ProgressBar::new(0));
        media_progress.set_style(
            ProgressStyle::with_template(
                "  {spinner:.blue} 媒体检测: [{bar:30.blue}] {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
        );

        Self {
            multi_progress: Some(multi_progress),
            total_progress: Some(total_progress),
            alive_progress: Some(alive_progress),
            speed_progress: Some(speed_progress),
            media_progress: Some(media_progress),
            total_nodes: Arc::new(AtomicU64::new(0)),
            alive_nodes: Arc::new(AtomicU64::new(0)),
            speed_nodes: Arc::new(AtomicU64::new(0)),
            media_nodes: Arc::new(AtomicU64::new(0)),
            checked_nodes: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn set_total_nodes(&self, total: u64) {
        self.total_nodes.store(total, Ordering::Relaxed);

        if let Some(pb) = &self.total_progress {
            pb.set_length(total);
            pb.set_position(0);
        }

        if let Some(pb) = &self.alive_progress {
            pb.set_length(total);
            pb.set_position(0);
        }

        if let Some(pb) = &self.speed_progress {
            pb.set_length(total);
            pb.set_position(0);
        }

        if let Some(pb) = &self.media_progress {
            pb.set_length(total);
            pb.set_position(0);
        }
    }

    pub fn increment_alive(&self, success: bool) {
        self.alive_nodes.fetch_add(1, Ordering::Relaxed);
        self.checked_nodes.fetch_add(1, Ordering::Relaxed);

        if let Some(pb) = &self.alive_progress {
            pb.inc(1);
            if success {
                pb.set_message("✅");
            } else {
                pb.set_message("❌");
            }
        }

        if let Some(pb) = &self.total_progress {
            pb.inc(1);
        }
    }

    pub fn increment_speed(&self, success: bool) {
        self.speed_nodes.fetch_add(1, Ordering::Relaxed);

        if let Some(pb) = &self.speed_progress {
            pb.inc(1);
            if success {
                pb.set_message("✅");
            } else {
                pb.set_message("❌");
            }
        }
    }

    pub fn increment_media(&self, success: bool) {
        self.media_nodes.fetch_add(1, Ordering::Relaxed);

        if let Some(pb) = &self.media_progress {
            pb.inc(1);
            if success {
                pb.set_message("✅");
            } else {
                pb.set_message("❌");
            }
        }
    }

    pub fn finish_alive_stage(&self) {
        if let Some(pb) = &self.alive_progress {
            pb.finish_with_message("完成");
        }
    }

    pub fn finish_speed_stage(&self) {
        if let Some(pb) = &self.speed_progress {
            pb.finish_with_message("完成");
        }
    }

    pub fn finish_media_stage(&self) {
        if let Some(pb) = &self.media_progress {
            pb.finish_with_message("完成");
        }
    }

    pub fn finalize(&self) {
        if let Some(pb) = &self.total_progress {
            let total = self.total_nodes.load(Ordering::Relaxed);
            pb.set_position(total);
            pb.finish_with_message("检测完成");
        }
    }

    pub fn get_stats(&self) -> ProgressStats {
        ProgressStats {
            total: self.total_nodes.load(Ordering::Relaxed),
            alive: self.alive_nodes.load(Ordering::Relaxed),
            speed: self.speed_nodes.load(Ordering::Relaxed),
            media: self.media_nodes.load(Ordering::Relaxed),
            checked: self.checked_nodes.load(Ordering::Relaxed),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.multi_progress.is_some()
    }
}

pub struct ProgressStats {
    pub total: u64,
    pub alive: u64,
    pub speed: u64,
    pub media: u64,
    pub checked: u64,
}

impl ProgressStats {
    pub fn success_rate(&self) -> f64 {
        if self.total > 0 {
            (self.alive as f64 / self.total as f64) * 100.0
        } else {
            0.0
        }
    }

    pub fn speed_rate(&self) -> f64 {
        if self.alive > 0 {
            (self.speed as f64 / self.alive as f64) * 100.0
        } else {
            0.0
        }
    }

    pub fn media_rate(&self) -> f64 {
        if self.alive > 0 {
            (self.media as f64 / self.alive as f64) * 100.0
        } else {
            0.0
        }
    }
}

// 简单的控制台进度显示（无依赖版本）
pub struct SimpleProgress {
    total: u64,
    current: u64,
    start_time: std::time::Instant,
}

impl SimpleProgress {
    pub fn new(total: u64) -> Self {
        Self {
            total,
            current: 0,
            start_time: std::time::Instant::now(),
        }
    }

    pub fn update(&mut self, increment: u64) {
        self.current += increment;
        self.print();
    }

    pub fn print(&self) {
        let percentage = (self.current as f64 / self.total as f64) * 100.0;
        let elapsed = self.start_time.elapsed();

        // 计算估计剩余时间
        let estimated_total = if self.current > 0 {
            Duration::from_secs_f64(elapsed.as_secs_f64() * self.total as f64 / self.current as f64)
        } else {
            Duration::from_secs(0)
        };

        let remaining = estimated_total.saturating_sub(elapsed);

        let bar_length = 50;
        let filled = (percentage * bar_length as f64 / 100.0) as usize;
        let bar = "█".repeat(filled) + &" ".repeat(bar_length - filled);

        println!(
            "\r[{bar}] {:.1}% | {}/{} | 耗时: {:?} | 剩余: {:?}",
            percentage, self.current, self.total, elapsed, remaining
        );
    }

    pub fn finish(&self) {
        let elapsed = self.start_time.elapsed();
        println!(
            "\n✅ 检测完成! 总耗时: {:?}, 平均每个节点: {:?}",
            elapsed,
            if self.total > 0 {
                elapsed / self.total as u32
            } else {
                Duration::from_secs(0)
            }
        );
    }
}
