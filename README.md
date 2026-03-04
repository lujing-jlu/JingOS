# jingOS（Rust OS 起步骨架）

这个仓库已经初始化为一个可启动的最小 Rust 操作系统骨架，包含：

- `kernel/`：`no_std` + `no_main` 内核（x86_64）
- `bootloader`：构建 BIOS/UEFI 启动镜像
- `src/main.rs`：主机侧启动器（调用 QEMU 运行镜像）

## 1. 环境准备

建议先安装（macOS）：

```bash
brew install qemu
```

Rust 工具链由仓库内 `rust-toolchain.toml` 固定为 nightly，并自动包含：

- `llvm-tools`
- `x86_64-unknown-none` target

## 2. 运行方式

BIOS 启动：

```bash
cargo run -- bios
```

UEFI 启动：

```bash
cargo run -- uefi
```

显示 QEMU 图形窗口（可看到 FrameBuffer `println!` 文本）：

```bash
cargo run -- bios --show
```

如果运行成功，QEMU 串口会输出内核日志并进入监视器输入窗口（约 8 秒），可输入命令：

- `help` / `status` / `ticks` / `uptime` / `irq` / `mem` / `maps [n|all]` / `heap` / `vm [addr]` / `vmmap [addr]`（分页级别详情） / `fault [addr]` / `echo` / `history` / `clear` / `exit`

监视器在有键盘输入时会自动续时；按 `Esc` 或输入 `exit` 可提前退出。

输入增强：

- `↑ / ↓`：命令历史浏览（最多 16 条）
- `Tab`：命令自动补全（命令前缀）
- `← / →`：行内移动光标
- `Ctrl + ← / Ctrl + →`：按单词跳转
- `Home / End`：跳转到行首/行尾
- `Backspace / Delete`：删除光标前/后字符（支持行内编辑插入）
- `Ctrl + Backspace / Ctrl + Delete`：删除左侧/右侧单词
- `Insert`：在插入模式与覆写模式之间切换

## 3. 当前目录结构

```text
.
├── Cargo.toml
├── build.rs
├── kernel
│   ├── Cargo.toml
│   └── src/main.rs
└── src/main.rs
```

## 4. 当前能力

- 串口日志输出（`uart_16550`）
- FrameBuffer 文本输出（内核 `print!` / `println!`）
- 中断系统已接入（`IDT + PIC + PIT`，可观测时钟 tick）
- 已接入键盘 IRQ1 读取 + scancode 解码（字符、方向键、Esc、F1-F12、Insert/Delete/Home/End/Page）
- 已加入内存地图统计与 4KiB 页帧分配器起步骨架（基于 BootInfo memory map）
- 已启用物理内存映射偏移（`physical_memory_offset`），为后续页表操作准备
- 已完成测试页映射探针（`OffsetPageTable` + `map_to`）
- 已完成内核堆初始化（`linked_list_allocator`）与 `Box/Vec` 分配探针
- 已接入页错误异常处理（Page Fault handler）
- 已提供最小监视器命令行（串口+FrameBuffer 同步回显）
- BIOS/UEFI 双启动路径

## 5. 下一步建议（按顺序）

1. **输入子系统增强**：补齐更多组合键和更完整的 keymap（含国际化布局）。
2. **内存管理深化**：完善虚拟内存布局、高半内核映射策略与页错误处理。
3. **任务调度**：先做简单协作式任务，再演进到抢占式。
4. **用户态雏形**：定义 syscall ABI，跑一个最小用户程序。
