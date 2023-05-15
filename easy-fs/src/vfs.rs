use log::info;

use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};

/// Virtual filesystem layer over easy-fs
pub struct Inode {
    /// 保存 Inode 对应的 DiskInode 保存在磁盘上的具体位置，
    /// 这里是 DiskInode 所在磁盘块的 device_block_id
    block_id: usize,
    /// 保存 Inode 对应的 DiskInode 保存在磁盘上的具体位置
    /// 因为 一个磁盘块上有4个 DiskInode， 读取时需要offset 定位到指定 DiskInode
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
    // inode_id: usize,
    // stat: Stat,
}

impl Inode {
    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
        // inode_id: u32,
        // stat_mode: StatMode
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
            // inode_id: inode_id as usize,
            // stat: Stat::new(inode_id as u64, stat_mode)
        }
    }
    /// Call a function over a disk inode to read it
    pub fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }
    /// Call a function over a disk inode to modify it
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }
    /// Find inode id/num of a file/diretory under a disk inode by file/dir name
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(dirent.inode_id() as u32);
            }
        }
        None
    }

    /// Find the inode of the corresponding name under current inode(directory) 
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                    // inode_id,
                ))
            })
        })
    }

    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        // v 存储 新分配空间的 磁盘数据块id device_block_id
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }
    /// Create inode under current inode by name
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.read_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block
        let new_inode_id = fs.alloc_inode();
        // initialize inode's corresponding disk_inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        // 初始化 disk_inode 磁盘块 上的信息
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(new_inode_id,DiskInodeType::File);
            });
        // 将新文件的目录信息 写入目录的 disk_inode
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
            // new_inode_id
        )))
        // release efs lock automatically by compiler
    }

    /// unlink at, remove dir entry
    pub fn unlinkat(&self, old_name:&str)  -> isize {
        
        info!("unlink at:nlink decreased by 1");
        let fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(old_name, root_inode)
        };
        let old_inode_id = self.read_disk_inode(op);
        // 文件不存在
        if old_inode_id.is_none(){
            return -1
        }
        // let old_inode_id = old_inode_id.unwrap();

        // plink 减一
        // let old_inode = self.find(old_name).unwrap(); // find 尝试获取锁 却等不到
        // old_inode.modify_disk_inode(|di_inode|{
        //     if di_inode.stat.nlink >= 1{
        //         di_inode.stat.nlink -= 1
        //     }
        //     // di_inode.stat.decrease_plink();
        // });
        
        let (block_id, block_offset) = fs.get_disk_inode_pos(old_inode_id.unwrap());
        let old_inode1 = Arc::new(
            Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        ));
        info!("unlink at: get, create in actually, old inode succeed, next is incrument stat.nlink ");
        let mut cur_nlink = 0;
        old_inode1.modify_disk_inode(|old_name_disk_inode|{
            info!("unlink at:trying decrease nlink");
            cur_nlink = old_name_disk_inode.stat.decrease_plink();
            // if 
        });
        drop(fs); // 释放锁
        info!("unlink at:nlink decreased by 1");
        if cur_nlink <= 0 {
            // 删除inode
            old_inode1.clear()
        }        
        let _fs = self.fs.lock();
        // check phsical link num, if = 1, clear else return
        // 清除 oldname的 dir entry
        self.modify_disk_inode(|dir_disk_inode|{
            assert!(dir_disk_inode.is_dir());
            let file_count = (dir_disk_inode.size as usize) / DIRENT_SZ;
            let mut dirent = DirEntry::empty();
            for i in 0..file_count {
                assert_eq!(
                    dir_disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                if dirent.name() == old_name {
                    // return Some(dirent.inode_id() as u32);
                    let empty_dirent = DirEntry::empty();
                    dir_disk_inode.write_at(DIRENT_SZ * i, empty_dirent.as_bytes(), &self.block_device,);
                }
            }
            
        });
        block_cache_sync_all(); 
        0
    }

    /// link at, create dir entry but using existing inode
    /// Increase the nlink count
    pub fn linkat(&self, old_name:&str, new_name:&str)  -> isize {
        info!("Inode::linkat ");
        if old_name == new_name {
            return -1
        }
        info!("Inode::linkat old! = new ");
        let mut fs = self.fs.lock();
        
        info!("Inode::linkat efs lock acquired ");
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            // 不必是 目录 ，可以是文件
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(old_name, root_inode)
        };
        info!("Inode::linkat ");
        let old_inode_id = self.read_disk_inode(op);
        if old_inode_id.is_none(){
            return -1
        }
        
        info!("Inode::linkat old inode is not None");
        let old_inode_id = old_inode_id.unwrap();
        info!("old inode id:{}",old_inode_id);
        // let old_inode = self.read_disk_inode(|disk_inode|{
                // 不行，find也获取锁
        //     self.find(old_name).unwrap()
        // });
        
        // info!(" tring get old inode");
        // // info!("op2");
        // self.modify_disk_inode(|di:&mut DiskInode|{            
        //     assert!(di.is_dir());
        //     let old_inode = self.find(old_name) // linkat 获取了锁，这里find 一直等待锁释放而不得
        //     .unwrap();
        //     old_inode.modify_disk_inode(|filediskInode|{
        //         filediskInode.stat.increase_plink()
        //     })
        // });
        // info!("stat nlink update succeed");

        // 根据inode_num 创建一个 Inode
        let (block_id, block_offset) = fs.get_disk_inode_pos(old_inode_id);
        let old_inode1 = Arc::new(
            Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        ));
        info!("get, create in actually, old inode succeed, next is incrument stat.nlink ");
        old_inode1.modify_disk_inode(|old_name_disk_inode|{
            old_name_disk_inode.stat.increase_plink();
        });
        // 在文件夹的目录里（这里是/）写入新的 dirEntry 文件名
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dir ent
            let dirent = DirEntry::new(new_name, old_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });
        info!("after add dir entry");
        // let (block_id, block_offset) = fs.get_disk_inode_pos(old_inode_id);
        block_cache_sync_all(); 
        0
    }

    /// List inodes under current inode
    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }
    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }
    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }
    /// Clear the data in current inode
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }
}
