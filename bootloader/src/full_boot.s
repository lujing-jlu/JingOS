; JingOS 完整 Bootloader
; 支持16位->32位->64位模式切换

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
    
    ; 加载GDT
    lgdt [gdt_descriptor]
    
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
    
    ; 显示保护模式消息
    mov esi, protected_msg
    call print_string_32
    
    ; 设置页表并切换到长模式
    call setup_paging
    call enable_long_mode
    
    ; 跳转到64位内核 (内核在0x10000处)
    jmp 0x18:0x10000

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

; 设置页表
setup_paging:
    ; 清除页表区域
    mov edi, 0x1000
    mov ecx, 0x1000
    xor eax, eax
    rep stosd
    
    ; 设置PML4
    mov edi, 0x1000
    mov eax, 0x2003
    stosd
    
    ; 设置PDPT
    mov edi, 0x2000
    mov eax, 0x3003
    stosd
    
    ; 设置PD (2MB页面)
    mov edi, 0x3000
    mov eax, 0x83
    mov ecx, 512
setup_pd_loop:
    stosd
    add eax, 0x200000
    loop setup_pd_loop
    
    ret

; 启用长模式
enable_long_mode:
    ; 加载页表
    mov eax, 0x1000
    mov cr3, eax
    
    ; 启用PAE
    mov eax, cr4
    or eax, 0x20
    mov cr4, eax
    
    ; 启用长模式
    mov ecx, 0xc0000080
    rdmsr
    or eax, 0x100
    wrmsr
    
    ; 启用分页
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax
    
    ret

; 数据段
boot_msg:
    db "JingOS Bootloader v1.0", 13, 10, 0

protected_msg:
    db "32-bit Mode OK, Loading Kernel...", 0

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
    
    ; 64位代码段
    dw 0xffff
    dw 0
    db 0
    db 0x9a
    db 0xaf
    db 0

gdt_descriptor:
    dw gdt_descriptor - gdt_start - 1
    dd gdt_start

; 填充到512字节并添加引导签名
times 510-($-start) db 0
dw 0xaa55
