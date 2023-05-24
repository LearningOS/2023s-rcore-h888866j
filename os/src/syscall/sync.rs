use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task, TaskControlBlock};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec::Vec;
use crate::task::{RESOURCE_CATEG_NUM, MAX_THREAD_NUM};
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    let avail = process_inner.available.clone();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        // 创建锁资源时 维护 AVIL，即可用资源数量（资源种类就是锁的id）
        avail.exclusive_access()[id] += 1;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        // 创建锁资源时 维护 AVIL，即可用资源数量（资源种类就是锁的id）
        avail.exclusive_access()[process_inner.mutex_list.len()-1] += 1;
        process_inner.mutex_list.len() as isize - 1
    }
}

/// 安全性算法 检测死锁，检测到可能出现死锁 就返回 -0xDEAD, 没有检测到返回 0
fn deadlock_detect(res_id:usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
            // fn deadlock_detect(res)
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid; 
    // let enable_deadlock_detect = process_inner.enable_deadlock_detect;
    let tasks = process_inner.tasks.clone();
    let avail = Arc::clone(&process_inner.available);
    let needed = process_inner.needed.clone();
    let allocation = process_inner.allocation.clone();
    drop(process_inner);
    drop(process);

    // // 需要的资源 线程tid 需要的资源 mutex_id 加一
    // NEEDED.exclusive_access()[tid][res_id] += 1;
    // if enable_deadlock_detect {
    info!("thread: {}", tid);
    info!("thread: {}, step 1", tid);
    // rCore内核态不会被打断
    // 每次检查死锁出现的可能性时，都需要遍历?
    let mut finish = [true;MAX_THREAD_NUM];
    // finish[0] = true;
    let mut work = [0_u8;RESOURCE_CATEG_NUM];
    for (i,v) in work.iter_mut().enumerate(){
        *v = avail.exclusive_access()[i];
    }
    // let mut work = avail.exclusive_access().clone();
    // 先将 已有线程的 finish 标记改为 false， 不存在的 线程就还是true
    let _x = tasks
        .iter()
        .filter(|&x|x.is_some())
        .map(|s|s.as_ref().unwrap())
        .filter(|&h|h.inner_exclusive_access().res.is_some())
        .map(|t|{
                let this_tid = t.inner_exclusive_access().res.as_ref().unwrap().tid;
                finish[this_tid] = false;
                t
        }).collect::<Vec<&Arc<TaskControlBlock>>>(); 
    loop {
        let  one_task = tasks
        .iter()
        .filter(|&x|x.is_some())
        .map(|s|s.as_ref().unwrap())
        .filter(|&h|h.inner_exclusive_access().res.is_some())
        .find(|&t|{
        // 第二步 从线程集合中 找 满足条件的线程
        // 条件是 还没能完成 + 需要的资源 少于 可用资源
        let this_tid = t.inner_exclusive_access()
                                .res
                                .as_ref()
                                .unwrap()
                                .tid;
        info!("step2 find: NEEDED.exclusive_access()[{}][{}]={:?}",
                this_tid,res_id,needed.exclusive_access()[this_tid][res_id]);

        // // 只看 传入参数 mutex_id 一个资源的 申请与可用 情况，死锁还是会发生
        // (finish[this_tid] == false) && 
        // (NEEDED.exclusive_access()[this_tid][mutex_id] <= work[mutex_id])

        // let mut flag = false;
        // // 需要的资源 needed 
        // let mut work_u64:[u64;512] = [0;512];
        // for (i,v) in work.iter().enumerate(){
        //     work_u64[i] = *v as u64;
        // }
        // for (certain_sem_id,j) in (&work_u64[0..5]).iter().enumerate(){
        //     if !(finish[this_tid] == false){
        //         break
        //     }
        //     info!("this_tid:{}, certain_sem_id:{}, value needed{} <=? res in work{}",
        //                 this_tid, certain_sem_id, NEEDED.exclusive_access()[this_tid][certain_sem_id], work[certain_sem_id]);
        //     if NEEDED.exclusive_access()[this_tid][certain_sem_id] <= work[certain_sem_id] {
        //         flag = true;
        //     }else{
        //         flag = false;
        //         break
        //     }
        // }
        let mut flag = true;
        if finish[this_tid] == false {
            for (certain_mutex_id,_j) in work.iter().enumerate(){
                if  needed.exclusive_access()[this_tid][certain_mutex_id] > work[certain_mutex_id] {
                    flag = false;
                }
            }  
        }  
        info!("flag: {} && finish[this_tid] == false: {}", flag, finish[this_tid] == false);
        (finish[this_tid] == false) && flag

        });
        if let Some(tt) = one_task{            
        // 能找到 进入第三步                
        // 系统 有 足够资源分配给 this_tid, 假定分配资源让其运行，
        // 先让其运行

        // 运行后 归还这个线程占用的资源
        let this_tid = tt.inner_exclusive_access()
                                .res
                                .as_ref()
                                .unwrap()
                                .tid;
        // step 3
        // 归还到 WORK
        for j in 0..RESOURCE_CATEG_NUM{
            work[j] += allocation.exclusive_access()[this_tid][j];
        }
        // 标记这个线程 可以完成
        finish[this_tid] = true;                
        info!("step3:  tid: {} could finish its job, goto step2. finish: {:?}",this_tid, &finish[0..5]);

        }else{
            // 不能找到，进入第四步
            info!("step3: condition not met, no task can be found, finish: {:?}", &finish[0..5]);
            break
        }
    }

    if finish.iter().all(|&t| t == true){
        info!("step 4: safe.......");
        // return 0
    }else{
        info!("step4:not all the task thread can be finished when lock res: {}",res_id);
        info!("step4: dead lock!!!!");
        // 还有不能完成的 task，说明出现死锁可能，拒绝这次 锁的 获取，返回 -xDEAD
        needed.exclusive_access()[tid][res_id] -= 1; // 需要减去这个吗？
        return -0xdead
    }
    0
}


