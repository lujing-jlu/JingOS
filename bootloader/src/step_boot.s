; JingOS 逐步测试 Bootloader
; 先测试到32位模式

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
    
    ; 启用A20地址线
    call enable_a20
    
    ; 显示A20完成消息
    mov si, a20_msg
    call print_string
    
    ; 加载GDT
    lgdt [gdt_descriptor]
    
    ; 显示GDT加载消息
    mov si, gdt_msg
    call print_string
    
    ; 切换到保护模式
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    
    ; 跳转到32位代码
    jmp 0x08:protected_mode

; 打印字符串函数 (16位模式)
print_string:
    lodsb
    test al, al
    jz print_done
    mov ah, 0x0e
    int 0x10
    jmp print_string
print_done:
    ret

; 启用A20地址线
enable_a20:
    ; 使用快速A20方法
    in al, 0x92
    or al, 2
    out 0x92, al
    ret

; 32位保护模式代码
[bits 32]
protected_mode:
    ; 设置段寄存器
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    
    ; 设置栈指针
    mov esp, 0x90000
    
    ; 显示32位模式成功消息
    mov esi, protected_msg
    call print_string_32
    
    ; 停在这里，不进入64位模式
    jmp $

; 32位打印函数
print_string_32:
    mov edi, 0xb8000
    mov ah, 0x0f
print_loop_32:
    lodsb
    test al, al
    jz print_done_32
    stosw
    jmp print_loop_32
print_done_32:
    ret

; 数据段
boot_msg:
    db "JingOS Step Test: Starting...", 13, 10, 0

a20_msg:
    db "A20 Line Enabled", 13, 10, 0

gdt_msg:
    db "GDT Loaded", 13, 10, 0

protected_msg:
    db "32-bit Protected Mode SUCCESS!", 0

; GDT
align 8
gdt_start:
    ; 空描述符
    dq 0
    
    ; 32位代码段
    dw 0xffff
    dw 0
    db 0
    db 0x9a
    db 0xcf
    db 0
    
    ; 32位数据段
    dw 0xffff
    dw 0
    db 0
    db 0x92
    db 0xcf
    db 0

gdt_descriptor:
    dw gdt_descriptor - gdt_start - 1
    dd gdt_start

; 填充到512字节并添加引导签名
times 510-($-start) db 0
dw 0xaa55
