## ch6 实验报告
- Stat 信息应该放在 OSInode 结构中，写入磁盘以持久化存储这些数据
- 系统调用中 根据 fd 拿到 fd_table 中打开的文件someOSInode，但是不是需要读写文件 而是需要读写文件元信息，read是读 data block中的数据，这不是我们需要的。
根据 trait object (someOSInode actually) 指针 获取/解释成 OSInode 类型 去调用他的方法 完成数据获取
- Inode 某些方法 全程持有 fs 锁，如果在一个方法中调用另一个方法，注意那个方法是不是也要持有锁

## ch7 问答

1. 举出使用 pipe 的一个实际应用的例子。

`ps -ef | grep process_name`, `cat src/task/task.rs | wc -l`

2. 如果需要在多个进程间互相通信，则需要为每一对进程建立一个管道，非常繁琐，请设计一个更易用的多进程通信机制。.

每个进程都往消息池 发送 （接收者，消息内容） 这样的消息，每个进程都循环从这个消息池中获取某一个所有消息拷贝，再对其中消息 甄别，只处理给它自己的消息。

## ch6 问题+过程
看完第六章，准备写作业，`linkat`可以实现，`unlinkat` 要 减少 physical link 数量，又先去实现 `sys_fstat` 系统调用:

看了一圈， 不确定该把 `Stat` 结构放到哪里，`OSInode` / `Inode` / `Dirent` / `DiskInode`. 

读第一遍的时候 只知道 `DiskInode`是存在磁盘上的其他几个在哪没注意，文件信息还需需要记录在磁盘上的，要不OS关机重启 这些信息就丢了 。

在实现 `sys_fstat` 的时候，发现 `fd_table` 里存储的应该是 `DiskInode`，但是从 fd_table 中根据 fd 用下标取出 OSInode 的时候，不让我调用 SomeOSInode 结构中定义的方法。定义`fd_table`字段的时候类型写的是 `Vec<Option<Arc<dyn File + Send + Sync>>>`, 只让我用 File trait 中定义的方法，难受，这不是 trait object 吗？ 不是只有运行时才能确定具体类型吗？ 为什么编译期 检查就不给我过？ 难道 看第六章的时候 有哪些用法细节没有觉察到？ 又跑回去看一遍还是没发现，无奈又回去看一遍并给代码加注释。花了许多时间，最后回来做昨夜 还是卡在这个地方，收获就是确定给stat加到DiskInode里了。

再后来就想能不能 获取 someOSInode的指针，再转成 OSInode 调用其上的方法。思路可行就尝试了一下，但是遇到个奇怪的问题提示multi borrow, 排查发现时 OSInode.inner.exclusive_access() 这里，可怎么想都想不到哪里借出去过！！ ，不知怎么的又看到一个提示（记不清这个error和multi borrow哪个先出现了） 说让用 thin pointer，又去搜索这个怎么调用 trait object 本身的方法而不是trait中定义的方法。看了 trait object 的内存布局发现他是 2usize，又找到一个帖子说怎么获取类型本身的 ptr 指针 而不是 vtable 指针。用了 `core::ptr` `someOsInode_ptr as *const () as usize as OSInode`. 

看着那个 帖子 有 `as *const (dyn XXX + XXX)` , 我也学了下 `as *const (dyn File + Send + Sync)` 结果除了大问题，应该就是这时候出了 multi borrow问题。。。诊断半天，后来无意间 切换回 `core::ptr` 那个 才成功，至此才发现问题! 

后面解决优化（先前调试把 SomeOSinode从fd_table中take出来了）。再去实现fstat，linkat，unlinkat。 这时也遇到一个问题，后来发现是 fs 锁引起的。


## 荣誉准则
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

        《你交流的对象说明》

2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

        《你参考的资料说明》

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。