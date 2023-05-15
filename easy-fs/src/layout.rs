use super::{get_block_cache, BlockDevice, BLOCK_SZ};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter, Result};
use bitflags::bitflags;

/// Magic number for sanity check
const EFS_MAGIC: u32 = 0x3b800001;
/// The max number of direct inodes
const INODE_DIRECT_COUNT: usize = 8;
/// The max length of inode name
const NAME_LENGTH_LIMIT: usize = 27;
/// The max number of indirect1 inodes
const INODE_INDIRECT1_COUNT: usize = BLOCK_SZ / 4;
/// The max number of indirect2 inodes
const INODE_INDIRECT2_COUNT: usize = INODE_INDIRECT1_COUNT * INODE_INDIRECT1_COUNT;
/// The upper bound of direct inode index
const DIRECT_BOUND: usize = INODE_DIRECT_COUNT;
/// The upper bound of indirect1 inode index
const INDIRECT1_BOUND: usize = DIRECT_BOUND + INODE_INDIRECT1_COUNT;
/// The upper bound of indirect2 inode indexs
#[allow(unused)]
const INDIRECT2_BOUND: usize = INDIRECT1_BOUND + INODE_INDIRECT2_COUNT;
/// Super block of a filesystem
#[repr(C)]
pub struct SuperBlock {
    magic: u32,
    pub total_blocks: u32,
    pub inode_bitmap_blocks: u32,
    pub inode_area_blocks: u32,
    pub data_bitmap_blocks: u32,
    pub data_area_blocks: u32,
}

impl Debug for SuperBlock {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("SuperBlock")
            .field("total_blocks", &self.total_blocks)
            .field("inode_bitmap_blocks", &self.inode_bitmap_blocks)
            .field("inode_area_blocks", &self.inode_area_blocks)
            .field("data_bitmap_blocks", &self.data_bitmap_blocks)
            .field("data_area_blocks", &self.data_area_blocks)
            .finish()
    }
}

impl SuperBlock {
    /// Initialize a super block
    pub fn initialize(
        &mut self,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
        inode_area_blocks: u32,
        data_bitmap_blocks: u32,
        data_area_blocks: u32,
    ) {
        *self = Self {
            magic: EFS_MAGIC,
            total_blocks,
            inode_bitmap_blocks,
            inode_area_blocks,
            data_bitmap_blocks,
            data_area_blocks,
        }
    }
    /// Check if a super block is valid using efs magic
    pub fn is_valid(&self) -> bool {
        self.magic == EFS_MAGIC
    }
}
/// Type of a disk inode
#[derive(PartialEq)]
pub enum DiskInodeType {
    File,
    Directory,
}

/// The stat of an inode
#[repr(C)]
#[derive(Debug,Clone)]
pub struct Stat {
    /// ID of device containing file
    pub dev: u64,
    /// inode number
    pub ino: u64,
    /// file type and mode
    pub mode: StatMode,
    /// number of hard links
    pub nlink: u32,
    /// unused pad
    pad: [u64; 7],
}

impl Stat{
    /// create a new file stat
    pub fn new(_dev: u64, inode_num:u64, stat_mode: StatMode, nlink:u32) -> Self{
        Stat {
            /// ID of device containing file
            dev: 0,
            /// inode number
            ino: inode_num,
            /// file type and mode
            mode: stat_mode,
            /// number of hard links
            nlink: nlink,
            /// unused pad
            pad: [0; 7],
        }
    }
    /// increase physical link number in Inode stat 
    pub fn increase_plink(&mut self){
        self.nlink += 1;
    }

    /// decrease physical link number in Inode stat 
    pub fn decrease_plink(&mut self) -> u32{
        if self.nlink >= 1 {
            self.nlink -= 1;
        }
        self.nlink
    }
}

bitflags! {
    /// The mode of a inode
    /// whether a directory or a file
    pub struct StatMode: u32 {
        /// null
        const NULL  = 0;
        /// directory
        const DIR   = 0o040000;
        /// ordinary regular file
        const FILE  = 0o100000;
    }
}

