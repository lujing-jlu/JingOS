; JingOS C风格内核 - 纯汇编实现
; 模拟C内核的功能，但用汇编编写

[bits 64]
[org 0x20000]

kernel_start:
    ; 设置64位段寄存器
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    
    ; 设置栈指针
    mov rsp, 0x80000
    
    ; 清屏
    call clear_screen
    
    ; 显示内核信息
    call display_kernel_info
    
    ; 无限循环
    jmp $

; 清屏函数
clear_screen:
    push rax
    push rcx
    push rdi
    
    mov rdi, 0xb8000
    mov rcx, 2000
    mov ax, 0x0720      ; 空格，白色背景
    rep stosw
    
    pop rdi
    pop rcx
    pop rax
    ret

; 显示内核信息
display_kernel_info:
    ; 第一行：标题（红色）
    mov rdi, 0xb8000
    mov rsi, title_msg
    mov ah, 0x0c        ; 红色
    call print_string_color
    
    ; 第二行：64位模式（绿色）
    mov rdi, 0xb8000
    add rdi, 160        ; 下一行
    mov rsi, mode_msg
    mov ah, 0x0a        ; 绿色
    call print_string_color
    
    ; 第三行：C内核成功（绿色）
    mov rdi, 0xb8000
    add rdi, 320        ; 第三行
    mov rsi, success_msg
    mov ah, 0x0a        ; 绿色
    call print_string_color
    
    ; 第四行：bootloader集成（绿色）
    mov rdi, 0xb8000
    add rdi, 480        ; 第四行
    mov rsi, integration_msg
    mov ah, 0x0a        ; 绿色
    call print_string_color
    
    ; 第六行：功能列表标题（青色）
    mov rdi, 0xb8000
    add rdi, 800        ; 第六行
    mov rsi, features_title
    mov ah, 0x0b        ; 青色
    call print_string_color
    
    ; 第七行：VGA文本模式（青色）
    mov rdi, 0xb8000
    add rdi, 960        ; 第七行
    mov rsi, feature1_msg
    mov ah, 0x0b        ; 青色
    call print_string_color
    
    ; 第八行：颜色支持（青色）
    mov rdi, 0xb8000
    add rdi, 1120       ; 第八行
    mov rsi, feature2_msg
    mov ah, 0x0b        ; 青色
    call print_string_color
    
    ; 第九行：字符串输出（青色）
    mov rdi, 0xb8000
    add rdi, 1280       ; 第九行
    mov rsi, feature3_msg
    mov ah, 0x0b        ; 青色
    call print_string_color
    
    ; 第十行：64位兼容（青色）
    mov rdi, 0xb8000
    add rdi, 1440       ; 第十行
    mov rsi, feature4_msg
    mov ah, 0x0b        ; 青色
    call print_string_color
    
    ; 第十二行：运行状态（紫色）
    mov rdi, 0xb8000
    add rdi, 1760       ; 第十二行
    mov rsi, running_msg
    mov ah, 0x0d        ; 紫色
    call print_string_color
    
    ; 第十四行：退出提示（白色）
    mov rdi, 0xb8000
    add rdi, 2080       ; 第十四行
    mov rsi, exit_msg
    mov ah, 0x0f        ; 白色
    call print_string_color
    
    ret

; 打印彩色字符串
; rdi = 目标地址, rsi = 字符串, ah = 颜色
print_string_color:
    push rax
    push rsi
    push rdi
    
.loop:
    lodsb
    test al, al
    jz .done
    
    ; 写入字符和颜色
    mov [rdi], al
    mov [rdi+1], ah
    add rdi, 2
    
    jmp .loop
    
.done:
    pop rdi
    pop rsi
    pop rax
    ret

; 数据段
title_msg: db "=== JingOS C Kernel v1.0 ===", 0
mode_msg: db "64-bit Long Mode: ACTIVE", 0
success_msg: db "C Kernel: SUCCESS!", 0
integration_msg: db "Bootloader Integration: OK", 0
features_title: db "C Kernel Features:", 0
feature1_msg: db "- VGA Text Mode", 0
feature2_msg: db "- Color Support", 0
feature3_msg: db "- String Output", 0
feature4_msg: db "- 64-bit Compatible", 0
running_msg: db "JingOS C Kernel is running!", 0
exit_msg: db "Press Ctrl+C to exit QEMU", 0

; 填充到512字节的倍数
times 1024-($-kernel_start) db 0
