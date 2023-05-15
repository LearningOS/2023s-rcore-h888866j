//! File and filesystem-related syscalls
use crate::fs::{open_file, linkat, unlinkat, OpenFlags, Stat, OSInode, };
use crate::mm::{translated_byte_buffer, translated_str, translated_refmut, UserBuffer};
use crate::task::{current_task, current_user_token};
use core::ptr;

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        // inner.fd_table[fd].unwrap()
        // .
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

/// YOUR JOB: Implement fstat.
pub fn sys_fstat(_fd: usize, _st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // println!("sys_stat syscall: _fd={}",_fd);
    let task = current_task().unwrap();
    let token = current_user_token();
    
    // let page_table = PageTable::from_token(current_user_token());
    // let va = VirtAddr::from(_st as usize);
    // // user space TaskInfo's start_ppn
    // let ppn_start: PhysPageNum= page_table.translate(va.floor()).unwrap().ppn();
    // let pa:PhysAddr = ppn_start.into();
    // // pa = PhysAddr::from(usize::from(pa) + va.page_offset());
    // // let pa_ptr = usize::from(pa) as *mut TaskInfo ;
    // let pa_ptr = (usize::from(pa) + va.page_offset()) as *mut Stat;

    let pa_ref = translated_refmut(token, _st);

    // println!("in fstat before crete tcb inner");
    let inner = task.inner_exclusive_access();
    
    // println!("in fstat after crete tcb inner");
    if let Some(file_osinode) = &inner.fd_table[_fd]{
        // *pa_ref = file_osinode.get_fstat();
        // let file = file_osinode.clone();
        // return core::any::type_name_of_val(&file);
        // assert_eq!(core::any::type_name_of_val(&file),"fs");

        // println!("fstat in if let");
        // let file_osinode= file_osinode.clone();
        
        // println!("fstat after file_node clone");

        // working codes
        // // 也是得先解引用两次成dyn fs::File + Send + Sync 在cast 成 *const (dyn File + Send + Sync)
        // let x = &(**file_osinode) as *const (dyn File + Send + Sync);
        // let pt = x as *const () as usize as *const OSInode;

        
        // 得是两个星号解引用成 trait对象 ，取其指针，一个星号的话需要从fd_table 中take()
        let osinode_ptr = ptr::addr_of!(**file_osinode);
        // https://users.rust-lang.org/t/cast-through-a-thin-pointer-first/36311        
        let pt = osinode_ptr as *const () as usize as *mut OSInode;
        
        // inner.fd_table[_fd] = Some(file_osinode);
        // drop(file_osinode);
        drop(inner);
        // println!("fstat after drop pcb inner");
        unsafe {
            // let uu = (*pt).clone();
            // drop(pt);
            let s = (*pt)
            .get_fstat();
            // println!("after deref and get fstat");
            // expected struct `fs::StatMode`, found struct `easy_fs::StatMode`
            // let mode = StatMode::from_bits(s.mode.bits()).unwrap();
            // println!("after get stat mode");
            // *pa_ref = Stat {
            //     dev:s.dev,
            //     ino:s.ino,
            //     mode:s.mode,
            //     nlink:s.nlink,
            //     pad:[0;7]
            // };
            *pa_ref = Stat::new(0,s.ino,s.mode,s.nlink);
            
            // println!("after deref and assign to pa_ref");
        }
        // println!("fstat before return");
        return 0
    }
    -1
}

/// YOUR JOB: Implement linkat.
pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_linkat IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let old_path = translated_str(token, _old_name);
    let new_path = translated_str(token, _new_name);
    linkat(old_path.as_str(),new_path.as_str())

    // -1
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_unlinkat IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let old_path = translated_str(token, _name);
    unlinkat(old_path.as_str())
    // -1
}
