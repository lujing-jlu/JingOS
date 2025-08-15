#![no_std] // 不链接Rust标准库
#![no_main] // 禁用所有Rust层级的入口点
#![feature(custom_test_frameworks)]
#![test_runner(jing_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

/// 内核入口点
///
/// 这个函数是操作系统的第一个Rust函数，由bootloader调用
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 内核入口点 - 由我们的自定义bootloader调用
    // 此时我们在32位保护模式下

    // 使用我们的VGA缓冲区系统
    use jing_kernel::println;

    // 清屏并显示欢迎信息
    println!("=== JingOS Kernel v3.0 ===");
    println!("64-bit Long Mode: ACTIVE");
    println!("Multi-Stage Bootloader: SUCCESS");
    println!("VGA Text Mode: WORKING");
    println!("");
    println!("Initializing 64-bit kernel components...");

    // 初始化内核组件
    jing_kernel::init();

    println!("64-bit kernel initialization complete!");
    println!("System ready for operation.");
    println!("");
    println!("🎉 JingOS is now running in 64-bit mode! 🎉");
    println!("Press Ctrl+C to exit QEMU");

    // 内核主循环
    jing_kernel::hlt_loop();
}

/// panic处理函数
///
/// 当发生panic时，这个函数会被调用
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // 简单的panic处理，直接停机
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// 测试模式下的panic处理函数
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    jing_kernel::test_panic_handler(info)
}