/// An indirect block
type IndirectBlock = [u32; BLOCK_SZ / 4];
/// A data block
type DataBlock = [u8; BLOCK_SZ];
/// A disk inode
#[repr(C)]
pub struct DiskInode {
    /// 表示文件/目录内容的字节数
    pub size: u32,
    /// 存储文件内容/目录内容的数据块的直接索引 ，数组里记录块编号，不是data_area_block里的序列编号，而是device_block_id
    pub direct: [u32; INODE_DIRECT_COUNT],
    /// 存储文件内容/目录内容的数据块的 一级间接索引 所在块的 device_block_id 
    pub indirect1: u32,
    /// 存储文件内容/目录内容的数据块的 二级间接索引 所在块的 device_block_id 
    pub indirect2: u32,
    /// inode stat, 文件/目录 信息
    pub stat: Stat,
    /// 索引节点的类型 DiskInodeType
    type_: DiskInodeType,
}

impl DiskInode {
    /// Initialize a disk inode, as well as all direct inodes under it
    /// indirect1 and indirect2 block are allocated only when they are needed
    pub fn initialize(&mut self, inode_num: u32, type_: DiskInodeType) {
        let stat_mode = match type_ {
            DiskInodeType::Directory => StatMode::DIR,
            DiskInodeType::File => StatMode::FILE
        };
        self.size = 0;
        self.direct.iter_mut().for_each(|v| *v = 0);
        self.indirect1 = 0;
        self.indirect2 = 0;
        self.stat = Stat::new(0,inode_num as u64, stat_mode,1);
        self.type_ = type_;
    }
    /// Whether this inode is a directory
    pub fn is_dir(&self) -> bool {
        self.type_ == DiskInodeType::Directory
    }
    /// Whether this inode is a file
    #[allow(unused)]
    pub fn is_file(&self) -> bool {
        self.type_ == DiskInodeType::File
    }
    /// Return block number correspond to size.
    /// 
    /// 根据文件/目录的大小 计算 需要占用的 磁盘块 数量
    pub fn data_blocks(&self) -> u32 {
        Self::_data_blocks(self.size)
    }
    ///  size 大小的文件、目录 本身 需要占用多少个 磁盘块（data_area_block_amount）, 向上取整
    fn _data_blocks(size: u32) -> u32 {
        (size + BLOCK_SZ as u32 - 1) / BLOCK_SZ as u32
    }
    /// Return number of blocks needed include indirect1/2.
    /// 
    /// 计算 保存 传入参数size的文件/目录 需要的 磁盘块 总数
    /// 
    /// 包含 文件、目录本身需要的空间 + 一二级索引块（也保存在数据块区域） 需要的空间 
    /// 
    /// 返回 需要的 数据块区域 数据块的个数
    pub fn total_blocks(size: u32) -> u32 {
        // data_area_block 数量
        let data_blocks = Self::_data_blocks(size) as usize;
        let mut total = data_blocks as usize;
        // indirect1 存储 indirect1 占用一个 数据块
        if data_blocks > INODE_DIRECT_COUNT {
            total += 1;
        }
        // indirect2 
        if data_blocks > INDIRECT1_BOUND {
            // self.indirect2 占用一个 数据块 
            total += 1;

            // // sub indirect1
            // total +=
            //     (data_blocks - INDIRECT1_BOUND + INODE_INDIRECT1_COUNT - 1) / INODE_INDIRECT1_COUNT;

            // data_blocks - INDIRECT1_BOUND 是 二级块能表达的范围 （去掉了直接和一级块能表达的范围）
            // 二级块 存储 INODE_INDIRECT1_COUNT 个一级块
            // 做除法向上取整，看看需要多少个一级索引块
            let sub_indirect1_needed = (data_blocks - INDIRECT1_BOUND + INODE_INDIRECT1_COUNT - 1) / INODE_INDIRECT1_COUNT;
            // 总数加上 sub_indirect1_needed
            total += sub_indirect1_needed;
        }
        // 返回 需要的 数据块区域 数据块的个数
        total as u32
    }
    /// Get the number of data blocks that have to be allocated given the new size of data
    pub fn blocks_num_needed(&self, new_size: u32) -> u32 {
        assert!(new_size >= self.size);
        Self::total_blocks(new_size) - Self::total_blocks(self.size)
    }
    /// Get id of block using inner_id(data_area_block's inner id) 
    /// 
    /// 用 数据块 在 数据块区域内的编号 获取 磁盘设备块编号 
    pub fn get_block_id(&self, inner_id: u32, block_device: &Arc<dyn BlockDevice>) -> u32 {
        let inner_id = inner_id as usize;
        if inner_id < INODE_DIRECT_COUNT {
            // 如果文件很小，可以从 直接索引 self.direct数组中取出 device_block_id:u32
            // 它是 存储文件内容/目录内容的数据块的索引、编号。
            self.direct[inner_id]
        } else if inner_id < INDIRECT1_BOUND {
            // 如果 inner_id 在 self.direct.len() 和 self.direct.len()+128 之间
            // 因为一级索引块 在数据块区域 中
            // 需要读取一级索引块，（读取块的 块缓存 ），获取块缓存
            // 从块缓存中 以 IndirectBlock：[u32数组] 数据结构读取块中的数据
            // 返回 一级索引块->该块的块缓存中 适当位置的数组元素 u32， 这个 u32 就是数据所在的 device_block_id
            get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect_block: &IndirectBlock| {                    
                    // 返回 一级索引块->该块的块缓存中 适当位置的数组元素 u32， 这个 u32 就是数据所在的 device_block_id
                    indirect_block[inner_id - INODE_DIRECT_COUNT]
                })
        } else {
            // 数据比较大，一个一级索引+直接索引 不够
            // 此时 可能需要一个二级间接索引+一个一级间接索引
            // 此时 已有多个二级间接索引+多个一级间接索引+直接索引（已满）

            // 使用二级索引块时，inner_id必然大于 一级索引块和直接索引能表达的上限
            // 而 使用二级索引块时 又不使用 disk-cache记录的一级和直接索引
            // 计算 二级索引块 中 一级索引块 的位置，需要除以二级索引块中一级索引块的最大个数
            // 为何要减掉直接和一级能表达的块数上限？？？？？？？？？
            // 存储/生成 索引 的时候，优先使用的 直接索引和一级索引，剩下的才利用 二级索引块 
            // 二级索引块中 是存储直接索引和一级索引后 剩下的 块 所在的 一级索引块 的 device_block_id
            let last = inner_id - INDIRECT1_BOUND;
            let indirect1 = get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect2: &IndirectBlock| {
                    // 以IndirectBlock：[u32；128]数据结构读取二级索引块内容
                    // 在这个块内 
                    // last / INODE_INDIRECT1_COUNT 计算二级块内的 一级块 在第多少个
                    // 在二级块中取出这个一级块的 device_block_id
                    indirect2[last / INODE_INDIRECT1_COUNT]
                });
            // 下面根据上面 从 二级索引块中取出的 一级索引块的 device_block_id
            // 从那个一级索引块（不是blcok_cache中的self.indirect1）中取出 目标 device_block_id
            get_block_cache(indirect1 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect1: &IndirectBlock| {
                    // 
                    // last % INODE_INDIRECT1_COUNT 计算二级块内的 一级块 在第多少个的 余数，就是在一级块内的偏移量
                    indirect1[last % INODE_INDIRECT1_COUNT]
                })
        }
    }
    /// Inncrease the size of current disk inode。
    /// 
    /// 将传入的 v 中保存的 新分配的数据块的磁盘块id device_block_id，
    /// 保存进 DiskInode 的 直接索引 、一级间接索引 、二级间接索引中
    /// 
    /// 实参 new_blocks 中包含了 数据占用的块 和 一二级间接索引块占用的块 如果有一二级索引的话
    pub fn increase_size(
        &mut self,
        new_size: u32,
        new_blocks: Vec<u32>, //new_blocks 是一个保存了本次容量扩充所需块编号的向量，这些块都是由上层的磁盘块管理器负责分配的。
        block_device: &Arc<dyn BlockDevice>,
    ) {
        let mut current_blocks = self.data_blocks();
        self.size = new_size;
        let mut total_blocks = self.data_blocks();
        let mut new_blocks = new_blocks.into_iter();
        // fill direct
        while current_blocks < total_blocks.min(INODE_DIRECT_COUNT as u32) {
            self.direct[current_blocks as usize] = new_blocks.next().unwrap();
            current_blocks += 1;
        }
        // alloc indirect1
        if total_blocks > INODE_DIRECT_COUNT as u32 {
            if current_blocks == INODE_DIRECT_COUNT as u32 {
                self.indirect1 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_DIRECT_COUNT as u32;
            total_blocks -= INODE_DIRECT_COUNT as u32;
        } else {
            return;
        }
        // fill indirect1
        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect1: &mut IndirectBlock| {
                while current_blocks < total_blocks.min(INODE_INDIRECT1_COUNT as u32) {
                    indirect1[current_blocks as usize] = new_blocks.next().unwrap();
                    current_blocks += 1;
                }
            });
        // alloc indirect2
        if total_blocks > INODE_INDIRECT1_COUNT as u32 {
            if current_blocks == INODE_INDIRECT1_COUNT as u32 {
                self.indirect2 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_INDIRECT1_COUNT as u32;
            total_blocks -= INODE_INDIRECT1_COUNT as u32;
        } else {
            return;
        }
        // fill indirect2 from (a0, b0) -> (a1, b1)
        let mut a0 = current_blocks as usize / INODE_INDIRECT1_COUNT;
        let mut b0 = current_blocks as usize % INODE_INDIRECT1_COUNT;
        let a1 = total_blocks as usize / INODE_INDIRECT1_COUNT;
        let b1 = total_blocks as usize % INODE_INDIRECT1_COUNT;
        // alloc low-level indirect1
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect2: &mut IndirectBlock| {
                while (a0 < a1) || (a0 == a1 && b0 < b1) {
                    if b0 == 0 {
                        indirect2[a0] = new_blocks.next().unwrap();
                    }
                    // fill current
                    get_block_cache(indirect2[a0] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            indirect1[b0] = new_blocks.next().unwrap();
                        });
                    // move to next
                    b0 += 1;
                    if b0 == INODE_INDIRECT1_COUNT {
                        b0 = 0;
                        a0 += 1;
                    }
                }
            });
    }

    /// Clear size to zero and return blocks that should be deallocated.
    /// We will clear the block contents to zero later.
    pub fn clear_size(&mut self, block_device: &Arc<dyn BlockDevice>) -> Vec<u32> {
        let mut v: Vec<u32> = Vec::new();
        let mut data_blocks = self.data_blocks() as usize;
        self.size = 0;
        let mut current_blocks = 0usize;
        // direct
        while current_blocks < data_blocks.min(INODE_DIRECT_COUNT) {
            v.push(self.direct[current_blocks]);
            self.direct[current_blocks] = 0;
            current_blocks += 1;
        }
        // indirect1 block
        if data_blocks > INODE_DIRECT_COUNT {
            v.push(self.indirect1);
            data_blocks -= INODE_DIRECT_COUNT;
            current_blocks = 0;
        } else {
            return v;
        }
        // indirect1
        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect1: &mut IndirectBlock| {
                while current_blocks < data_blocks.min(INODE_INDIRECT1_COUNT) {
                    v.push(indirect1[current_blocks]);
                    //indirect1[current_blocks] = 0;
                    current_blocks += 1;
                }
            });
        self.indirect1 = 0;
        // indirect2 block
        if data_blocks > INODE_INDIRECT1_COUNT {
            v.push(self.indirect2);
            data_blocks -= INODE_INDIRECT1_COUNT;
        } else {
            return v;
        }
        // indirect2
        assert!(data_blocks <= INODE_INDIRECT2_COUNT);
        let a1 = data_blocks / INODE_INDIRECT1_COUNT;
        let b1 = data_blocks % INODE_INDIRECT1_COUNT;
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect2: &mut IndirectBlock| {
                // full indirect1 blocks
                for entry in indirect2.iter_mut().take(a1) {
                    v.push(*entry);
                    get_block_cache(*entry as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter() {
                                v.push(*entry);
                            }
                        });
                }
                // last indirect1 block
                if b1 > 0 {
                    v.push(indirect2[a1]);
                    get_block_cache(indirect2[a1] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter().take(b1) {
                                v.push(*entry);
                            }
                        });
                    //indirect2[a1] = 0;
                }
            });
        self.indirect2 = 0;
        v
    }
    /// Read data from current disk inode
    /// 
    /// 将文件内容从 offset 字节开始的部分读到内存中的缓冲区 buf 中，
    /// 并返回实际读到的字节数。如果文件剩下的内容还足够多，那么缓冲区会被填满；
    /// 否则文件剩下的全部内容都会被读到缓冲区中
    pub fn read_at(
        &self,
        offset: usize,
        buf: &mut [u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        let mut start_bytes = offset;
        // 比较 起始+缓冲区长度 和 文件、目录的大小，ending_bytes 等于其中 较小者
        // ending_bytes 是个字节数
        let ending_bytes = (offset + buf.len()).min(self.size as usize);
        if start_bytes >= ending_bytes {
            // 要从超过文件长度的地方读取文件，异常
            return 0;
        }
        // diskInode指向的数据分布在多个数据块中
        // 计算 start/offset 在第几个block，一会将这个 inner_id 转换成 device_block_id 
        let mut start_block = start_bytes / BLOCK_SZ;
        // 记录 总读取字节数
        let mut read_size_in_bytes = 0usize;

        // 循环读取，循环一次最多 一个block，多次循环加起来 最多需要读满缓冲区长度
        loop {
            // calculate end of current block
            // 计算 当前块结尾的的字节地址, 先按一个块长度来
            // 1 + 512 xxxx
            // 512 + 512*blockid
            // start_bytes 加一个块的字节
            // let mut end_current_block_in_bytes = (start_bytes / BLOCK_SZ + 1) * BLOCK_SZ;
            let mut end_in_bytes_to_read_till = (start_bytes / BLOCK_SZ + 1) * BLOCK_SZ;
            // let mut end_current_block_in_bytes = (start_block + 1) * BLOCK_SZ;
            // let mut end_current_block_in_bytes = start_bytes +  BLOCK_SZ; xxxx 不能这么简化
            // let mut end_current_block_in_bytes =  BLOCK_SZ - start_bytes % BLOCK_SZ;
            // 如果 end_bytes_to_be_read_till 小于 ending_bytes 就用end_bytes_to_be_read_till，否则 取前面计算的 ending_bytes

            // （offset+缓冲区长度）缓冲区装满时的 结尾字节数；文件大小字节数；这次可以能读取到的最大字节地址） 三者取最小者 
            end_in_bytes_to_read_till = end_in_bytes_to_read_till.min(ending_bytes);
            // read and update read size
            // 这次能读取的 字节长度
            let block_read_size_in_bytes = end_in_bytes_to_read_till - start_bytes;
            let dst = &mut buf[read_size_in_bytes..read_size_in_bytes + block_read_size_in_bytes];
            get_block_cache(
                self.get_block_id(start_block as u32, block_device) as usize,
                Arc::clone(block_device),
            )
            .lock()
            .read(0, |data_block: &DataBlock| {
                // start_bytes % BLOCK_SZ 是在取字节索引，因为DataBlock里是u8数组
                // 
                let src = &data_block[start_bytes % BLOCK_SZ..start_bytes % BLOCK_SZ + block_read_size_in_bytes];
                dst.copy_from_slice(src);
            });
            read_size_in_bytes += block_read_size_in_bytes;
            if end_in_bytes_to_read_till == ending_bytes {
                break;
            }
            // move to next block
            start_block += 1;
            start_bytes = end_in_bytes_to_read_till;
        }
        read_size_in_bytes
    }
    /// Write data into current disk inode
    /// size must be adjusted properly beforehand
    pub fn write_at(
        &mut self,
        offset: usize,
        buf: &[u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        let mut start = offset;
        let end = (offset + buf.len()).min(self.size as usize);
        assert!(start <= end);
        let mut start_block = start / BLOCK_SZ;
        let mut write_size = 0usize;
        loop {
            // calculate end of current block
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            end_current_block = end_current_block.min(end);
            // write and update write size
            let block_write_size = end_current_block - start;
            get_block_cache(
                self.get_block_id(start_block as u32, block_device) as usize,
                Arc::clone(block_device),
            )
            .lock()
            .modify(0, |data_block: &mut DataBlock| {
                let src = &buf[write_size..write_size + block_write_size];
                let dst = &mut data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_write_size];
                dst.copy_from_slice(src);
            });
            write_size += block_write_size;
            // move to next block
            if end_current_block == end {
                break;
            }
            start_block += 1;
            start = end_current_block;
        }
        write_size
    }
}
/// A directory entry
#[repr(C)]
pub struct DirEntry {
    name: [u8; NAME_LENGTH_LIMIT + 1],
    inode_id: u32,
}
/// Size of a directory entry
pub const DIRENT_SZ: usize = 32;

impl DirEntry {
    /// Create an empty directory entry
    pub fn empty() -> Self {
        Self {
            name: [0u8; NAME_LENGTH_LIMIT + 1],
            inode_id: 0,
        }
    }
    /// Crate a directory entry from name and inode number
    pub fn new(name: &str, inode_id: u32) -> Self {
        let mut bytes = [0u8; NAME_LENGTH_LIMIT + 1];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Self {
            name: bytes,
            inode_id,
        }
    }
    /// Serialize into bytes
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self as *const _ as usize as *const u8, DIRENT_SZ) }
    }
    /// Serialize into mutable bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self as *mut _ as usize as *mut u8, DIRENT_SZ) }
    }
    /// Get name of the entry
    pub fn name(&self) -> &str {
        let len = (0usize..).find(|i| self.name[*i] == 0).unwrap();
        core::str::from_utf8(&self.name[..len]).unwrap()
    }
    /// Get inode number of the entry
    pub fn inode_id(&self) -> u32 {
        self.inode_id
    }
}
