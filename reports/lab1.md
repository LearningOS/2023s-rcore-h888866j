## lab实验报告 
计算任务执行经历的时间，需要任务的开始时间，开始把 `start_time` 字段 放到了 `TaskInfo` 里，发现用户程序使用了的 `TaskInfo` 数据结构没有这个字段。那放这里不合适。后来就放到 TCB 里并且把 TaskInfo 保存为 TCB的一个字段。

在实现 系统调用计数时候，感觉思路是正确的，但是统计的 数字就是 0. 后来诊断发现在累加函数内 确实实现了累加。但是在函数外面 打印发现系统调用次数还是0。 那只能是 累加的数据没有保存下来。后来发现应该获取可变引用进行累加操作，因为tcb 等都是实现了copy的，复制了一份数据给我，在里面累加没有保存到源统计数据里去。

中间 通过 __switch 汇编代码梳理了一遍任务切换流程，放在文末。

## 荣誉准则
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

        《你交流的对象说明》

2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

        《你参考的资料说明》

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

## 简答作业
深入理解 trap.S 中两个函数 __alltraps 和 __restore 的作用，并回答如下问题:

### L40：刚进入 __restore 时，a0 代表了什么值。请指出 __restore 的两种使用情景。
`a0` 是 `trap_handler` 的返回值， 也就是当前 app 的trap Context指针地址
- 第一种使用情景：
    用户态应用运行 trap 到内核，内核完成处理，准备回到 用户态时使用 `__restore`
- 第二种使用情景：
    应用被 `__switch` 到时，用 `__restore` 进入用户态运行
### L43-L48：这几行汇编代码特殊处理了哪些寄存器？这些寄存器的的值对于进入用户态有何意义？请分别解释。
```shell
    # sp 是当前app 内核栈栈顶sp、 TrapContext 地址
    # restore sstatus/sepc
    ld t0, 32*8(sp) # 从内核栈TrapContext里读取 sstatus 保存到t0
    ld t1, 33*8(sp) # 从内核栈TrapContext里读取 sepc 保存到t1
    ld t2, 2*8(sp)  # 从内核栈TrapContext里读取 sp, 这是trap到内核的时候保存的用户栈sp
    csrw sstatus, t0  # 恢复 sstatus，
    csrw sepc, t1     # 恢复 sepc，返回用户态时要从这个地址开始运行
    csrw sscratch, t2 # 恢复 sscratch 其应该指向 user stack, 返回用户态前要使用
```
### L50-L56：为何跳过了 x2 和 x4？
```shell
ld x1, 1*8(sp)
#  这里为什么不恢复x2/sp呢, 因为现在sp指向app内核栈栈顶,我们还要用它的地址 读取并恢复 其他寄存器的值
ld x3, 3*8(sp)
# 没有保存tp不知道为什么
.set n, 5
.rept 27
   LOAD_GP %n
   .set n, n+1
.endr
```
### L60：该指令之后，sp 和 sscratch 中的值分别有什么意义？
```
csrrw sp, sscratch, sp
```

该指令之后，
`sp`指向用户态栈顶 方便用户态程序正常执行，

`sscratch` 指向 该app内核栈栈顶，再次 trap 到内核态的时候还要这么交换一下，方便在内核栈保存数据

### __restore：中发生状态切换在哪一条指令？为何该指令执行之后会进入用户态？
`sret` 返回用户态，因为当前在 S 态， `sret`返回用户态

### L13：该指令之后，`sp` 和 `sscratch` 中的值分别有什么意义？
```
csrrw sp, sscratch, sp
```
`sp` 指向该app 内核栈栈顶，方便分配栈帧空间 和 保存寄存器。 

`ssctratch` 保存 用户态栈顶sp，方便 restore的时候 给`sp`赋值，返回用户态就能正在执行了

### 从 U 态进入 S 态是哪一条指令发生的？
`ecall`

