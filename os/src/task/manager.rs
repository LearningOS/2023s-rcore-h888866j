//!Implementation of [`TaskManager`]
use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
// use crate::config::BIG_STRIDE;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        // // self.ready_queue.pop_front()
        // let x = self.ready_queue
        // .iter()
        // .enumerate()
        // .min_by_key(|(_,task1)|
        // BIG_STRIDE/task1.inner_exclusive_access().priority
        // );
        // self.ready_queue.pop_front()
        // println!("{:?}",self.ready_queue);
        let x = self.ready_queue
        .iter()
        .enumerate()
        .min_by_key(|(_,task1)|
            task1.inner_exclusive_access().stride
        );
        // println!("TaskManager.fetch() x.isSome: {:?}",x.is_some());
        if let Some((index,_)) = x {
            // println!("found a smallest stride in ready queue, index :{}",index);
            // println!("len of readyqueue: {}",self.ready_queue.len());
            let  task = self.ready_queue.remove(index);            
            // add pass to stride immediately after fetch
            // task = task.map(| x|{
            //     let mut tcb = x.inner_exclusive_access();
            //     let priority = tcb.priority;
            //     tcb.stride += BIG_STRIDE / priority;
            //     drop(tcb);
            //     x
            // }
            // );
            // task
            
            // add pass to stride immediately after fetch
            // if let Some(ref mut tcb) = task {
            //     let priority = tcb.inner_exclusive_access().priority;
            //     tcb.inner_exclusive_access().stride += BIG_STRIDE / priority;
            // }

            task
        }else{
            println!("No task in ready queue");
            None
        }
    }
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Add process to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    TASK_MANAGER.exclusive_access().add(task);
}

/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}
