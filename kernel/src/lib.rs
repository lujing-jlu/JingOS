#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

pub mod serial;
pub mod vga_buffer;

// 重新导出宏，使其在整个crate中可用
// 注意：宏会自动导出到crate根部

/// 初始化内核组件
pub fn init() {
    println!("  - Setting up interrupt handlers...");
    // TODO: 设置中断描述符表(IDT)

    println!("  - Initializing memory management...");
    // TODO: 设置堆分配器

    println!("  - Loading device drivers...");
    // TODO: 初始化设备驱动

    println!("  - Starting system services...");
    // TODO: 启动系统服务
}

/// CPU休眠循环
///
/// 使用hlt指令让CPU进入休眠状态，直到下一个中断
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

/// 测试框架
#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("运行 {} 个测试", tests.len());
    for test in tests {
        test();
    }
    exit_qemu(QemuExitCode::Success);
}

/// 测试panic处理函数
pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[失败]\n");
    serial_println!("错误: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    hlt_loop();
}

/// QEMU退出代码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

/// 退出QEMU
pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

/// 测试入口点
#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    init();
    test_main();
    hlt_loop();
}

/// 测试模式下的panic处理
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

/// 基础测试
#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
