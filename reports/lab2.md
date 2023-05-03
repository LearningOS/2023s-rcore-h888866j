# lab2 reprot
 `prot` 参数的语义，它与内核定义的 `MapPermission` 有明显不同！没看到这句提示，调试半天。

 左移运算优先级低于减法运算，之前没注意，导致 prot 验证出现失误，耽误不少时间.

`sys_mmap` 就是判断pte是否存在是否有效，然后使用 memset.insert_frames_area 插入。其中 translate 提供的转换功能，调用的是 find_pte，它在找到末级节点之后直接返回pte并不检查有效性，所以我们需要自己判断。

`sys_munmmap` 需要在 mem_set.areas 里面找到对应的start 和 len map的area，移除以释放物理页帧， 维护页表，清空pte。



## 荣誉准则
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

        《你交流的对象说明》

2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

        《你参考的资料说明》

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。


# 问题：

## 还没体会到 手动分配和回收物理内存 的麻烦，就直接用RAII思想给优化了
http://rcore-os.cn/rCore-Tutorial-Book-v3/chapter4/4sv39-implementation-2.html

FrameTracker 那里

可以发现， `frame_alloc` 的返回值类型并不是 `FrameAllocator` 要求的物理页号 `PhysPageNum` ，而是将其进一步包装为一个 `FrameTracker` 。这里借用了 RAII 的思想，将一个物理页帧的生命周期绑定到一个 `FrameTracker` 变量上，当一个 `FrameTracker` 被创建的时候，我们需要从 `FRAME_ALLOCATOR` 中分配一个物理页帧：

