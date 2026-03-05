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

- `help` / `status` / `ticks` / `uptime` / `irq` / `mem` / `maps [n|all]` / `heap` / `vm [addr]` / `vmmap [addr]`（分页级别详情） / `fault [addr]` / `syscall <n> [a0] [a1] [a2]` / `sysabi` / `tasks` / `tasknew [userdemo|monitor|fastsyscall|fastsyscall_fail]` / `taskstep` / `userdemo` / `usermode` / `usermode_syscall` / `usermode_syscall_fail` / `echo` / `history` / `clear` / `exit`

监视器在有键盘输入时会自动续时；按 `Esc` 或输入 `exit` 可提前退出。

也支持串口输入（可用于自动化脚本），例如：

```bash
printf 'help\rstatus\ruserdemo\rexit\r' | cargo run -- bios --serial-only
```

也可用内置脚本发送参数（默认延迟 6000ms，建议与内核输出的 `[[JINGOS_MONITOR_READY]]` 标记配合观察）：

```bash
cargo run -- bios --serial-script ./scripts/monitor-demo.txt
```

验证调度器雏形命令：

```bash
cargo run -- bios --serial-script ./scripts/scheduler-demo.txt
```

验证调度器驱动 fast syscall 成功路径：

```bash
cargo run -- bios --serial-script ./scripts/scheduler-fast-syscall-demo.txt
```

验证调度器驱动 fast syscall 错误路径：

```bash
cargo run -- bios --serial-script ./scripts/scheduler-fast-syscall-fail-demo.txt
```

验证调度器在同次启动内连续执行 fast syscall 成功+错误路径：

```bash
cargo run -- bios --serial-script ./scripts/scheduler-fast-syscall-sequential-demo.txt
```

验证 usermode 返回监视器：

```bash
cargo run -- bios --serial-script ./scripts/usermode-return-demo.txt
```

验证 ring3 `syscall/sysret` 预研路径（会回显 fast-syscall 报告：`rax` 返回值与 `r10` 状态码）：

```bash
cargo run -- bios --serial-script ./scripts/usermode-syscall-demo.txt
```

验证 ring3 `syscall/sysret` 错误路径（unknown/overflow 状态码）：

```bash
cargo run -- bios --serial-script ./scripts/usermode-syscall-fail-demo.txt
```

验证同次启动内连续执行 fast syscall 成功+错误路径：

```bash
cargo run -- bios --serial-script ./scripts/usermode-syscall-sequential-demo.txt
```

脚本支持按行指令：普通行会作为命令发送，`sleep <ms>`/`sleep_ms <ms>`/`sleep_s <sec>` 可做分阶段等待；支持整行 `#` 注释，也支持命令后的内联注释（形如 `... # comment`）。

脚本模式会优先等待 `[[JINGOS_MONITOR_READY]]` 标记后再发送命令；若等待窗口超时则回退为直接发送。

如需调整等待窗口基准值，可追加 `--serial-delay-ms 9000`。

一键自动回归：

```bash
# full（默认，包含 sequential 场景）
./scripts/regression-suite.sh

# fast（更快，适合本地快速检查）
./scripts/regression-suite.sh fast

# full（显式指定）
./scripts/regression-suite.sh full
```

可通过环境变量覆盖串口脚本等待窗口基准值（默认 12000ms）与回归模式：

```bash
JINGOS_SERIAL_DELAY_MS=15000 ./scripts/regression-suite.sh fast
JINGOS_REGRESSION_MODE=fast ./scripts/regression-suite.sh
```

CI 自动回归（GitHub Actions）：

- 工作流文件：`.github/workflows/regression-suite.yml`
- `Serial Regression (Fast)`：`push` / `pull_request` / `workflow_dispatch` 可触发
- `Serial Regression (Full)`：仅 `workflow_dispatch` 或 `push` 到 `master/main` 时执行
- `push`/`pull_request` 仅在关键路径变更时触发（`kernel/**`、`scripts/**`、`src/**`、`README.md` 等）
- 同一分支/PR 触发新任务时会自动取消旧的进行中任务（并发去重）
- 已启用 Cargo/target 缓存（`actions/cache`）以加速重复构建
- 失败时自动上传 `/tmp/jingos-*.log` 工件

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
- 已接入 int 0x80 syscall 分发层（寄存器 ABI：`rax/rdi/rsi/rdx` 入参，`rax/rcx` 出参）与 userdemo 用户态雏形命令
- 已加入任务调度器雏形（task table + `tasks/tasknew/taskstep` 命令，`user_demo` 任务可在 `taskstep` 时真实执行）
- 已接入 ring3 跳转探针（`usermode` / `usermode_syscall` / `usermode_syscall_fail` 命令，经 `int 0x81` 返回监视器；`usermode_syscall` 校验成功路径，`usermode_syscall_fail` 校验 unknown/overflow 错误路径的 `rax(value)` + `r10(status)`）
- 已加入 `syscall/sysret` 预研骨架（CPU SYSCALL 能力探测 + STAR/LSTAR/SFMASK 预写入 + selector/GS 基址规划，SCE 实验性启用）
- BIOS/UEFI 双启动路径

## 5. 下一步建议（按顺序）

1. **输入子系统增强**：补齐更多组合键和更完整的 keymap（含国际化布局）。
2. **内存管理深化**：完善虚拟内存布局、高半内核映射策略与页错误处理。
3. **任务调度**：先做简单协作式任务，再演进到抢占式。
4. **用户态雏形**：定义 syscall ABI，跑一个最小用户程序。
