//! 简化缓存实现
//! 提供基本的缓存功能

use std::sync::Arc;
use tokio::sync::RwLock;

/// 简单的内存缓存存储
#[derive(Debug, Clone)]
pub struct SimpleCacheStore {
    data: std::collections::HashMap<String, Vec<u8>>,
}

impl SimpleCacheStore {
    /// 创建新的缓存存储
    pub fn new() -> Self {
        Self {
            data: std::collections::HashMap::new(),
        }
    }

    /// 获取缓存数据
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.data.get(key).cloned()
    }

    /// 设置缓存数据
    pub fn set(&mut self, key: String, value: Vec<u8>) {
        self.data.insert(key, value);
    }

    /// 删除缓存数据
    pub fn remove(&mut self, key: &str) -> Option<Vec<u8>> {
        self.data.remove(key)
    }

    /// 清空缓存
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

/// 线程安全的缓存文件类型别名
pub type ThreadSafeCacheFile = Arc<RwLock<SimpleCacheStore>>;

/// 创建线程安全的缓存实例
pub fn create_simple_cache_store() -> ThreadSafeCacheFile {
    Arc::new(RwLock::new(SimpleCacheStore::new()))
}