### 问题2
[页表基本数据结构与访问接口](http://rcore-os.cn/rCore-Tutorial-Book-v3/chapter4/4sv39-implementation-2.html#id6) 这里 `frames` 字段

此外，向量 `frames` 以 `FrameTracker` 的形式保存了页表所有的节点（包括根节点）所在的物理页帧。这与物理页帧管理模块的测试程序是一个思路，即将这些 `FrameTracker` 的生命周期进一步绑定到 `PageTable` 下面。当 `PageTable` 生命周期结束后，向量 `frames` 里面的那些 `FrameTracker` 也会被回收，也就意味着存放多级页表节点的那些物理页帧被回收了。


当我们通过 new 方法新建一个 PageTable 的时候，**它只需有一个根节点。为此我们需要分配一个物理页帧 `FrameTracker`** 并挂在向量 frames 下，然后更新根节点的物理页号 root_ppn 。


为什么要分配一个 物理页帧 ， 分配了就是它在用吗？？？

页表节点使用的物理页帧

### FrameTracker
```rust
pub struct FrameTracker {
    pub ppn: PhysPageNum,
}
impl FrameTracker {
    pub fn new(ppn: PhysPageNum) -> Self {
        // page cleaning
        let bytes_array = ppn.get_bytes_array();
        for i in bytes_array {
            *i = 0;
        }
        Self { ppn }
    }
}
impl Drop for FrameTracker {
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}
``` 
FrameTracker 是传入 ppn 来创建的，ppn.get_bytes_array(); 又对其进行清空。最后才返回的 FT

#### PageTable
```rust
pub struct PageTable {
    /// 一个 `PageTable` 要保存它根节点的物理页号 root_ppn 作为页表唯一的区分标志
    /// 这个 `root_ppn` ，可能会写入 `satp` 的。代表的 就是一个页表
    root_ppn: PhysPageNum,
    /// 保存了 页表 节点 所在的物理页帧 PPN 创建的 `FrameTracker，`
    /// FrameTracker implement了 `drop`，drop 调用了 dealloc(ppn)
    /// 所以 PageTable 丢弃的时候，pt.frames，ft 都drop了，
    /// 占用的内存空间 也释放掉
    /// 
    ///  向量 `frames` 以 `FrameTracker` 的形式保存了页表所有的节点（包括根节点）所在的物理页帧。
    frames: Vec<FrameTracker>,
}

impl PageTable {
    /// 当我们通过 `new` 方法新建一个 `PageTable` 的时候，
    /// 它只需有一个根节点。为此我们需要分配一个物理页帧 `FrameTracker` 
    /// 并挂在向量 `frames` 下，然后更新根节点的物理页号 `root_ppn` 。
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }
}

```

`PageTable` 被销毁回收的时候，自然能`drop`掉`frames`字段里的 `FrameTracker`，`PageTable` 各级节点 占用的 物理页帧 也能被回收（FrameTracker `Drop` trait 调用了 `frame_dealloc`/ FRAME_ALLOCATER.dealloc(ppn) ）。

那我们的数据结构 `PageTable` 跟 页表 又有啥关系呢？

我的理解： pagetable保存了root_ppn和根页表所在物理页帧的frameTracker。pageTable 离开作用域的时候，也触发 `FrameTracker`的`dealloc`，调用 StackFrameAllocator 的`drop` 释放物理页帧进行回收，放到`StackFrameAllocator.recycled`里面

## 启用SV39页表机制：

`satp` 寄存器保存了 页表根节点的 `ppn` ，
先创建一个 `PageTable` ，用给 `PageTable` 分配的 frame的 `ppn` ，创建 token ， 给 `satp`？？？

```rust
// os/src/mm/page_table.rs

pub fn token(&self) -> usize {
    8usize << 60 | self.root_ppn.0
}

// os/src/mm/memory_set.rs

impl MemorySet {
    pub fn activate(&self) {
        let satp = self.page_table.token();
        unsafe {
            satp::write(satp);
            core::arch::asm!("sfence.vma");
        }
    }
}

```
`PageTable::token` 会按照 `satp` CSR 格式要求 构造一个无符号 64 位无符号整数，使得其 分页模式为 SV39 ，且将当前多级页表的根节点所在的物理页号填充进去。在 `activate` 中，我们将这个值写入当前 CPU 的 `satp` CSR ，从这一刻开始 SV39 分页模式就被启用了，而且 MMU 会使用内核地址空间的多级页表进行地址转换。( 因为 satp 的 Mode 位不再是 0 了)

>默认情况下 MMU 未被使能，此时无论 CPU 处于哪个特权级，访存的地址都将直接被视作物理地址。 可以通过修改 S 特权级的 `satp` CSR 来启用分页模式，此后 S 和 U 特权级的访存地址会被视为虚拟地址，经过 MMU 的地址转换获得对应物理地址，再通过它来访问物理内存。
../_images/satp.png

>上图是 RV64 架构下 `satp` 的字段分布。当 MODE 设置为 `0` 的时候，所有访存都被视为物理地址；而设置为 `8` 时，SV39 分页机制被启用，所有 S/U 特权级的访存被视为一个 39 位的虚拟地址，MMU 会将其转换成 56 位的物理地址；如果转换失败，则会触发异常。

我们必须注意切换 `satp` CSR 是否是一个 平滑 的过渡：其含义是指，切换 `satp` 的指令及其下一条指令这两条相邻的指令的 虚拟地址是相邻的（由于切换 `satp` 的指令并不是一条跳转指令， `pc` 只是简单的自增当前指令的字长!!!!!!）， 而它们所在的物理地址一般情况下也是相邻的，但是它们所经过的地址转换流程却是不同的——切换 `satp` 导致 MMU 查的多级页表 是不同的。这就要求前后两个地址空间在切换 `satp` 的指令 附近 的映射满足某种意义上的连续性。

幸运的是，我们做到了这一点。这条写入 `satp` 的指令及其下一条指令都在**内核内存布局**的代码段中，在切换之后是一个恒等映射， 而在切换之前是视为物理地址直接取指，也可以将其看成一个恒等映射。这完全符合我们的期待：即使切换了地址空间，指令仍应该 能够被连续的执行。


## 页表查询过程

1. 根据 `root_ppn/satp.ppn` 查询一级页表中 `vpn1` 对应的 `PTE_L1`，

2. 得到 `PTE_L1 = satp.ppn.PTEs[vpn1]`， (`ppn`转成`PA`（这个PA就是一级页表的物理地址）再以512长度的 PTE slice方式去访问 得到切片 PTEs,再索引切片)

3. 如果 `PTE_L1`（一级页表内512个PTE中的一个） 不合法（v位为0）,就创建 它，创建的是合法 `PTE`, 包含分配物理页帧用来给下级节点存放pte数据，拿到分配来的物理页帧的 ppn，用ppn和flags创建在一级页表中的 PTE（你怎么把数据写进一级页表对vpn1对应的64bits的位置的呢？？？）， PTE bits字段包含有ppn

`*pte = PageTableEntry::new(frame.ppn, PTEFlags::V);`


3. 然后根据`PTE_L1`找到 二级页表 `PTE_L1.ppn` 即 `satp.ppn.PTEs[vpn1].ppn` , 并查询二级页表中 `vpn2`索引 对应的 `PTE_L2`，

4. 得到 `PTE_L2 = satp.ppn.PTEs[vpn1].ppn.PTEs[vpn2]`

5. 然后根据 `PTE_L2`的`ppn`，即 `satp.ppn.PTEs[vpn1].ppn.PTEs[vpn2].ppn` 查询三级页表 `vpn3` 对应的 `PTE_L3` (`PTE_L2.ppn.PTEs_level3[vpn3]`)

6. 得到 `PTE_L3 = satp.ppn.PTEs[vpn1].ppn.PTE[vpn2].ppn.leafPTEs[vpn3]`

7. 虚拟地址 对应的 物理地址就是 `PA = PTE_L3.ppn + VA.offset`

## 勘误：这就需要我们提前扩充多级页表维护的映射， 
这一点的位置应该往下放一放

放到：

在分页机制开启前，这样做自然成立；而开启之后，

## 通过 trapContext ppn，得能获取到 ppn对应pa指向的trapContext 的可变引用
用于读取和修改 trapContext 里的数据
```rust
impl TaskControlBlock {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
}
```
此处需要说明的是，返回 `'static` 的可变引用和之前一样可以看成一个绕过 `unsafe` 的裸指针；而 `PhysPageNum::get_mut` 是一个泛型函数，由于我们已经声明了总体返回 `TrapContext` 的可变引用，则Rust编译器会给 `get_mut` 泛型函数针对具体类型 `TrapContext` 的情况生成一个特定版本的 `get_mut` 函数实现。在 `get_trap_cx` 函数中则会静态调用``get_mut`` 泛型函数的特定版本实现。

### trap.S 注释

```shell
.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm
    .section .text.trampoline
    .globl __alltraps
    .globl __restore
    .align 2
__alltraps:
    # now sscratch -> *TrapContext in user space, sp -> user stack in app user space
    # 当应用 Trap 进入内核的时候，硬件会设置一些 CSR 并在 S 特权级下跳转到 __alltraps 保存 Trap 上下文。
    # 此时 sp 寄存器仍指向用户栈，
    # 但 sscratch 则被设置为指向应用地址空间中存放 Trap 上下文的位置（实际在次高页面）
    # 随后，就像之前一样，我们 csrrw 交换 sp 和 sscratch ，
    # 并基于指向 Trap 上下文位置的 sp 开始保存通用寄存器和一些 CSR 
    csrrw sp, sscratch, sp
    # now sp->*TrapContext in user space, sscratch->user stack in app user space
    # save other general purpose registers
    sd x1, 1*8(sp)
    # skip sp(x2), we will save it later, ~这个需要存的x2在sscratch里面~
    sd x3, 3*8(sp)
    # skip tp(x4), application does not use it
    # save x5~x31
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr
    
# pub struct TrapContext {
#     /// General-Purpose Register x0-31
#     pub x: [usize; 32],
#     /// Supervisor Status Register
#     pub sstatus: Sstatus,
#     /// Supervisor Exception Program Counter
#     pub sepc: usize,
#     /// Token of kernel address space
#     pub kernel_satp: usize, 
#     /// Kernel stack pointer of the current application 应用内核栈顶的地址
#     pub kernel_sp: usize,
#     /// Virtual address of trap handler entry point in kernel
#     pub trap_handler: usize,
# }
    # we can use t0/t1/t2 freely, because they have been saved in TrapContext
    csrr t0, sstatus # 把 sstatus 里的 值 存到 t0 里面
    csrr t1, sepc    # 把 sepc 存到 t1
    sd t0, 32*8(sp)  # 把 t0,也就是 当前sstatus里的值 保存到 TrapContext对应位置 sstatus字段
    sd t1, 33*8(sp)
    # read user stack from sscratch and save it in TrapContext
    csrr t2, sscratch    # 把存储在 sscratch 里的 sp 读到 t2
    sd t2, 2*8(sp)       # 保存用户应用地址空间用户栈sp
    # load kernel_satp into t0
    # 从 TrapContext 中读取 kernel_satp 字段到t0寄存器
    ld t0, 34*8(sp)
    # load trap_handler into t1
    ld t1, 36*8(sp)
    # move to kernel_sp， 应用内核栈顶的地址
    ld sp, 35*8(sp)
    # switch to kernel space
    # 下一条命令 将 kernel_satp 写入 satp 寄存器，
    csrw satp, t0
    sfence.vma
    # jump to trap_handler
    # 跳转到 trap_handler, 虚地址，借用mmu和satp转换成 物理地址
    jr t1

__restore:
    # 它有两个参数：第一个是 Trap 上下文在应用地址空间中的位置，
    # 这个对于所有的应用来说都是相同的，在 a0 寄存器中传递；
    # 第二个则是即将回到的应用的地址空间的 token ，在 a1 寄存器中传递。
    # a0: *TrapContext in user space(Constant); a1: user space token， satp值
    # switch to user space
    # sp 此时指向 内核栈？ 应用内核栈顶的地址
    # sscratch 此时指向  sscratch->user stack in app user space
    csrw satp, a1
    sfence.vma
    csrw sscratch, a0 # a0/TrapContext位置 赋值给 sscratch，这样 __alltraps 中才能基于它将 Trap 上下文保存到正确的位置；
    mv sp, a0  # 把 a0 赋值给 sp
    # now sp points to TrapContext in user space, start restoring based on it
    # restore sstatus/sepc
    ld t0, 32*8(sp) # 从 TrapContext sstatus 字段加载到 t0
    ld t1, 33*8(sp) # 从 TrapContext sepc 加载到 t1
    csrw sstatus, t0
    csrw sepc, t1
    # restore general purpose registers except x0/sp/tp
    ld x1, 1*8(sp)  # ra
    ld x3, 3*8(sp)  # 
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # back to user stack
    ld sp, 2*8(sp)     # 从 TrapContext 中读取 x1、sp值保存到 sp 寄存器，sp就指向用户空间用户栈栈顶了
    sret
```