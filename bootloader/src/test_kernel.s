; JingOS 测试内核 - 验证64位跳转
; 这是一个简单的测试内核，用于验证bootloader到内核的跳转

[bits 64]
[org 0x10000]

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
    
    ; 显示内核启动消息
    mov rsi, kernel_msg
    call print_kernel_string
    
    ; 显示成功消息
    mov rsi, success_msg
    call print_kernel_string
    
    ; 显示系统信息
    mov rsi, system_info_msg
    call print_kernel_string
    
    ; 无限循环
    jmp $

; 64位内核打印函数
print_kernel_string:
    push rax
    push rdi
    
    ; 计算屏幕位置（从第20行开始）
    mov rdi, 0xb8000
    add rdi, 3200       ; 20行 * 160字节
    
.loop:
    lodsb
    test al, al
    jz .done
    
    ; 写入字符（黄色高亮）
    mov ah, 0x0e
    mov [rdi], ax
    add rdi, 2
    
    jmp .loop
    
.done:
    pop rdi
    pop rax
    ret

; 数据段
kernel_msg: db "*** JingOS Test Kernel Started ***", 0
success_msg: db "64-bit Kernel Jump: SUCCESS!", 0
system_info_msg: db "Test Kernel Running in 64-bit Mode", 0

; 填充到512字节
times 512-($-kernel_start) db 0
