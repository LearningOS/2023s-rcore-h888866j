//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.

mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::loader::{get_app_data, get_num_app};
use crate::sync::UPSafeCell;
use crate::syscall::process::TaskInfo;
// use crate::timer::get_time_us;
use crate::trap::TrapContext;
use alloc::vec::Vec;
use lazy_static::*;
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};
use crate::mm::{VirtAddr,VirtPageNum,MapPermission,};
pub use context::TaskContext;

/// The task manager, where all the tasks are managed.
///
/// Functions implemented on `TaskManager` deals with all task state transitions
/// and task context switching. For convenience, you can find wrappers around it
/// in the module level.
///
/// Most of `TaskManager` are hidden behind the field `inner`, to defer
/// borrowing checks to runtime. You can see examples on how to use `inner` in
/// existing functions on `TaskManager`.
pub struct TaskManager {
    /// total number of tasks
    num_app: usize,
    /// use inner value to get mutable access
    inner: UPSafeCell<TaskManagerInner>,
}

/// The task manager inner in 'UPSafeCell'
struct TaskManagerInner {
    /// task list
    tasks: Vec<TaskControlBlock>,
    /// id of current `Running` task
    current_task: usize,
}

lazy_static! {
    /// a `TaskManager` global instance through lazy_static!
    pub static ref TASK_MANAGER: TaskManager = {
        println!("init TASK_MANAGER");
        let num_app = get_num_app();
        println!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(get_app_data(i), i));
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    /// Run the first task in task list.
    ///
    /// Generally, the first task in task list is an idle task (we call it zero process later).
    /// But in ch4, we load apps statically, so the first task is a real app.
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let next_task = &mut inner.tasks[0];
        next_task.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(&mut _unused as *mut _, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// Change the status of current `Running` task into `Ready`.
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    /// Change the status of current `Running` task into `Exited`.
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    /// Find next task to run and return task id.
    ///
    /// In this case, we only return the first `Ready` task in task list.
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// Get the current 'Running' task's token.
    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_user_token()
    }

    /// Get the current 'Running' task's trap contexts.
    fn get_current_trap_cx(&self) -> &'static mut TrapContext {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_trap_cx()
    }

    /// Change the current 'Running' task's program break
    pub fn change_current_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].change_program_brk(size)
    }

    /// Switch current `Running` task to the task we have found,
    /// or there is no `Ready` task and we can exit with all applications completed
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.tasks[next].task_info.status = TaskStatus::Running;// 这里更新与否不重要，因为再taskinfo的接口中更新它
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                // 调用switch函数, 根据调用约定，需要保存一些寄存器比如 t0-t6,a0-a7
                // 根据调用约定，a0 被设置为 current_task_cx_ptr
                // 根据调用约定，a1 被设置为 next_task_cx_ptr
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // go back to user mode
        } else {
            panic!("All applications completed!");
        }
    }
    /// update task info
    fn update_current_task_info(&self){
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let current_task = &mut inner.tasks[current];
        // cannot borrow `inner` as immutable because it is also borrowed as mutable
        // immutable borrow occurs here. try adding a local storing this...
        // mod.rs(160, 33): ...and then using that local here
        current_task.time_elapsed();
        current_task.task_info.status = current_task.task_status;
    }
    /// get current task info
    fn get_current_task_info(&self) -> TaskInfo{
        self.update_current_task_info();
        let inner = self.inner.exclusive_access();
        let current_task = &inner.tasks[inner.current_task]; 
        // current_task.time_elapsed();
        // current_task.task_info.status = current_task.task_status;
        // current_task.task_info.syscall_times =
        // println!("{:?}",current_task.task_info.syscall_times);
        current_task.task_info
    }

    // /// get current task control block
    // fn get_current_tcb_copy(&self) -> TaskControlBlock{
    //     self.update_current_task_info();
    //     let inner = self.inner.exclusive_access();
    //     let current_task = inner.tasks[inner.current_task];
    //     // current_task.time_elapsed();
    //     // current_task.task_info
    //     current_task
    // }

    fn incrument_syscall_calling_times(&self,syscall_id:usize){
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let current_task_tcb = &mut inner.tasks[current];
        // current_task_tcb.time_elapsed();
        // current_task_tcb.task_info
        current_task_tcb.syscall_record_update(syscall_id);
    }

    fn munmap(&self,start: usize, len: usize) -> isize{
        // let token = self.get_current_token();
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let current_task_tcb = &mut inner.tasks[current];
        let vpn_start = VirtAddr::from(start).floor();
        let vpn_end = VirtAddr::from(start+len).floor();
        // 直接使用token变量，不能 直接传入 self.get_current_token();
        // let  pt = PageTable::from_token(token);
        // check if the [start,start+len) has already been mapped                
        info!("check if the [start,start+len) has already been mapped");
        for vpn in usize::from(vpn_start)..vpn_end.into(){
            // 这样unmap 没有回收资源啊， 应该是 memset 或者mapArea去umpap
            // pt.unmap(VirtPageNum::from(vpn));

            let x = current_task_tcb
            .memory_set
            .translate(VirtPageNum::from(vpn));
            // if let Some(pte) = x {
            //     if !pte.is_valid() {
            //         println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
            //         return -1
            //     }
            // }else{
            //     println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
            //     return -1
            // }

            // if x.is_some() && !x.unwrap().is_valid(){                
            //     println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
            //     return -1
            // }else if x.is_none() {                                
            //     println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
            //     return -1
            // }

            match x {
                // Some(pte) => match pte.is_valid() {
                //     false => {
                //         println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
                //         return -1
                //     },
                //     true => {}
                // }
                Some(pte) => if let false = pte.is_valid() {
                    println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
                    return -1
                }
                None => {
                    println!("vpn:{:?} were not mapped earlier, return -1",VirtPageNum::from(vpn));
                    return -1
                }
            }
        }
        current_task_tcb
        .memory_set
        .free_mapped_area(start,len)
        // 0

    }

    fn mmap(&self,start: usize, len: usize, prot: usize) -> isize {        
        // info!("Task Manager mmap0");
        // 需要先拿出来保存，否则可能造成 两次mutable借用
        // let token = self.get_current_token();
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let current_task_tcb = &mut inner.tasks[current];
        // current_task_tcb.memory_set.insert_framed_area(start_va, end_va, permission)
        let vpn_start = VirtAddr::from(start).floor();
        let vpn_end = VirtAddr::from(start+len).ceil();
        // println!("vpn_start:{:?},vpn_end:{:?},vpn_end+1:{:?}",vpn_start,vpn_end,
        // VirtPageNum::from(usize::from(vpn_end)+1_usize));
        // 直接使用token变量，不能 直接传入 self.get_current_token();
        // 用 current_task_tcb.memory_set.translate(vpn)就不用创建查询页表了
        // let pt = PageTable::from_token(token);
        // check if the [start,start+len) has already been mapped                
        // info!("check if the [start,start+len) has already been mapped");
        // for vpn in usize::from(vpn_start)..vpn_end.into(){
        for vpn_usize in usize::from(vpn_start)..usize::from(vpn_end){
            // let x = pt
            let x = current_task_tcb
            .memory_set
            .translate(VirtPageNum::from(vpn_usize));
            // println!("is none:{:?}, is_some:{:?}",x.is_none(),x.is_some());
            // if x.is_some() && x.unwrap().is_valid() {
            // x.map(||)
            // match x {
            //     Some(pte) => {
            //         match pte.is_valid() {
            //             true => {
            //                 return -1
            //             }
            //             _ => {}
            //         }
            //     }
            //     None => {
                    
            //     }
            // }
            if let Some(pte) = x {
                if pte.is_valid() {
                    println!("vpn:{:?} were mapped earlier, return -1",VirtPageNum::from(vpn_usize));
                    return -1
                }
            }
            // if x.is_valid(){
            //     println!("x.unwrap().is_valid: {}",x.unwrap().is_valid());
            //     println!("mmap:vpn:{:x} of [va:{:x},va:start+len:{:x}) 
            //     has already been mapped earlier, has already been used",vpn_usize,start,start+len);
            //     return -1;
            // }
        }
        // insert framed mapArea
        // info!("insert frames mapArea, prot = {:08b}", prot );
        // let ll = MapPermission::R|MapPermission::W|MapPermission::X|MapPermission::U;
        // if prot as u8 > ll.bits() {
        //     println!("Error, prot as u8 > ll.bits()")
        // }        
        // info!("all mapPerm              = {:08b}", ll );
        let _perm = MapPermission::from_bits(prot as u8).unwrap();
        // info!("from_bits converted: _perm {:?}",_perm);
        // 插入： 分配 map
        // 如何知晓 剩余容量 是否够分配呢？
        
        // println!("pt.translate(vpn_start).is_some():{}\n pt.translate(va+4096*1).is_some():{}\n{}\n{}\n{}",
        // pt.translate(vpn_start).is_some(),
        // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*1))).is_some(),
        // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*2))).is_some(),
        // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*3))).is_some(),
        // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*40))).is_some(),
    
        // );
        current_task_tcb
        .memory_set
        .insert_framed_area(
            VirtAddr::from(start),
            VirtAddr::from(start+len), 
            _perm | MapPermission::U,
        );
        // println!("pt.translate(vpn_start).is_some():{}\n pt.translate(va+4096*1).is_some():{}\n{}\n{}\n{}",
        // pt.translate(vpn_start).is_some(),
        // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*1))).is_some(),
        // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*2))).is_some(),
        // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*3))).is_some(),
        // pt.translate(VirtPageNum::from(VirtAddr::from(start+4096*4))).is_some(),    
        // );
        
        0
    }
}

