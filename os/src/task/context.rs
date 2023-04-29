//! Implementation of [`TaskContext`]
use crate::trap::trap_return;

#[repr(C)]
/// task context structure containing some registers
///
/// 为什么只有ra,sp,s0-s12需要被保存？
///
/// 因为是在内核态同一特权级切换task(trap控制流中)，
///
/// 调用 __switch 函数时的 开场白会帮我们保存调用者保存寄存器
pub struct TaskContext {
    /// Ret position after task switching
    ra: usize,
    /// Stack pointer
    sp: usize,
    /// s0-11 register, callee saved
    s: [usize; 12],
}

impl TaskContext {
    /// Create a new empty task context
    pub fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }
    /// Create a new task context with a trap return addr and a kernel stack pointer
    /// 
    /// return TaskContext instance
    ///
    /// ch3: goto_restore 保存传入的 kstack_ptr（trapContextPtr）到sp字段，
    /// 
    /// ch4: goto_trap_return 保存传入的 kstack_ptr（appKernalSpaceStackPtr）到sp字段，
    ///
    /// ch3:并将 ra字段 设置为 __restore 的入口地址，构造任务上下文后返回。
    /// 
    /// ch4:并将 ra字段 设置为 trap_return 的入口地址，构造任务上下文后返回。
    pub fn goto_trap_return(kstack_ptr: usize) -> Self {
        Self {
            ra: trap_return as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
