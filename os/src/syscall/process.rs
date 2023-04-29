//! Process management syscalls


use crate::{
    config::MAX_SYSCALL_NUM,
    task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, 
        TaskStatus, get_current_task_info, current_user_token, mmap, munmap},
    timer::get_time_us, mm::{VirtAddr, PhysPageNum},
    mm::{PageTable, PhysAddr},
};

#[repr(C)]
#[derive(Debug)]
/// time Val
pub struct TimeVal {
    /// sec
    pub sec: usize,
    /// usec
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
#[derive(Clone,Copy)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    pub status: TaskStatus,
    /// The numbers of syscall called by task
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    pub time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    
    trace!("kernel: sys_get_time start");
    let t = get_time_us();
    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    let va = VirtAddr::from(_ts as usize);
    let vpn_start = va.floor();
    // let x:VirtAddr = 2_usize.into();
    // user space TimeVal's start_ppn
    let ppn_start: PhysPageNum= page_table.translate(vpn_start).unwrap().ppn();
    let mut pa:PhysAddr = ppn_start.into();
    pa = PhysAddr::from(usize::from(pa) + va.page_offset());
    let pa_ptr = usize::from(pa) as *mut TimeVal ;
    unsafe{
        *pa_ptr = TimeVal{
            sec: t / 1_000_000,
            usec:  t % 1_000_000,
        };
    }
    trace!("kernel: sys_get_time       end");
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {    
    trace!("kernel: sys_task_info start");
    let ti = get_current_task_info();
    // println!("task time elapsed:{:?}",ti.time);
    // println!("syscall SYSCALL_GET_TIME 169 was called for ：{:?} times",ti.syscall_times[169]);
    // println!("syscall SYSCALL_TASK_INFO 410 was called for ：{:?} times",ti.syscall_times[410]);
    // unsafe {
    //     *_ti = ti;
    // };
    
    // working codes: this version does not take the condition like
    // TaskInfo spanning into two inconsecutive physical pages into consideration
    let page_table = PageTable::from_token(current_user_token());
    let va = VirtAddr::from(_ti as usize);
    // user space TaskInfo's start_ppn
    let ppn_start: PhysPageNum= page_table.translate(va.floor()).unwrap().ppn();
    let pa:PhysAddr = ppn_start.into();
    // pa = PhysAddr::from(usize::from(pa) + va.page_offset());
    // let pa_ptr = usize::from(pa) as *mut TaskInfo ;
    let pa_ptr = (usize::from(pa) + va.page_offset()) as *mut TaskInfo;
    unsafe{
        *pa_ptr = ti
    }
    trace!("kernel: sys_task_info         end");
    0
    // -1
}


/// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {

    // let _perm = MapPermission::R|MapPermission::W|MapPermission::X|MapPermission::U;
    // trace!("prot bits: prot origin          :{:08b}",prot);
    // trace!("prot bits: ((1<<3) - 1)         :{:08b}",(1<<3) - 1);
    // trace!("prot bits:     7                :{:08b}",8 - 1);
    // trace!("prot bits: prot & !((1<<3) - 1) :{:08b}", prot & !((1<<3) - 1));
    // trace!("prot bits: prot & ((1<<3) - 1)) :{:08b}",(prot & ((1<<3) - 1)));
    // trace!("start % 4096 = {}",start % 4096);
    // 判断 prot 有效性
    if (prot & !((1<<3) - 1)) != 0  || (prot & ((1<<3) - 1)) == 0 || (start % 4096) != 0 {
        trace!("mmap: prot arg is not valid or start is not aligned with 4096");
        return -1
    }

    // let vpn_start = VirtAddr::from(start).floor();
    // let vpn_end = VirtAddr::from(start+len).ceil();
    // let token = current_user_token();
    // 不能在这分配啊，函数结束直接回收了, 得和sys_time 一样调用公有函数，在tcb里实现，大概是MapArea.map
    // 以下是错误实现
    // let mut x = MapArea::new(VirtAddr::from(start), VirtAddr::from(start+len), MapType::Framed, perm);
    // x.map(&mut PageTable::from_token(token));
    // PageTable::from_token(token).map(vpn, ppn, flags)

    // trace!("kernel: sys_mmap !");
    // 调用 mmap1 ，它又调用 mmap0，TaskManager.mmap0 里实现的
    // 左移一位，因为 MapPermission 最少做一个了一个1
    mmap(start, len, prot<<1)

    // trace!("kernel: sys_mmap");
    // 0
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    // trace!("kernel: sys_munmap !");
    munmap(start, len)
    // -1
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