/// Run the first task in task list.
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// Switch current `Running` task to the task we have found,
/// or there is no `Ready` task and we can exit with all applications completed
fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

/// Change the status of current `Running` task into `Ready`.
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

/// Change the status of current `Running` task into `Exited`.
fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

/// Get current running taskinfo instance
pub fn get_current_task_info() -> TaskInfo{
    TASK_MANAGER.get_current_task_info()
}

// /// Get current running taskControlBlock instance
// pub fn get_current_tcb_copy() -> &'static TaskControlBlock{
//     TASK_MANAGER.get_current_tcb_copy()
// }

/// incrument syscall record
pub fn incrument_syscall_calling_times(syscall_id:usize){
    // let inner = TASK_MANAGER.inner.exclusive_access();
    // let current = inner.current_task;
    // println!{"before: update syscall time func, syscallID were called for: {} times",inner.tasks[current].task_info.syscall_times[syscall_id]};
    // drop(inner);
    TASK_MANAGER.incrument_syscall_calling_times(syscall_id);
    // let inner = TASK_MANAGER.inner.exclusive_access();
    // let current = inner.current_task;
    // println!{"after: update syscall time func, syscallID were called for: {} times",inner.tasks[current].task_info.syscall_times[syscall_id]};
    // drop(inner);
}
/// Get the current 'Running' task's token.
pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

/// Get the current 'Running' task's trap contexts.
pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}

/// Change the current 'Running' task's program break
pub fn change_program_brk(size: i32) -> Option<usize> {
    TASK_MANAGER.change_current_program_brk(size)
}

/// mmap 
pub fn mmap(start: usize, len: usize, prot: usize)->isize{
    TASK_MANAGER.mmap(start, len, prot)
}
/// unmap
pub fn munmap(start: usize, len: usize) -> isize {
    TASK_MANAGER.munmap(start, len)
}
