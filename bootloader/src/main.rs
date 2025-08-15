// JingOS 自定义 Bootloader 构建脚本
// 这个文件用于构建我们的自定义bootloader

use std::process::Command;
use std::path::Path;

fn main() {
    println!("Building JingOS Custom Bootloader...");
    
    // 编译汇编代码
    let output = Command::new("as")
        .args(&["--32", "src/boot.s", "-o", "boot.o"])
        .output()
        .expect("Failed to assemble bootloader");
    
    if !output.status.success() {
        panic!("Assembly failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // 链接生成二进制文件
    let output = Command::new("ld")
        .args(&[
            "-m", "elf_i386",
            "-Ttext", "0x7c00",
            "--oformat", "binary",
            "boot.o",
            "-o", "bootloader.bin"
        ])
        .output()
        .expect("Failed to link bootloader");
    
    if !output.status.success() {
        panic!("Linking failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    println!("Bootloader built successfully!");
}
