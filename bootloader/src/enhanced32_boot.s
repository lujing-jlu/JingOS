; JingOS 增强32位 Bootloader
; 基于稳定的32位版本，逐步添加64位支持

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
    
    ; 检查是否支持长模式
    call check_long_mode_support
    
    ; 如果支持，尝试切换到64位
    cmp eax, 1
    je try_64bit_mode
    
    ; 如果不支持，继续32位模式
    mov esi, mode32_msg
    call print_string_32
    
    ; 跳转到32位内核
    jmp 0x08:0x8000  ; 内核在第二个扇区，0x7e00 + 偏移
    
try_64bit_mode:
    ; 显示64位切换消息
    mov esi, mode64_msg
    call print_string_32
    
    ; 设置页表
    call setup_simple_paging
    
    ; 启用长模式
    call enable_long_mode
    
    ; 跳转到64位内核
    jmp 0x18:0x8000

; 检查长模式支持
check_long_mode_support:
    ; 检查CPUID是否可用
    pushfd
    pop eax
    mov ecx, eax
    xor eax, 1 << 21
    push eax
    popfd
    pushfd
    pop eax
    push ecx
    popfd
    xor eax, ecx
    jz no_long_mode_support
    
    ; 检查扩展功能
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb no_long_mode_support
    
    ; 检查长模式位
    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    jz no_long_mode_support
    
    ; 支持长模式
    mov esi, lm_support_msg
    call print_string_32
    mov eax, 1
    ret

no_long_mode_support:
    mov esi, lm_no_support_msg
    call print_string_32
    mov eax, 0
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
    mov dword [0x1004], 0
    
    ; PDPT[0] -> PD
    mov dword [0x2000], 0x3003
    mov dword [0x2004], 0
    
    ; PD[0] -> 2MB页面，映射前2MB
    mov dword [0x3000], 0x83
    mov dword [0x3004], 0
    
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
    db "Enhanced 16bit OK", 13, 10, 0

a20_msg:
    db "A20 OK", 13, 10, 0

gdt_msg:
    db "GDT OK", 13, 10, 0

msg32:
    db "32bit OK", 0

lm_support_msg:
    db "LM: YES", 0

lm_no_support_msg:
    db "LM: NO", 0

mode32_msg:
    db "Mode: 32bit", 0

mode64_msg:
    db "Mode: 64bit", 0

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
