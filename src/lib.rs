use std::cell::UnsafeCell;
use std::hash::{BuildHasher, Hasher};
use std::sync::Condvar;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct ConcurrentMap<V> {
    map: UnsafeCell<std::collections::HashMap<u64, V, NoHashBuilder>>,
    pub readers: AtomicUsize,       // 当前正在读的线程数
    pub write_pending: AtomicBool,  // 是否有写操作等待
    pub lock: std::sync::Mutex<()>, // 用于写等待所有读线程退出
    pub condvar: Condvar,           // 用于通知
}

unsafe impl<V: Send> Send for ConcurrentMap<V> {}
unsafe impl<V: Send> Sync for ConcurrentMap<V> {}

impl<V> ConcurrentMap<V> {
    pub fn new() -> Self {
        Self {
            map: UnsafeCell::new(std::collections::HashMap::with_hasher(NoHashBuilder)),
            readers: AtomicUsize::new(0),
            write_pending: AtomicBool::new(false),
            lock: std::sync::Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    /// 读初始化，若有写等待则阻塞
    pub fn read_init(&self) {
        loop {
            if !self.write_pending.load(Ordering::Acquire) {
                self.readers.fetch_add(1, Ordering::AcqRel);
                if !self.write_pending.load(Ordering::Acquire) {
                    break;
                } else {
                    self.readers.fetch_sub(1, Ordering::AcqRel);
                }
            }
            let mut guard = self.lock.lock().unwrap();
            while self.write_pending.load(Ordering::Acquire) {
                guard = self.condvar.wait(guard).unwrap();
            }
        }
    }

    /// 读结束
    pub fn read_done(&self) {
        let prev = self.readers.fetch_sub(1, Ordering::AcqRel);
        if prev == 1 && self.write_pending.load(Ordering::Acquire) {
            let guard = self.lock.lock().unwrap();
            self.condvar.notify_all();
            drop(guard);
        }
    }

    /// 写初始化，通知所有读线程停止读取，并等待所有读线程退出
    pub fn write_init(&self) {
        self.write_pending.store(true, Ordering::Release);
        let mut guard = self.lock.lock().unwrap();
        while self.readers.load(Ordering::Acquire) > 0 {
            guard = self.condvar.wait(guard).unwrap();
        }
    }

    /// 写结束，允许读线程继续
    pub fn write_done(&self) {
        self.write_pending.store(false, Ordering::Release);
        let guard = self.lock.lock().unwrap();
        self.condvar.notify_all();
        drop(guard);
    }

    /// 读操作（需配合 read_init/read_done）
    pub fn get<F, R>(&self, key: u64, f: F) -> R
    where
        F: FnOnce(Option<&V>) -> R,
    {
        // 安全：通知机制保证没有写入
        let map = unsafe { &*self.map.get() };
        f(map.get(&key))
    }

    /// 写操作（需配合 write_init/write_done）
    pub fn insert(&self, key: u64, value: V) {
        // 安全：通知机制保证没有读
        let map = unsafe { &mut *self.map.get() };
        map.insert(key, value);
    }
}

// NoHashHasher: 直接用u64 key本身作为哈希值
#[derive(Default, Clone)]
pub struct NoHashHasher {
    hash: u64,
}

impl Hasher for NoHashHasher {
    fn write(&mut self, bytes: &[u8]) {
        // 只支持8字节key
        if bytes.len() == 8 {
            self.hash = u64::from_le_bytes(bytes.try_into().unwrap());
        } else {
            panic!("NoHashHasher 只支持8字节key");
        }
    }
    fn finish(&self) -> u64 {
        self.hash
    }
}

#[derive(Default, Clone)]
pub struct NoHashBuilder;

impl BuildHasher for NoHashBuilder {
    type Hasher = NoHashHasher;
    fn build_hasher(&self) -> Self::Hasher {
        NoHashHasher::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nohash_hasher_u64() {
        let mut hasher = NoHashHasher::default();
        let key: u64 = 0x1234_5678_9abc_def0;
        hasher.write(&key.to_le_bytes());
        assert_eq!(hasher.finish(), key);
    }
}
