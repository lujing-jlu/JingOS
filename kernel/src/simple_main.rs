#![no_std] // 不链接Rust标准库
#![no_main] // 禁用所有Rust层级的入口点

use core::panic::PanicInfo;

/// 简单的VGA缓冲区写入
fn write_string(s: &str) {
    let vga_buffer = 0xb8000 as *mut u8;
    let bytes = s.as_bytes();
    
    for (i, &byte) in bytes.iter().enumerate() {
        unsafe {
            *vga_buffer.offset((i * 2) as isize) = byte;
            *vga_buffer.offset((i * 2 + 1) as isize) = 0x0c; // 红色
        }
    }
}

/// 内核入口点 - 专门为我们的64位bootloader设计
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 清屏
    let vga_buffer = 0xb8000 as *mut u16;
    for i in 0..2000 {
        unsafe {
            *vga_buffer.offset(i) = 0x0720; // 空格，白色背景
        }
    }
    
    // 显示简单的内核信息
    write_string("JingOS Simple Kernel - 64-bit Mode");
    
    // 在第二行显示成功信息
    let vga_buffer = 0xb8000 as *mut u8;
    let success_msg = b"Rust Kernel: SUCCESS!";
    for (i, &byte) in success_msg.iter().enumerate() {
        unsafe {
            *vga_buffer.offset((160 + i * 2) as isize) = byte;
            *vga_buffer.offset((160 + i * 2 + 1) as isize) = 0x0a; // 绿色
        }
    }
    
    // 无限循环
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// panic处理函数
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // 显示panic信息
    let vga_buffer = 0xb8000 as *mut u8;
    let panic_msg = b"KERNEL PANIC!";
    for (i, &byte) in panic_msg.iter().enumerate() {
        unsafe {
            *vga_buffer.offset((320 + i * 2) as isize) = byte;
            *vga_buffer.offset((320 + i * 2 + 1) as isize) = 0x04; // 红色
        }
    }
    
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
