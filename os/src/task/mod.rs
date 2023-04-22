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

use crate::config::MAX_APP_NUM;
use crate::config::MAX_SYSCALL_NUM;
use crate::loader::{get_num_app, init_app_cx};
use crate::sync::UPSafeCell;
use crate::syscall::TaskInfo;
use crate::timer::get_time_us;
// use crate::timer::get_time_us;
use lazy_static::*;
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

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

/// Inner of Task Manager
pub struct TaskManagerInner {
    /// task list
    tasks: [TaskControlBlock; MAX_APP_NUM],
    /// id of current `Running` task
    current_task: usize,
}

lazy_static! {
    /// Global variable: TASK_MANAGER
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [TaskControlBlock {
            // 创建MAX_APP_NUM个 任务控制块 用0初始化任务上下文TaskContext
            task_cx: TaskContext::zero_init(),
            task_status: TaskStatus::UnInit,
            // task_info:TaskInfo::init(),
            task_info: TaskInfo { status: TaskStatus::UnInit, syscall_times: [0;MAX_SYSCALL_NUM], time: 0 },
            start_time: 0,
        }; MAX_APP_NUM];
        for (i, task) in tasks.iter_mut().enumerate() {
            // 每个任务控制块里的 任务上下文进行 实际初始化
            // 第一次进入用户态需要的数据 ra=__resore, sp=trapContextPtr
            // 这样，任务管理器中各个应用的任务上下文就得到了初始化。16个全部初始化了
            task.task_cx = TaskContext::goto_restore(init_app_cx(i));
            task.task_status = TaskStatus::Ready;
            task.task_info.status = TaskStatus::Ready;
            task.start_time = get_time_us();
            // task.task_info.time = get_time_us();
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
    /// But in ch3, we load apps statically, so the first task is a real app.
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
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
        let current_task = inner.tasks[inner.current_task]; // 已经clone了一份
        // current_task.time_elapsed();
        // current_task.task_info.status = current_task.task_status;
        // current_task.task_info.syscall_times = 
        // println!("{:?}",current_task.task_info.syscall_times);
        current_task.task_info.clone()
    }

    /// get current task control block
    fn get_current_tcb_copy(&self) -> TaskControlBlock{
        // self.update_current_task_info();
        let inner = self.inner.exclusive_access();
        let current_task = inner.tasks[inner.current_task].clone();
        // current_task.time_elapsed();
        // current_task.task_info
        current_task
    }

    fn incrument_syscall_calling_times(&self,syscall_id:usize){
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let current_task_tcb = &mut inner.tasks[current];
        // current_task_tcb.time_elapsed();
        // current_task_tcb.task_info
        current_task_tcb.syscall_record_update(syscall_id);
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

/// Get current running taskControlBlock instance
pub fn get_current_tcb_copy() -> TaskControlBlock{
    TASK_MANAGER.get_current_tcb_copy()
}

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