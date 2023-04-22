//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM,
    task::{exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, get_current_task_info},
    timer::get_time_us,
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
#[derive(Copy,Clone)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    pub status: TaskStatus,
    /// The numbers of syscall called by task
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    pub time: usize,
    // start time of a task
    // pub start_time: usize,
}

// // #[allow(dead_code)]
// impl TaskInfo{
//     /// init a new taskinfo
//     pub fn init() -> TaskInfo{
//         // let time1 = get_time_us();
//         TaskInfo { 
//             status: TaskStatus::UnInit, 
//             syscall_times: [0;MAX_SYSCALL_NUM], 
//             time: 0,
//             // start_time:get_time_us()
//         }
//     }
//     // /// record syscall
//     // pub fn syscall_record(&mut self, sys_call_id: usize){
//     //     self.syscall_times[sys_call_id] += 1;
//     // }
//     // /// caculate time elapsed and return it
//     // pub fn time_elapsed(&mut self) -> usize{
//     //     // self.time = get_time_us() - self.start_time;
//     //     self.time
//     // }
// }

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    let ti = get_current_task_info();
    // println!("task time elapsed:{:?}",ti.time);
    // println!("syscall SYSCALL_GET_TIME 169 was called for ：{:?} times",ti.syscall_times[169]);
    // println!("syscall SYSCALL_TASK_INFO 410 was called for ：{:?} times",ti.syscall_times[410]);
    unsafe {
        *_ti = ti;
    }
    trace!("kernel: sys_task_info");
    0
}
