; JingOS 64位调试 Bootloader
; 详细调试64位模式切换

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
    
    ; 检查CPU是否支持长模式
    call check_long_mode
    
    ; 启用A20地址线
    call enable_a20
    mov si, a20_msg
    call print_string
    
    ; 加载GDT
    lgdt [gdt_descriptor]
    mov si, gdt_msg
    call print_string
    
    ; 切换到保护模式
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    
    ; 跳转到32位代码
    jmp 0x08:protected_mode

; 检查长模式支持
check_long_mode:
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
    jz no_long_mode
    
    ; 检查扩展功能
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb no_long_mode
    
    ; 检查长模式位
    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    jz no_long_mode
    
    mov si, cpu_ok_msg
    call print_string
    ret

no_long_mode:
    mov si, cpu_error_msg
    call print_string
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
    
    ; 等待一下，让用户看到消息
    call wait_a_bit
    
    ; 设置页表
    call setup_paging_debug
    
    ; 等待
    call wait_a_bit
    
    ; 启用长模式
    call enable_long_mode_debug
    
    ; 等待
    call wait_a_bit
    
    ; 跳转到64位代码
    jmp 0x18:long_mode_64

; 等待函数
wait_a_bit:
    mov ecx, 0x1000000
wait_loop:
    nop
    loop wait_loop
    ret

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
    add dword [screen_pos], 160  ; 换行
    ret

; 调试版页表设置
setup_paging_debug:
    mov esi, paging_start_msg
    call print_string_32
    
    ; 清除页表区域
    mov edi, 0x1000
    mov ecx, 0x1000
    xor eax, eax
    rep stosd
    
    mov esi, clear_msg
    call print_string_32
    
    ; 设置PML4[0] -> PDPT
    mov dword [0x1000], 0x2003
    mov dword [0x1004], 0
    
    mov esi, pml4_msg
    call print_string_32
    
    ; 设置PDPT[0] -> PD
    mov dword [0x2000], 0x3003
    mov dword [0x2004], 0
    
    mov esi, pdpt_msg
    call print_string_32
    
    ; 设置PD[0] -> 2MB页面
    mov dword [0x3000], 0x83
    mov dword [0x3004], 0
    
    mov esi, pd_msg
    call print_string_32
    
    ret

; 调试版长模式启用
enable_long_mode_debug:
    mov esi, longmode_start_msg
    call print_string_32
    
    ; 加载页表
    mov eax, 0x1000
    mov cr3, eax
    
    mov esi, cr3_msg
    call print_string_32
    
    ; 启用PAE
    mov eax, cr4
    or eax, 0x20
    mov cr4, eax
    
    mov esi, pae_msg
    call print_string_32
    
    ; 启用长模式位
    mov ecx, 0xc0000080
    rdmsr
    or eax, 0x100
    wrmsr
    
    mov esi, lme_msg
    call print_string_32
    
    ; 启用分页
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax
    
    mov esi, paging_enabled_msg
    call print_string_32
    
    ret

; 64位长模式代码
[bits 64]
long_mode_64:
    ; 显示成功消息
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

boot_msg: db "JingOS 64-bit Debug Starting...", 13, 10, 0
cpu_ok_msg: db "CPU Long Mode: SUPPORTED", 13, 10, 0
cpu_error_msg: db "CPU Long Mode: NOT SUPPORTED", 13, 10, 0
a20_msg: db "A20 Line: ENABLED", 13, 10, 0
gdt_msg: db "GDT: LOADED", 13, 10, 0
protected_msg: db "32-bit Mode: OK", 0
paging_start_msg: db "Setting up paging...", 0
clear_msg: db "Memory cleared", 0
pml4_msg: db "PML4 set", 0
pdpt_msg: db "PDPT set", 0
pd_msg: db "PD set", 0
longmode_start_msg: db "Enabling long mode...", 0
cr3_msg: db "CR3 loaded", 0
pae_msg: db "PAE enabled", 0
lme_msg: db "LME bit set", 0
paging_enabled_msg: db "Paging enabled", 0
success_msg: db "64-BIT SUCCESS!", 0

; GDT
align 8
gdt_start:
    dq 0                    ; 空描述符

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

; 检查大小并填充
%if ($-start) > 510
    %error "Bootloader too large"
%endif

times 510-($-start) db 0
dw 0xaa55
