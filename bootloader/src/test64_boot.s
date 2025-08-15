; JingOS 64位测试 Bootloader
; 测试64位模式切换，不加载内核

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
    mov esp, 0x90000
    
    ; 显示32位模式消息
    mov esi, protected_msg
    call print_string_32
    
    ; 设置简单的页表
    call setup_simple_paging
    
    ; 显示页表设置完成消息
    mov esi, paging_msg
    call print_string_32
    
    ; 启用长模式
    call enable_long_mode
    
    ; 显示长模式启用消息
    mov esi, long_mode_msg
    call print_string_32
    
    ; 跳转到64位代码
    jmp 0x18:long_mode_64

; 32位打印函数
print_string_32:
    mov edi, 0xb8000
    add edi, [screen_pos]
    mov ah, 0x0f
print_loop_32:
    lodsb
    test al, al
    jz print_done_32
    stosw
    jmp print_loop_32
print_done_32:
    mov eax, edi
    sub eax, 0xb8000
    mov [screen_pos], eax
    ; 换行
    add dword [screen_pos], 160
    ret

; 设置简单页表
setup_simple_paging:
    ; 清除页表区域
    mov edi, 0x1000
    mov ecx, 0x1000
    xor eax, eax
    rep stosd
    
    ; PML4[0] -> PDPT
    mov dword [0x1000], 0x2003
    
    ; PDPT[0] -> PD
    mov dword [0x2000], 0x3003
    
    ; PD[0] -> 2MB页面，映射前2MB
    mov dword [0x3000], 0x83
    
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
    
    ; 启用长模式位
    mov ecx, 0xc0000080
    rdmsr
    or eax, 0x100
    wrmsr
    
    ; 启用分页
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax
    
    ret

; 64位长模式代码
[bits 64]
long_mode_64:
    ; 显示64位模式成功消息
    mov rsi, success_msg
    call print_string_64
    
    ; 无限循环
    jmp $

; 64位打印函数
print_string_64:
    mov rdi, 0xb8000
    add rdi, [screen_pos]
    mov ah, 0x0a  ; 绿色
print_loop_64:
    lodsb
    test al, al
    jz print_done_64
    stosw
    jmp print_loop_64
print_done_64:
    ret

; 数据段
screen_pos: dd 0

boot_msg:
    db "JingOS 64-bit Test Starting...", 13, 10, 0

protected_msg:
    db "32-bit Mode OK", 0

paging_msg:
    db "Paging Setup OK", 0

long_mode_msg:
    db "Long Mode Enabled", 0

success_msg:
    db "64-bit Mode SUCCESS!", 0

; GDT
align 8
gdt_start:
    ; 空描述符
    dq 0
    
    ; 32位代码段
    dw 0xffff, 0
    db 0, 0x9a, 0xcf, 0
    
    ; 32位数据段
    dw 0xffff, 0
    db 0, 0x92, 0xcf, 0
    
    ; 64位代码段
    dw 0xffff, 0
    db 0, 0x9a, 0xaf, 0

gdt_descriptor:
    dw gdt_descriptor - gdt_start - 1
    dd gdt_start

; 填充到512字节
times 510-($-start) db 0
dw 0xaa55
