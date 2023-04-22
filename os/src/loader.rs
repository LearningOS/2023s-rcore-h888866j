//! Loading user applications into memory
//!
//! For chapter 3, user applications are simply part of the data included in the
//! kernel binary, so we only need to copy them to the space allocated for each
//! app to load them. We also allocate fixed spaces for each task's
//! [`KernelStack`] and [`UserStack`].

use crate::config::*;
use crate::trap::TrapContext;
use core::arch::asm;

/// 内核栈数据结构，data字段用数组表示一个 KERNEL_STACK_SIZE长度的字节区域
#[repr(align(4096))]
#[derive(Copy, Clone)]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

/// 用户栈数据结构，data字段用数组表示一个 KERNEL_STACK_SIZE长度的字节区域
#[repr(align(4096))]
#[derive(Copy, Clone)]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

/// 内核栈 根据最大应用数目，创建相应数量的 内核栈数组
/// 内核栈大小由常量KERNEL_STACK_SIZE提供
static KERNEL_STACK: [KernelStack; MAX_APP_NUM] = [KernelStack {
    data: [0; KERNEL_STACK_SIZE],
}; MAX_APP_NUM];

static USER_STACK: [UserStack; MAX_APP_NUM] = [UserStack {
    data: [0; USER_STACK_SIZE],
}; MAX_APP_NUM];

impl KernelStack {
    /// 获取这个KernalStack的 stack pointer / 栈顶地址
    /// 
    /// 因为还没 存东西，所以self.data.as_ptr() as usize + KERNEL_STACK_SIZE，往上到栈顶，以后存数据慢慢往下存
    fn get_sp(&self) -> usize {
        
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }
    /// Push TrapContext 到 KernalStack上
    /// 
    /// 包含 在内核栈上分配空间，拿到栈顶指针地址，转换为TrapContext类型的裸指针
    /// 
    /// 解引用裸指针，再将传入参数赋值给他
    /// 
    /// 然后 这个sp指向的就是 内核栈上的 TrapContext
    /// 
    /// 返回trap_cx_ptr 也就是压入 Trap 上下文后内核栈的sp
    pub fn push_context(&self, trap_cx: TrapContext) -> usize {
        // 在这个KernalStack上 分配空间 给TrapContext
        // sp - TrapContext大小 就是sp下移。
        let trap_cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *trap_cx_ptr = trap_cx;
        }
        trap_cx_ptr as usize
    }
}

impl UserStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

/// Get base address of app i.
fn get_base_i(app_id: usize) -> usize {
    APP_BASE_ADDRESS + app_id * APP_SIZE_LIMIT
}

/// Get the total number of applications.
pub fn get_num_app() -> usize {
    extern "C" {
        fn _num_app();
    }
    unsafe { (_num_app as usize as *const usize).read_volatile() }
}

/// Load nth user app at
/// [APP_BASE_ADDRESS + n * APP_SIZE_LIMIT, APP_BASE_ADDRESS + (n+1) * APP_SIZE_LIMIT).
pub fn load_apps() {
    extern "C" {
        fn _num_app();
    }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    let app_start = unsafe { core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1) };
    // clear i-cache first
    unsafe {
        asm!("fence.i");
    }
    // load apps
    for i in 0..num_app {
        let base_i = get_base_i(i);
        // clear region
        (base_i..base_i + APP_SIZE_LIMIT)
            .for_each(|addr| unsafe { (addr as *mut u8).write_volatile(0) });
        // load app from data section to memory
        let src = unsafe {
            core::slice::from_raw_parts(app_start[i] as *const u8, app_start[i + 1] - app_start[i])
        };
        let dst = unsafe { core::slice::from_raw_parts_mut(base_i as *mut u8, src.len()) };
        dst.copy_from_slice(src);
    }
}

/// get app info(trap context) with entry and sp using `TrapContext::app_init_context` function
/// 
/// and save `TrapContext` in kernel stack using `push_context` method of `KernelStack`
/// 
/// return trap_cx_ptr， it is also the `sp` of KernelStack
pub fn init_app_cx(app_id: usize) -> usize {
    // todo!("push contexgt 方法只push了trapcontext，那taskcontext怎么保存到该应用内核栈上的呢？");
    KERNEL_STACK[app_id].push_context(TrapContext::app_init_context(
        get_base_i(app_id),
        USER_STACK[app_id].get_sp(),
    ))
}
