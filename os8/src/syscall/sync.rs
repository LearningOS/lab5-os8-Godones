use alloc::collections::{BTreeMap, BTreeSet};
use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::syscall::thread::sys_gettid;
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec::Vec;

pub fn sys_sleep(ms: usize) -> isize {
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}

// LAB5 HINT: you might need to maintain data structures used for deadlock detection
// during sys_mutex_* and sys_semaphore_* syscalls
pub fn sys_mutex_create(blocking: bool) -> isize {
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();

    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        process_inner.allocated_mutex[id]=None; //建立锁与对应线程关系
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        let index= process_inner.mutex_list.len()-1;
        process_inner.allocated_mutex[index] = None;
        process_inner.mutex_list.len() as isize - 1

    }
}

// LAB5 HINT: Return -0xDEAD if deadlock is detected
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mut tid = sys_gettid() as usize;
    process_inner.request_mutex[tid] = Some(mutex_id);
    let mut request_road = BTreeSet::<usize>::new();
    request_road.insert(tid);
    let mut mutex_id = mutex_id;
    if process_inner.enable_lock_detect{
        // 根据图算法检测是否存在线程之间的依赖关系
        // 如果发生A拥有锁a,B拥有锁b,A调用锁b,B调用锁a,则可能存在线程之间的依赖关系
       while let Some(t) = process_inner.allocated_mutex[mutex_id] {
            if request_road.get(&t).is_some(){
                return -0xDEAD; //出现环路
            }
            request_road.insert(t);
            if let Some(id) = process_inner.request_mutex[t] {
                mutex_id = id;
            }else {
                break;
            }
        }
    }

    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.lock();
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.allocated_mutex[mutex_id] = Some(tid);
    process_inner.request_mutex[tid] = None;
    0
}

pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.allocated_mutex[mutex_id] = None;
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}

pub fn sys_semaphore_create(res_count: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        process_inner.available_semaphore[id] = res_count;
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        let len = process_inner.semaphore_list.len()-1;
        process_inner.available_semaphore[len] = res_count;
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}

pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let tid = sys_gettid() as usize;
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.allocated_semaphore[tid][sem_id] -= 1;
    process_inner.available_semaphore[sem_id] += 1;
    0
}

// LAB5 HINT: Return -0xDEAD if deadlock is detected
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    // 银行家算法求解是否发生死锁
    let tid = sys_gettid() as usize;
    process_inner.request_semaphore[tid][sem_id] +=1;
    if process_inner.enable_lock_detect {
        let mut work = process_inner.available_semaphore; //可用资源
        let mut finish:Vec<usize>= Vec::new(); //所有线程都未结束
        process_inner.tasks.iter().for_each(|task|{
            if let Some(task) = task{
                if let Some(res) = task.inner_exclusive_access().res.as_ref(){
                    finish.push(res.tid);
                }
            }
        });
        let sem_num = process_inner.semaphore_list.len();
        let mut ok = false;
        while !ok {
            let mut find = true;
            for (index,tid) in finish.iter().enumerate() {
                //检查需求向量
                find = true;
                let req = process_inner.request_semaphore[*tid];
                for i in 0..sem_num{
                    if req[i] > work[i]{
                        find = false;
                        break;
                    }
                }
                if find{
                    // 更新对应的计数值
                    let allocated = process_inner.allocated_semaphore[*tid];
                    for i in 0..sem_num{
                        work[i] += allocated[i];
                    }
                    finish.remove(index);
                    break;
                }
            }
            if !find&&!finish.is_empty() {
                // 找不到一个可以运行的线程
                return -0xDEAD;
            }
            if finish.is_empty() {
                // 所有线程都已经结束
                ok = true;
            }
            info!("loop in find");
        }
    }
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.down();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.allocated_semaphore[tid][sem_id] += 1;
    process_inner.available_semaphore[sem_id] -= 1;
    process_inner.request_semaphore[tid][sem_id] -= 1;
    0
}

pub fn sys_condvar_create(_arg: usize) -> isize {
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

pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}

pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}

// todo!LAB5 YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.enable_lock_detect = enabled == 1;
    0
}
