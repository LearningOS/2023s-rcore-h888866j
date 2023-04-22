//! Types related to task management

use crate::timer::get_time_us;

use super::TaskContext;
use super::super::syscall::*;
/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
    /// The task info
    pub task_info: TaskInfo,
    /// start time of the task
    pub start_time: usize,
}
impl TaskControlBlock{
    /// calc time elapsed, trasnlate to ms.
    pub fn time_elapsed(&mut self) -> usize {        
        self.task_info.time = (get_time_us() - self.start_time)/1000;
        self.task_info.time
    }
    /// record syscall calling times
    pub fn syscall_record_update(&mut self, syscall_id:usize){

        self.task_info.syscall_times[syscall_id] += 1;
    }
}

/// The status of a task
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}
