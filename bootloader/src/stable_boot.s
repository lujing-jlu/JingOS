; JingOS 稳定测试 Bootloader
; 只显示消息然后停止，不做复杂操作

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
    
    ; 等待一下让BIOS消息稳定
    call wait_bios
    
    ; 清屏
    call clear_screen
    
    ; 显示我们的消息
    mov si, msg1
    call print_string
    
    mov si, msg2
    call print_string
    
    mov si, msg3
    call print_string
    
    ; 显示一个持续的状态指示
    mov si, status_msg
    call print_string
    
    ; 无限循环，不做任何其他操作
    jmp $

; 等待BIOS稳定
wait_bios:
    mov cx, 0xffff
wait_loop1:
    mov dx, 0xffff
wait_loop2:
    nop
    dec dx
    jnz wait_loop2
    dec cx
    jnz wait_loop1
    ret

; 清屏函数
clear_screen:
    mov ax, 0x0003  ; 设置80x25文本模式
    int 0x10
    ret

; 打印字符串函数
print_string:
    lodsb
    test al, al
    jz print_done
    mov ah, 0x0e
    mov bh, 0
    mov bl, 0x0f  ; 白色
    int 0x10
    jmp print_string
print_done:
    ret

; 数据段
msg1:
    db "JingOS Bootloader - Stability Test", 13, 10, 0

msg2:
    db "This message should stay stable", 13, 10, 0

msg3:
    db "No complex operations, just display", 13, 10, 0

status_msg:
    db "Status: STABLE - System Halted", 13, 10, 0

; 填充到512字节并添加引导签名
times 510-($-start) db 0
dw 0xaa55
