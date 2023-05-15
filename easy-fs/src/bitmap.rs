use super::{get_block_cache, BlockDevice, BLOCK_SZ};
use alloc::sync::Arc;
/// A bitmap block
type BitmapBlock = [u64; 64];
/// Number of bits in a block
const BLOCK_BITS: usize = BLOCK_SZ * 8;
/// A bitmap
pub struct Bitmap {
    start_block_id: usize,
    blocks: usize,
}

/// Decompose bits into (block_pos, bits64_pos, inner_pos)
fn decomposition(mut bit: usize) -> (usize, usize, usize) {
    let block_pos = bit / BLOCK_BITS;
    bit %= BLOCK_BITS;
    (block_pos, bit / 64, bit % 64)
}

impl Bitmap {
    /// A new bitmap from start block id and number of blocks
    pub fn new(start_block_id: usize, blocks: usize) -> Self {
        Self {
            start_block_id,
            blocks,
        }
    }
    /// Allocate a new block from a block device
    /// 
    /// 返回 分配来的空闲块的 在 inode_area_block / data_area_block 中的 id
    /// 
    /// 可能会分配失败
    /// 
    /// 其主要思路是遍历区域中的每个块，再在每个块中以bit组（每组 64 bits）为单位进行遍历，
    /// 找到一个尚未被全部分配出去的组，最后在里面分配一个bit。它将会返回分配的bit所在的位置，
    /// 等同于索引节点/数据块的编号。如果所有bit均已经被分配出去了，则返回 None 。
    pub fn alloc(&self, block_device: &Arc<dyn BlockDevice>) -> Option<usize> {
        // 根据位图拥有的block数量，遍历 位图 所有 block，每次得到一个 nth_of_block_in_bitmap
        for nth_of_block_in_bitmap in 0..self.blocks {
            let pos = get_block_cache(
                // 计算 block_id , 因为块缓存管理器对列里 存储的是（block_id,Arc<Mutex<BlockCache>>）元组
                // 要根据 block_id 去取出 BlockCache，如果不存在的话会触发 磁盘读取 这个块 到内存缓存管理器
                nth_of_block_in_bitmap + self.start_block_id as usize,
                Arc::clone(block_device),
            )
            .lock()
            // 这里block cache里存储的从磁盘读取到内存的 Bitmap 
            // 总结一下，这里 modify 的含义就是：从缓冲区偏移量为 0 的位置开始将一段连续的数据（数据的长度随具体类型而定）
            // 解析为一个 BitmapBlock 并要对该数据结构进行修改。在闭包内部，我们可以使用这个 BitmapBlock 的可变引用
            //  bitmap_block 对它进行访问。 read/get_ref 的用法完全相同，后面将不再赘述。
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                if let Some((bits64_pos, inner_pos)) = bitmap_block
                    .iter()
                    .enumerate()
                    // bitmap_block 是 [u64;64], 从中找到一个还未分配满的/不到最大值的 u64 
                    .find(|(_, bits64)| **bits64 != u64::MAX)
                    // bits64_pos 是 bitmap_block:[u64;64]中某个 u64 所在下标
                    // map 返回 这个 下标 和 u64里trailing_ones 组成的元组
                    .map(|(bits64_pos, bits64)| (bits64_pos, bits64.trailing_ones() as usize))
                {
                    // modify cache
                    // 其主要思路是遍历区域中的每个块，再在每个块中以bit组（每组 64 bits）为单位进行遍历，
                    // 找到一个尚未被全部分配出去的组，最后在里面分配一个bit。它将会返回分配的bit所在的位置，
                    // 等同于索引节点编号/数据块的编号。如果所有bit均已经被分配出去了，则返回 None 。
                    
                    // 这里分配一个bit，置1
                    bitmap_block[bits64_pos] |= 1u64 << inner_pos;

                    // 然后返回 bit 所在位置 / 返回分配的bit编号
                    // (第几个bitmap_block-1)*BLOCK_BITS + 某个bitmap_block里u64位置*64 + 空闲位置在第几个位置（从0开始数）                    
                    // 这是个 比特位id in data/inode area，代表  data/inode area 中的第几个数据块是空闲的，将其分配出去
                    Some(nth_of_block_in_bitmap * BLOCK_BITS + bits64_pos * 64 + inner_pos as usize)
                } else {
                    None
                }
            });
            // 如果找到空闲位，分配到了，就退出，包括退出循环
            // 这是个 id in data/inode area
            if pos.is_some() {
                return pos;
            }
        }
        // 整个遍历一边都找不到空闲位，直接返回None，分配不成功
        None
    }
    /// Deallocate a block
    pub fn dealloc(&self, block_device: &Arc<dyn BlockDevice>, bit: usize) {
        let (block_pos, bits64_pos, inner_pos) = decomposition(bit);
        get_block_cache(block_pos + self.start_block_id, Arc::clone(block_device))
            .lock()
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                assert!(bitmap_block[bits64_pos] & (1u64 << inner_pos) > 0);
                bitmap_block[bits64_pos] -= 1u64 << inner_pos;
            });
    }
    /// Get the max number of allocatable blocks
    pub fn maximum(&self) -> usize {
        self.blocks * BLOCK_BITS
    }
}
