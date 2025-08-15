; JingOS 32位测试 Bootloader
; 测试32位模式切换的稳定性

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
    
    ; 等待BIOS稳定
    call wait_bios
    
    ; 清屏
    call clear_screen
    
    ; 显示16位模式消息
    mov si, msg16
    call print_string
    
    ; 启用A20地址线
    call enable_a20
    
    ; 显示A20启用消息
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

; 等待BIOS稳定
wait_bios:
    mov cx, 0x1000
wait_loop1:
    mov dx, 0x1000
wait_loop2:
    nop
    dec dx
    jnz wait_loop2
    dec cx
    jnz wait_loop1
    ret

; 清屏函数
clear_screen:
    mov ax, 0x0003
    int 0x10
    ret

; 启用A20地址线
enable_a20:
    in al, 0x92
    or al, 2
    out 0x92, al
    ret

; 16位打印函数
print_string:
    lodsb
    test al, al
    jz print_done
    mov ah, 0x0e
    mov bh, 0
    mov bl, 0x0f
    int 0x10
    jmp print_string
print_done:
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
    mov esi, msg32
    call print_string_32
    
    ; 显示稳定状态消息
    mov esi, stable_msg
    call print_string_32
    
    ; 无限循环，保持稳定
    jmp $

; 32位打印函数
print_string_32:
    mov edi, 0xb8000
    add edi, [screen_pos]
    mov ah, 0x0f  ; 白色前景，黑色背景
print_loop_32:
    lodsb
    test al, al
    jz print_done_32
    stosw
    jmp print_loop_32
print_done_32:
    ; 更新屏幕位置
    mov eax, edi
    sub eax, 0xb8000
    mov [screen_pos], eax
    ; 换行
    add dword [screen_pos], 160
    ret

; 数据段
screen_pos: dd 0

msg16:
    db "JingOS 32-bit Test - 16bit Mode OK", 13, 10, 0

a20_msg:
    db "A20 Line Enabled", 13, 10, 0

gdt_msg:
    db "GDT Loaded, Switching to 32-bit...", 13, 10, 0

msg32:
    db "32-bit Protected Mode: SUCCESS!", 0

stable_msg:
    db "System Stable in 32-bit Mode", 0

; GDT
align 8
gdt_start:
    ; 空描述符
    dq 0
    
    ; 32位代码段
    dw 0xffff    ; 段限制
    dw 0         ; 基址低16位
    db 0         ; 基址中8位
    db 0x9a      ; 访问字节
    db 0xcf      ; 标志和段限制高4位
    db 0         ; 基址高8位
    
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
