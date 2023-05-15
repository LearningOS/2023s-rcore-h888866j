use super::{BlockDevice, BLOCK_SZ};
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
use spin::Mutex;
/// Cached block inside memory
pub struct BlockCache {
    /// cached block data
    cache: [u8; BLOCK_SZ],
    /// underlying block id
    block_id: usize,
    /// underlying block device
    block_device: Arc<dyn BlockDevice>,
    /// whether the block is dirty
    modified: bool,
}

impl BlockCache {
    /// Load a new BlockCache from disk.
    /// 
    /// 创建 BlockCache 时，将一个块从磁盘读到缓冲区 cache ：
    pub fn new(block_id: usize, block_device: Arc<dyn BlockDevice>) -> Self {
        let mut cache = [0u8; BLOCK_SZ];
        block_device.read_block(block_id, &mut cache);
        Self {
            cache,
            block_id,
            block_device,
            modified: false,
        }
    }
    /// Get the address of an offset inside the cached block data
    /// 用offset 在self.cache：`[u8,BLOCK_SZ]` 中索引出裸指针 转成usise返回
    /// 
    /// addr_of_offset 可以得到一个 BlockCache 内部的缓冲区中指定偏移量 offset 的字节地址；
    fn addr_of_offset(&self, offset: usize) -> usize {
        &self.cache[offset] as *const _ as usize
    }
    /// get ref, 根据传入的offset 找到cache缓冲区中offset地址， 转成裸指针解引用，再取引用
    /// 返回引用
    /// 
    /// get_ref 是一个泛型方法，它可以获取缓冲区中的位于偏移量 offset 的一个类型为 T 的磁盘上数据结构的不可变引用。
    /// 
    /// 传给它的闭包需要显式声明参数类型为  ，不然的话， 
    /// BlockCache 的泛型方法 get_ref 无法得知应该用哪个类型来解析块上的数据。
    pub fn get_ref<T>(&self, offset: usize) -> &T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        let addr = self.addr_of_offset(offset);
        unsafe { &*(addr as *const T) }
    }

    /// get_mut 是一个泛型方法，它可以获取缓冲区中的位于偏移量 offset 的一个类型为 T 的磁盘上数据结构的可变引用。
    ///
    /// 传给它的闭包需要显式声明参数类型为  ，不然的话， 
    /// BlockCache 的泛型方法 get_mut 无法得知应该用哪个类型来解析块上的数据。
    pub fn get_mut<T>(&mut self, offset: usize) -> &mut T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        self.modified = true;
        let addr = self.addr_of_offset(offset);
        unsafe { &mut *(addr as *mut T) }
    }

    /// BlockCache的read方法，
    /// 
    /// 在 BlockCache 缓冲区偏移量为 offset 的位置，获取一个类型为 T （闭包f形参）的 不可变/可变引用，
    /// 将闭包 f 作用于这个引用，返回 f 的返回值。 
    /// 
    /// 传给它的闭包需要显式声明参数类型为  ，不然的话， 
    /// BlockCache 的泛型方法 modify/get_mut 无法得知应该用哪个类型来解析块上的数据。
    pub fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get_ref(offset))
    }

    /// 在 BlockCache 缓冲区偏移量为 offset 的位置，获取一个类型为 T（闭包f形参）的 不可变/可变引用，
    /// 将闭包 f 作用于这个引用，返回 f 的返回值。 
    pub fn modify<T, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }

    ///  如果缓冲区修改过，写回磁盘
    pub fn sync(&mut self) {
        if self.modified {
            self.modified = false;
            self.block_device.write_block(self.block_id, &self.cache);
        }
    }
}

impl Drop for BlockCache {
    fn drop(&mut self) {
        self.sync()
    }
}
/// Use a block cache of 16 blocks
const BLOCK_CACHE_SIZE: usize = 16;

pub struct BlockCacheManager {
    queue: VecDeque<(usize, Arc<Mutex<BlockCache>>)>,
}

impl BlockCacheManager {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    ///get_block_cache 方法尝试从块缓存管理器中获取一个编号为 block_id 的块缓存，
    /// 如果找不到的话会读取磁盘，还有可能会发生缓存替换：
    /// 返回 `Arc<Mutex<BlockCache>>`
    pub fn get_block_cache(
        &mut self,
        block_id: usize,
        block_device: Arc<dyn BlockDevice>,
    ) -> Arc<Mutex<BlockCache>> {
        // 尝试冲 块缓存管理器中 找到 实参 block_id 对应的块缓存BlcokCache
        // 找到的话，返回这个块缓存的 Arc<Mutex<BlockCache>>
        // 找不到的话，就从磁盘读取到块缓存再返回
        if let Some(pair) = self.queue.iter().find(|pair| pair.0 == block_id) {
            Arc::clone(&pair.1)
        } else {
            // substitute
            // 块缓存管理器队列已满
            if self.queue.len() == BLOCK_CACHE_SIZE {
                // from front to tail
                if let Some((idx, _)) = self
                    .queue
                    .iter()
                    .enumerate()
                    .find(|(_, pair)| Arc::strong_count(&pair.1) == 1)
                {
                    self.queue.drain(idx..=idx);
                } else {
                    panic!("Run out of BlockCache!");
                }
            }
            // 创建一个新的块缓存（会触发 read_block 进行块读取）并加入到队尾，最后返回给请求者。
            // load block into mem and push back
            // BlockCache::new 会将 块 从磁盘读取到 内存BlockCache缓冲区中
            let block_cache = Arc::new(Mutex::new(BlockCache::new(
                block_id,
                Arc::clone(&block_device),
            )));
            self.queue.push_back((block_id, Arc::clone(&block_cache)));
            block_cache
        }
    }
}

lazy_static! {
    /// The global block cache manager
    pub static ref BLOCK_CACHE_MANAGER: Mutex<BlockCacheManager> =
        Mutex::new(BlockCacheManager::new());
}
/// Get the block cache corresponding to the given block id and block device
/// 
///get_block_cache 方法尝试从块缓存管理器中获取一个编号为 block_id 的块缓存，
/// 如果找不到的话会读取磁盘，还有可能会发生缓存替换：
/// 返回 `Arc<Mutex<BlockCache>>`
pub fn get_block_cache(
    device_block_id: usize,
    block_device: Arc<dyn BlockDevice>,
) -> Arc<Mutex<BlockCache>> {
    BLOCK_CACHE_MANAGER
        .lock()
        .get_block_cache(device_block_id, block_device)
}

/// Sync all block cache to block device
/// 
/// 检查块缓存全局管理器中所有块缓存，sync到磁盘（如果修改过）
pub fn block_cache_sync_all() {
    let manager = BLOCK_CACHE_MANAGER.lock();
    for (_, cache) in manager.queue.iter() {
        cache.lock().sync();
    }
}