/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let enable_deadlock_detect = process_inner.enable_deadlock_detect;
    let avail = process_inner.available.clone();
    let allocation = process_inner.allocation.clone();
    let needed = process_inner.needed.clone();
    let tid = current_task()
                    .unwrap()
                    .inner_exclusive_access()
                    .res
                    .as_ref()
                    .unwrap()
                    .tid; 
    drop(process_inner);
    drop(process);
    
    // 需要的资源 线程tid 需要的资源 mutex_id 加一
    needed.exclusive_access()[tid][mutex_id] += 1;
    if enable_deadlock_detect {
        match deadlock_detect(mutex_id){
            0 => mutex.lock(),
            x => return x,
        }
    }else{
        mutex.lock()
    }
 
    // info!("tid: {}, lock succeed, lock:{}",tid,mutex_id);

    // 不能放在 if 里
    // if enable_deadlock_detect {
    // 在 成功获取 锁资源之后
    // 已获取锁，维护已分配资源 
    allocation.exclusive_access()[tid][mutex_id] += 1;
    // info!("ALLOCATION: {:?}",ALLOCATION.exclusive_access());
    // 维护 剩余资源 
    avail.exclusive_access()[mutex_id] -= 1;
    info!("Avail: {:?}",&avail.exclusive_access()[..5]);
    // 维护 还需要的资源
    needed.exclusive_access()[tid][mutex_id] -= 1; 
    0
}

/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let avail = process_inner.available.clone();
    let allocation = process_inner.allocation.clone();
    // let needed = process_inner.needed.clone();
    let tid = current_task()
                    .unwrap()
                    .inner_exclusive_access()
                    .res
                    .as_ref()
                    .unwrap()
                    .tid; 
    drop(process_inner);
    drop(process);
    mutex.unlock();

    // 释放 锁资源 之后，维护 已分配资源
    allocation.exclusive_access()[tid][mutex_id] -= 1;
    // WORK.exclusive_access()[mutex_id] += 1;
    // 维护 可用 资源，循环中包含本次释放的锁资源 xxx 错误
    // for j in 0..512{
    //     AVAIL.exclusive_access()[j] += ALLOCATION.exclusive_access()[tid][j];// 每个锁释放都这么来，会多次维护造成资源数目
    // }
    avail.exclusive_access()[mutex_id] += 1; // 这个锁的释放系统调用中 只 维护这个锁的数据
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let avail = process_inner.available.clone();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));        
        // 创建锁资源时 维护 AVIL，即可用资源数量（资源种类就是锁的id）
        avail.exclusive_access()[id] += res_count as u8;
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));        
        // 创建锁资源时 维护 AVIL，即可用资源数量（资源种类就是锁的id）
        avail.exclusive_access()[process_inner.semaphore_list.len() - 1] += res_count as u8;
        process_inner.semaphore_list.len() - 1
    };

    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let avail = process_inner.available.clone();
    let allocation = process_inner.allocation.clone();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let tid = current_task()
                    .unwrap()
                    .inner_exclusive_access()
                    .res
                    .as_ref()
                    .unwrap()
                    .tid; 
    drop(process_inner);
    sem.up();

    // 释放 锁资源 之后，维护 已分配资源
    allocation.exclusive_access()[tid][sem_id] -= 1;
    avail.exclusive_access()[sem_id] += 1; // 这个锁的释放系统调用中 只 维护这个锁的数据
    info!("avail: {:?}",&avail.exclusive_access()[..5]);
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let tid = current_task()
                    .unwrap()
                    .inner_exclusive_access()
                    .res
                    .as_ref()
                    .unwrap()
                    .tid; 
    let enable_deadlock_detect = process_inner.enable_deadlock_detect;
    let avail = process_inner.available.clone();
    let allocation = process_inner.allocation.clone();
    let needed = process_inner.needed.clone();
    // let tasks = process_inner.tasks.clone();
    drop(process_inner);
    drop(process);

    // 需要的资源 线程tid 需要的资源 mutex_id 加一
    // info!("before add: NEEDED.exclusive_access()[{}][{}]={:?}",tid,sem_id,NEEDED.exclusive_access()[tid][sem_id]);
    needed.exclusive_access()[tid][sem_id] += 1;
    // info!("after  add: NEEDED.exclusive_access()[{}][{}]={:?}",tid,sem_id,NEEDED.exclusive_access()[tid][sem_id]);
    
    if enable_deadlock_detect {
        match deadlock_detect(sem_id){
            0 => sem.down(),
            x => return x,
        }
    }else{
        sem.down();
    }

    // info!("tid: {}, lock succeed, lock:{}",tid,sem_id);


    // 不能放在 if 里
    // if enable_deadlock_detect {
    // 在 成功获取 锁资源之后
    // 已获取锁，维护已分配资源 
    allocation.exclusive_access()[tid][sem_id] += 1;
    // info!("ALLOCATION: {:?}",ALLOCATION.exclusive_access());
    // 维护 剩余资源 
    avail.exclusive_access()[sem_id] -= 1;
    info!("Avail: {:?}",&avail.exclusive_access()[..5]);
    // 维护 还需要的资源
    needed.exclusive_access()[tid][sem_id] -= 1; 
    
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect NOT IMPLEMENTED");
    let process = current_process();
    let enable = match _enabled{
        0 => false,
        1 => true,
        _ => return -1
    };
    let ret = process.inner_exclusive_access().sys_enable_deadlock_detect(enable);
    drop(process);
    info!("dead lock check enable succeed");
    // -1
    ret
}