## switch.S
```shell
.altmacro
.macro SAVE_SN n
    sd s\n, (\n+2)*8(a0)
.endm
.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)
.endm
    .section .text
    .globl __switch
__switch:
    # __switch(
    #     current_task_cx_ptr: *mut TaskContext,
    #     next_task_cx_ptr: *const TaskContext
    # )
    # save kernel stack of current task
    # __switch 被解释成一个函数
    # 参数 current_task_cx_ptr 根据调用约定保存在 a0 寄存器
    # 第一次 __switch 时 a0参数= current_task_cx_ptr ，它是0初始化的taskContext，里面是假数据sp=0
    # 所有被切换任务第一次被 切换到 的时候 a1 = next/first_task_cx_ptr (数据结构里面，ra=__restore,sp=trap_ctx_ptr)
    # 第二次及后续 切换 a0 = current_task_cx_ptr(已存在), a1 = next_task_cx_ptr(第一次被切换到就看上面),
    # 第二次及后续 被切换到 时, a1 = next_task_cx_ptr,但是它里面包的数据是 前面被切走时保存的数据
    # 确定 a0,a1 再来看看 当前 sp 指向什么 , 先看 第二次及后续 切换

    # ==== 第二次及后续调用 __switch 时 sp指向问题 开始 ====
    # 而 第二次及后续调用 __switch 时 a0= current_task_cx_ptr 就是当前app内核栈上 任务上下文的指针，
    # 也是 该任务内核栈的 sp (每个app/任务都有自己的内核栈)    
    # a0 现在是app内核栈栈顶sp，app内核栈栈顶的栈帧是 taskContext， 
    # taskContext 的指针地址 或者说 taskContext 的sp就是 app内核栈sp
    # ==== 第二次及后续调用 __switch 时 sp指向问题 结束 ====
    # 那第一次调用 __switch 的时候呢？ 
    # ====== 第一次调用 __switch 时 sp指向问题 开始 ====
    # 我们是在内核态 调用的 __switch
    # 第一次调用__switch是在 run_first_task里, __switch(用0初始化的空的TaskContextPtr,firstTaskContextPtr)
    # 不会切换回zero_init_taskContext，因为 第一个任务之后都是选取 Ready 的任务进行切换
    # 这时 sp 指向哪里？
    # run_first_task是在 rust_main 函数被调用的，rust_main是入口 entry.asm 设定的rust入口函数
    # entry.asm 中 设置了 la sp, boot_stack_top
    # rust_main 先调用了几个其他关联函数 初始化环境,之后调用的 run_first_task
    # 所以此时应该是指向 内核自身的 内核栈 栈顶
    # ====== 第一次调用 __switch 时 sp指向问题 结束 =====
    # run_first_task 其实是调用 TASK_MANAGER.run_first_task();
    # TASK_MANAGER 是lazy_static! 现在才开始创建 TaskManager
    # 此时 根据 MAX_APP_NUM=16(对应的USER_STACK也是16个位置) 创建 TaskManagerInner 的字段 tasks: TaskControlBlock 数组 (UnInit状态,zeroInit的taskContext)
    # 对已有16任务,进行初始化,包含更改任务状态到Ready和构建taskContext(用了TaskContext::goto_restore(init_app_cx(i)))
    # === 任务上下文初始化 开始 ======
    # 这run_first_task里第一个被切换 到 的任务firstTaskContext
    # 是通过 TaskContext::goto_restore(init_app_cx(i));这个关联函数进行初始化的，（每个任务都是这样初始化的）
    # 这个关联函数 做了一件事共三小步，其中最麻烦的一步是 要传入新建trapContext指针转换得来的usize（init_app_cx返回值）
    # 返回一个构建的任务上下文结构体 TaskContext{ra:__restore地址,sp：传入参数（该任务trapContext指针）,s:[0;12]}
    # === 任务上下文初始化 结束 ======
    # 这个init_app_cx(i) 其实应该称为 init_app_trap_cx(i),因为它构造了一个trapContext，i是任务编号
    # 这个函数实际上 调用 KERNEL_STACK[app_id].push_context，返回的是TrapContext指针转换成的usize
    # 下面看看 怎么构建trapContext 又怎么push到内核栈上
    # ================ push TrapContext 到该任务的内核栈上去 开始 ========
    # push_context push了一个 trapContext 到该任务的 内核栈上去，包含在栈上分配空间，下面展开讲
    # 用TrapContext::app_init_context 构造一个 trapContext实例，作为实参传给 push_context 
    # push_context 先拿该app的kernelStack的sp，self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    # 然后在该app的内核栈上分配TrapContext的栈空间 
    # trap_cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;拿到新的sp
    # 然后再把参数（实参trapContext实例）赋值给 通过解引用trap_cx_ptr（栈顶指针）， 拷贝数据到分配好的栈空间里去
    # ================ push TrapContext 到该任务的内核栈上去 结束 ========
    # 再回过头看 trapContext实例 怎么创建的
    # ========= 构建 TrapContext 开始 ======
    # TrapContext::app_init_context(entry,sp) 获取并更改 sstatus CSR 状态，设定 之前的特权级模式 为用户态！！
    # 创建一个TrapContext，保存所有通用寄存器（都是0） 
    # + sstatus寄存器 (刚修改过 之前的特权级改成了 用户态)
    # + sepc寄存器（实参entry=get_base_i(app_id),， 表面从app起始位置开始运行，因为只有第一次需要构建，后面trap到内核态就有trap上下文也有ra了）
    # trapContext 里x2/sp 值设置为app_init_context的第二个参数sp
    # 实参sp=USER_STACK[app_id].get_sp(), get_sp就是self.data.as_ptr() as usize + USER_STACK_SIZE
    # ========= 构建 TrapContext 结束 ======

    # __switch 汇编代码
    sd sp, 8(a0)
    # 为什么要保存 sp 呢？是存储在 a0 向上移动8字节 的地方 （ 保存在currenttaskContext的数据结构中共后续恢复时使用）
    # 先看看a0 是什么, current_task_context_ptr, 内核调用的__switch, 
    # 根据前面的解释,第一次调用 __switch的时候 sp是内核自身的内核栈sp
    # 后续调用 __switch 时(run_next_task), 是 任务A trap到内核 后
    # 根据 trap.S, csrrw sp, sscratch, sp # now sp->kernel stack, sscratch->user stack
    # allocate a TrapContext on kernel stack


    # TaskContext数据结构 TaskContext{ra,sp,s:[0;12]}
    # taskContext 前8字节保存的是ra，接下来8字节保存的是 sp, 这里我们先保存了sp
    # 现在是在__switch函数内，被调用函数需要保存 ra 寄存器 和一些被调用者保存寄存器
    # save ra & s0~s11 of current execution
    sd ra, 0(a0)
    .set n, 0
    .rept 12
        SAVE_SN %n
        .set n, n + 1
    .endr
    # restore ra & s0~s11 of next execution
    # 参数next_task_cx_ptr根据调用约定保存在 a1 寄存器
    # 它也是下一个要切换到的任务 的内核栈栈顶 sp
    # 根据这个sp（这里用a1代替），取出8字节数据，保存在 ra 寄存器中，
    # 因为 TaskContext 数据结，先保存的是ra，然后是sp，然后是其他 被调用者保存寄存器
    ld ra, 0(a1) # 恢复 ra
    # 恢复被调用者保存寄存器, 从next_task_context里面取出数据,保存到相应寄存器中
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n + 1
    .endr
    # restore kernel stack of next task
    # 根据当前的 a1，也就是 next_task_context_ptr, next_task 内核栈栈顶sp值（nextTaskContext指针），
    # 找到保存在nextTaskContext中的第二个字段 `sp`, 它是 next任务的TrapContextPtr, next task trap的sp
    ld sp, 8(a1)
    # 这就是任务切换,保存A任务的状态到内存,取出B任务的状态放到寄存器里
    # 返回，不切换特权级
    # 返回到哪里？ 到 ra 寄存器保存的位置那里！
    # ra 里保存的是啥 ？
    # 第一次 init taskContext 的时候，在它里面保存的是 汇编标记 __restore 的地址！
    ret
    # __switch 函数结束,出来了,出来之后去哪了?
    # 如果 next_task 是第一次运行时，转到 __restore 位置继续运行,去到用户态

    # 如果 next_task 已经运行过一次之后，那么刚刚恢复出来的 ra 就是 该任务前一次 切换到其他任务时
    # 保存到 taskContext 的第一个字段 ra： /// Return position after task switching
    # 看看上面我们保存了什么 
    # 是 调用 __switch 函数 并且 到 __switch 函数里时的 由__switch 保存的 ra，
    # 这个 ra 就指向了 调用 __switch 函数的地方
    # 我们在哪里调用了 __switch 呢？
    # TaskManager.run_next_task 函数
    # 随后调用 __switch 结束了，然后调用它的这个函数 TaskManager.run_next_task(&self)也结束返回了
    # 随后 调用 TaskManager.run_next_task(&self) 的 pub fn run_next_task 结束返回
    # 随后 调用 run_next_task 的 suspend_current_and_run_next 或者 exit_current_and_run_next 返回
    # 谁又调用了这两个函数呢？
    # sys_yield , sys_exit 等 系统提供的 系统调用实现里 调用的，
    # 之前 该任务!! trap到内核态的时候 才会通过 trap_handler 运行这些系统提供的 系统调用函数 
    # trap_handler 结束并返回 trapContext， 这是哪个任务的 trapContext? 就是它自己
    # 后面还是调用 __restore , 此时trap_handler 返回值保存在a0寄存器
    # 此时的 sp 指向trapContextPtr (内核栈上 trap控制流中调用的函数,函数调用的__switch的)
    # 都在app内核栈上,中断处理结束 返回__restore,返回用户态

 
#  __alltraps:
#     # 在这一行之前 sp 指向用户栈， sscratch 指向内核栈（原因稍后说明），现在 sp 指向内核栈， sscratch 指向用户栈。
#     csrrw sp, sscratch, sp
#     # now sp->kernel stack, sscratch->user stack
#     # allocate a TrapContext on kernel stack
#     addi sp, sp, -34*8
#     # save general-purpose registers
#     sd x1, 1*8(sp)
#     # skip sp(x2), we will save it later
#     sd x3, 3*8(sp)
#     # skip tp(x4), application does not use it
#     # save x5~x31
#     .set n, 5
#     .rept 27
#         SAVE_GP %n
#         .set n, n+1
#     .endr
#     # we can use t0/t1/t2 freely, because they were saved on kernel stack
#     csrr t0, sstatus
#     csrr t1, sepc
#     sd t0, 32*8(sp)
#     sd t1, 33*8(sp)
#     # read user stack from sscratch and save it on the kernel stack
#     csrr t2, sscratch
#     sd t2, 2*8(sp)
#     # set input argument of trap_handler(cx: &mut TrapContext)
#     mv a0, sp
#     call trap_handler

# pub struct TrapContext {
#     /// General-Purpose Register x0-31
#     pub x: [usize; 32],
#     /// Supervisor Status Register
#     pub sstatus: Sstatus,
#     /// Supervisor Exception Program Counter
#     pub sepc: usize,
# }
# pub struct TaskContext {
#     /// Ret position after task switching
#     ra: usize,
#     /// Task' Trap Context Pointer/Stack pointer
#     sp: usize,
#     /// s0-11 register, callee saved
#     s: [usize; 12],
# }

# __restore:
#     # 上面 __switch 最后从 TaskContext数据结构中 找到 next task's trapContextPtr/trap_sp 赋值给了sp
#     # 现在这个 sp 就是next任务的 trapContextPtr/trap_sp ,在kernelstack上
#     # 接下来从 trapContext 中恢复 陷入 之前的寄存器状态
#     # now sp->kernel stack(after allocated), sscratch->user stack 
#     # sscratch 为什么 指向user stack呢?? 这里不是正确的user stack sp地址,下面要进行恢复
    # # restore sstatus/sepc
    # ld t0, 32*8(sp) # 从内核栈TrapContext里读取 sstatus 保存到t0
    # ld t1, 33*8(sp) # 从内核栈TrapContext里读取 sepc 保存到t1
    # ld t2, 2*8(sp)  # 从内核栈TrapContext里读取 sp, 这是trap到内核的时候保存的用户栈sp
    # csrw sstatus, t0  # 恢复 sstatus
    # csrw sepc, t1     # 恢复 sepc
    # csrw sscratch, t2 # 恢复 sscratch 其应该指向 user stack
#     # restore general-purpuse registers except sp/tp
#     ld x1, 1*8(sp)
#     #  这里为什么不恢复x2/sp呢, 因为现在sp指向内核栈栈顶,我们还要用它的地址 读取并恢复 其他寄存器的值
#     ld x3, 3*8(sp)
#     .set n, 5
#     .rept 27
#         LOAD_GP %n
#         .set n, n+1
#     .endr
#     # release TrapContext on kernel stack
#     addi sp, sp, 34*8  # sp = sp + 34*8
#     # 这个地方对sp其做了加法,释放了TrapContext栈帧, 现在sp指向内核栈没有分配trapContext栈帧的位置
#     # now sp->kernel stack, sscratch->user stack
#     # 准备交换 ssratch(指向user stack)和 sp(nextTask的trapContextPtr被释放栈帧后的地址)
#     csrrw sp, sscratch, sp 
#     # 现在 sp是指向 用户态 user stack, sscratch指向 该app 内核栈(前一次分配的trapContext栈帧已经被释放,下次陷入时重新分配)
#     sret
#     # 返回用户态
```