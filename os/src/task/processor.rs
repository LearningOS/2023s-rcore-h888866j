//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;
use crate::task::TaskManager;

/// Processor management structure
pub struct Processor {
    ///The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,

    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    ///Create an empty Processor
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            // release coming task_inner manually
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            warn!("no tasks available in run_tasks");
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

///Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

// impl TaskManager
// /// update task info
// fn update_current_task_info(&self){
//     let mut inner = self.inner.exclusive_access();
//     let current = inner.current_task;
//     let current_task = &mut inner.tasks[current];
//     // cannot borrow `inner` as immutable because it is also borrowed as mutable
//     // immutable borrow occurs here. try adding a local storing this...
//     // mod.rs(160, 33): ...and then using that local here
//     current_task.time_elapsed();
//     current_task.task_info.status = current_task.task_status;
// }
// /// get current task info
// fn get_current_task_info(&self) -> TaskInfo{
//     self.update_current_task_info();
//     let inner = self.inner.exclusive_access();
//     let current_task = &inner.tasks[inner.current_task];
//     // current_task.time_elapsed();
//     // current_task.task_info.status = current_task.task_status;
//     // current_task.task_info.syscall_times =
//     // println!("{:?}",current_task.task_info.syscall_times);
//     current_task.task_info
// }
//
// // /// get current task control block
// // fn get_current_tcb_copy(&self) -> TaskControlBlock{
// //     self.update_current_task_info();
// //     let inner = self.inner.exclusive_access();
// //     let current_task = inner.tasks[inner.current_task];
// //     // current_task.time_elapsed();
// //     // current_task.task_info
// //     current_task
// // }
//
// fn incrument_syscall_calling_times(&self,syscall_id:usize){
//     let mut inner = self.inner.exclusive_access();
//     let current = inner.current_task;
//     let current_task_tcb = &mut inner.tasks[current];
//     // current_task_tcb.time_elapsed();
//     // current_task_tcb.task_info
//     current_task_tcb.syscall_record_update(syscall_id);
// }
//
// fn munmap(&self,start: usize, len: usize) -> isize{
//     // let token = self.get_current_token();
//     let mut inner = self.inner.exclusive_access();
//     let current = inner.current_task;
//     let current_task_tcb = &mut inner.tasks[current];
//     let vpn_start = VirtAddr::from(start).floor();
//     let vpn_end = VirtAddr::from(start+len).floor();
//     // 直接使用token变量，不能 直接传入 self.get_current_token();
//     // let  pt = PageTable::from_token(token);
//     // check if the [start,start+len) has already been mapped
//     info!("check if the [start,start+len) has already been mapped");
//     for vpn in usize::from(vpn_start)..vpn_end.into(){
//         // 这样unmap 没有回收资源啊， 应该是 memset 或者mapArea去umpap
//         // pt.unmap(VirtPageNum::from(vpn));
//
//         let x = current_task_tcb
//             .memory_set
//             .translate(VirtPageNum::from(vpn));
//         // if let Some(pte) = x {
//         //     if !pte.is_valid() {
//         //         println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
//         //         return -1
//         //     }
//         // }else{
//         //     println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
//         //     return -1
//         // }
//
//         // if x.is_some() && !x.unwrap().is_valid(){
//         //     println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
//         //     return -1
//         // }else if x.is_none() {
//         //     println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
//         //     return -1
//         // }
//
//         match x {
//             // Some(pte) => match pte.is_valid() {
//             //     false => {
//             //         println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
//             //         return -1
//             //     },
//             //     true => {}
//             // }
//             Some(pte) => if let false = pte.is_valid() {
//                 println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
//                 return -1
//             }
//             None => {
//                 println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
//                 return -1
//             }
//         }
//     }
//     current_task_tcb
//         .memory_set
//         .free_mapped_area(start,len)
//     // 0
//
// }
//
// fn mmap(&self,start: usize, len: usize, prot: usize) -> isize {
//     // info!("Task Manager mmap0");
//     // 需要先拿出来保存，否则可能造成 两次mutable借用
//     // let token = self.get_current_token();
//     let mut inner = self.inner.exclusive_access();
//     let current = inner.current_task;
//     let current_task_tcb = &mut inner.tasks[current];
//     // current_task_tcb.memory_set.insert_framed_area(start_va, end_va, permission)
//     let vpn_start = VirtAddr::from(start).floor();
//     let vpn_end = VirtAddr::from(start+len).ceil();
//     // println!("vpn_start:{:?},vpn_end:{:?},vpn_end+1:{:?}",vpn_start,vpn_end,
//     // VirtPageNum::from(usize::from(vpn_end)+1_usize));
//     // 直接使用token变量，不能 直接传入 self.get_current_token();
//     // 用 current_task_tcb.memory_set.translate(vpn)就不用创建查询页表了
//     // let pt = PageTable::from_token(token);
//     // check if the [start,start+len) has already been mapped
//     // info!("check if the [start,start+len) has already been mapped");
//     // for vpn in usize::from(vpn_start)..vpn_end.into(){
//     for vpn_usize in usize::from(vpn_start)..usize::from(vpn_end){
//         // let x = pt
//         let x = current_task_tcb
//             .memory_set
//             .translate(VirtPageNum::from(vpn_usize));
//         // println!("is none:{:?}, is_some:{:?}",x.is_none(),x.is_some());
//         // if x.is_some() && x.unwrap().is_valid() {
//         // x.map(||)
//         // match x {
//         //     Some(pte) => {
//         //         match pte.is_valid() {
//         //             true => {
//         //                 return -1
//         //             }
//         //             _ => {}
//         //         }
//         //     }
//         //     None => {
//
//         //     }
//         // }
//         if let Some(pte) = x {
//             if pte.is_valid() {
//                 println!("vpn:{:?} were mapped earlier, return -1",VirtPageNum::from(vpn_usize));
//                 return -1
//             }
//         }
//         // if x.is_valid(){
//         //     println!("x.unwrap().is_valid: {}",x.unwrap().is_valid());
//         //     println!("mmap:vpn:{:x} of [va:{:x},va:start+len:{:x})
//         //     has already been mapped earlier, has already been used",vpn_usize,start,start+len);
//         //     return -1;
//         // }
//     }
//     // insert framed mapArea
//     // info!("insert frames mapArea, prot = {:08b}", prot );
//     // let ll = MapPermission::R|MapPermission::W|MapPermission::X|MapPermission::U;
//     // if prot as u8 > ll.bits() {
//     //     println!("Error, prot as u8 > ll.bits()")
//     // }
//     // info!("all mapPerm              = {:08b}", ll );
//     let _perm = MapPermission::from_bits(prot as u8).unwrap();
//     // info!("from_bits converted: _perm {:?}",_perm);
//     // 插入： 分配 map
//     // 如何知晓 剩余容量 是否够分配呢？
//
//     // println!("pt.translate(vpn_start).is_some():{}\n pt.translate(va+4096*1).is_some():{}\n{}\n{}\n{}",
//     // pt.translate(vpn_start).is_some(),
//     // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*1))).is_some(),
//     // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*2))).is_some(),
//     // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*3))).is_some(),
//     // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*40))).is_some(),
//
//     // );
//     current_task_tcb
//         .memory_set
//         .insert_framed_area(
//             VirtAddr::from(start),
//             VirtAddr::from(start+len),
//             _perm | MapPermission::U,
//         );
//     // println!("pt.translate(vpn_start).is_some():{}\n pt.translate(va+4096*1).is_some():{}\n{}\n{}\n{}",
//     // pt.translate(vpn_start).is_some(),
//     // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*1))).is_some(),
//     // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*2))).is_some(),
//     // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*3))).is_some(),
//     // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*4))).is_some(),
//     // );
//
//     0
// }
// }
//
// /// Run the first task in task list.
// pub fn run_first_task() {
//     TASK_MANAGER.run_first_task();
// }
//
// /// Switch current `Running` task to the task we have found,
// /// or there is no `Ready` task and we can exit with all applications completed
// fn run_next_task() {
//     TASK_MANAGER.run_next_task();
// }
//
// /// Change the status of current `Running` task into `Ready`.
// fn mark_current_suspended() {
//     TASK_MANAGER.mark_current_suspended();
// }
//
// /// Change the status of current `Running` task into `Exited`.
// fn mark_current_exited() {
//     TASK_MANAGER.mark_current_exited();
// }
//
// /// Suspend the current 'Running' task and run the next task in task list.
// pub fn suspend_current_and_run_next() {
//     mark_current_suspended();
//     run_next_task();
// }
//
// /// Exit the current 'Running' task and run the next task in task list.
// pub fn exit_current_and_run_next() {
//     mark_current_exited();
//     run_next_task();
// }
//
// /// Get current running taskinfo instance
// pub fn get_current_task_info() -> TaskInfo{
//     TASK_MANAGER.get_current_task_info()
// }
//
// // /// Get current running taskControlBlock instance
// // pub fn get_current_tcb_copy() -> &'static TaskControlBlock{
// //     TASK_MANAGER.get_current_tcb_copy()
// // }
//
// /// incrument syscall record
// pub fn incrument_syscall_calling_times(syscall_id:usize){
//     // let inner = TASK_MANAGER.inner.exclusive_access();
//     // let current = inner.current_task;
//     // println!{"before: update syscall time func, syscallID were called for: {} times",inner.tasks[current].task_info.syscall_times[syscall_id]};
//     // drop(inner);
//     TASK_MANAGER.incrument_syscall_calling_times(syscall_id);
//     // let inner = TASK_MANAGER.inner.exclusive_access();
//     // let current = inner.current_task;
//     // println!{"after: update syscall time func, syscallID were called for: {} times",inner.tasks[current].task_info.syscall_times[syscall_id]};
//     // drop(inner);
// }
// /// Get the current 'Running' task's token.
// pub fn current_user_token() -> usize {
//     TASK_MANAGER.get_current_token()
// }
//
// /// Get the current 'Running' task's trap contexts.
// pub fn current_trap_cx() -> &'static mut TrapContext {
//     TASK_MANAGER.get_current_trap_cx()
// }
//
// /// Change the current 'Running' task's program break
// pub fn change_program_brk(size: i32) -> Option<usize> {
//     TASK_MANAGER.change_current_program_brk(size)
// }
//
// /// mmap
// pub fn mmap(start: usize, len: usize, prot: usize)->isize{
//     TASK_MANAGER.mmap(start, len, prot)
// }
// /// unmap
// pub fn munmap(start: usize, len: usize) -> isize {
//     TASK_MANAGER.munmap(start, len)
// }

