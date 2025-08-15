; 简化的JingOS Bootloader
[bits 16]
[org 0x7c00]

start:
    ; 清除中断
    cli
    
    ; 设置段寄存器
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    
    ; 显示启动消息
    mov si, boot_msg
    call print_string
    
    ; 简单的无限循环，先确保基础工作
    jmp $

; 打印字符串函数
print_string:
    lodsb
    test al, al
    jz print_done
    mov ah, 0x0e
    int 0x10
    jmp print_string
print_done:
    ret

; 数据
boot_msg:
    db "JingOS Simple Bootloader Working!", 13, 10, 0

; 填充到512字节并添加引导签名
times 510-($-start) db 0
dw 0xaa55
