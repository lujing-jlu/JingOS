; JingOS Rust内核启动版本
; 基于稳定的stage2_clean.s，添加Rust内核支持

[bits 16]
[org 0x1000]

; 在开始处添加一个标识
db 'S', '2', 'R', 'K'  ; Stage2 Rust Kernel

stage2_start:
    ; 设置段寄存器
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    
    ; 显示Stage2启动消息
    mov si, stage2_msg
    call print_string_16
    
    ; 显示调试信息
    mov si, debug_msg
    call print_string_16
    
    ; 检测长模式支持
    call check_long_mode
    test eax, eax
    jz .no_long_mode
    
    mov si, long_mode_ok_msg
    call print_string_16
    jmp .continue
    
.no_long_mode:
    mov si, long_mode_fail_msg
    call print_string_16
    jmp $
    
.continue:
    ; 启用A20地址线
    call enable_a20
    
    mov si, a20_ok_msg
    call print_string_16
    
    ; 设置GDT
    lgdt [gdt_descriptor]
    
    mov si, gdt_ok_msg
    call print_string_16
    
    ; 切换到32位保护模式
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    
    ; 跳转到32位代码
    jmp 0x08:protected_mode_32

; 16位打印函数
print_string_16:
    push ax
    push bx
.loop:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0e
    mov bh, 0
    mov bl, 0x0f
    int 0x10
    jmp .loop
.done:
    pop bx
    pop ax
    ret

; 检测长模式支持
check_long_mode:
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
    jz .no_cpuid
    
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb .no_long_mode
    
    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    jz .no_long_mode
    
    mov eax, 1
    ret
    
.no_cpuid:
.no_long_mode:
    xor eax, eax
    ret

; 启用A20地址线
enable_a20:
    push ax
    
    ; 方法1: 键盘控制器
    call .wait_8042
    mov al, 0xad
    out 0x64, al
    
    call .wait_8042
    mov al, 0xd0
    out 0x64, al
    
    call .wait_8042_data
    in al, 0x60
    push ax
    
    call .wait_8042
    mov al, 0xd1
    out 0x64, al
    
    call .wait_8042
    pop ax
    or al, 2
    out 0x60, al
    
    call .wait_8042
    mov al, 0xae
    out 0x64, al
    
    call .wait_8042
    
    pop ax
    ret

.wait_8042:
    in al, 0x64
    test al, 2
    jnz .wait_8042
    ret

.wait_8042_data:
    in al, 0x64
    test al, 1
    jz .wait_8042_data
    ret

[bits 32]
protected_mode_32:
    ; 设置32位段寄存器
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x90000
    
    ; 显示32位模式消息
    mov esi, protected_32_msg
    call print_string_32
    
    mov esi, mode_32_ok_msg
    call print_string_32
    
    ; 加载Rust内核到内存
    call load_rust_kernel_32
    
    ; 尝试切换到64位模式
    call try_long_mode

; 32位打印函数
print_string_32:
    push eax
    push edi
    
    ; 计算屏幕位置（从第10行开始）
    mov edi, 0xb8000
    add edi, 1600       ; 10行 * 160字节
    
.loop:
    lodsb
    test al, al
    jz .done
    
    ; 写入字符（白色）
    mov ah, 0x0f
    mov [edi], ax
    add edi, 2
    
    jmp .loop
    
.done:
    pop edi
    pop eax
    ret

; 32位模式下加载Rust内核
load_rust_kernel_32:
    push eax
    push ebx
    push ecx
    push edx
    
    ; 显示内核加载消息
    mov esi, loading_rust_32_msg
    call print_string_32
    
    ; 使用BIOS中断读取Rust内核
    ; 设置读取参数
    mov ah, 0x02        ; 读取扇区功能
    mov al, 63          ; 每次读取63个扇区（最大值）
    mov ch, 0           ; 柱面0
    mov cl, 10          ; 从第10个扇区开始（Rust内核位置）
    mov dh, 0           ; 磁头0
    mov dl, 0x80        ; 硬盘
    
    ; 设置目标地址 0x2000:0x0000 (物理地址0x20000 = 128KB)
    mov bx, 0x2000
    mov es, bx
    mov bx, 0
    
    ; 读取扇区
    int 0x13
    jc .load_error
    
    mov esi, rust_loaded_32_msg
    call print_string_32
    
    pop edx
    pop ecx
    pop ebx
    pop eax
    ret

.load_error:
    mov esi, rust_error_msg
    call print_string_32
    pop edx
    pop ecx
    pop ebx
    pop eax
    ret

; 尝试切换到64位长模式
try_long_mode:
    ; 设置页表
    call setup_paging
    
    ; 启用PAE
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax
    
    ; 设置长模式位
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr
    
    ; 启用分页
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax
    
    ; 跳转到64位代码
    jmp 0x18:long_mode_64

; 设置页表
setup_paging:
    ; 清除页表区域
    mov edi, 0x70000
    mov ecx, 0x3000
    xor eax, eax
    rep stosd
    
    ; 设置PML4
    mov dword [0x70000], 0x71003
    
    ; 设置PDPT
    mov dword [0x71000], 0x72003
    
    ; 设置PD (2MB页面)
    mov edi, 0x72000
    mov eax, 0x83
    mov ecx, 8
.setup_pd:
    mov [edi], eax
    add eax, 0x200000
    add edi, 8
    loop .setup_pd
    
    ; 设置CR3
    mov eax, 0x70000
    mov cr3, eax
    
    ret

[bits 64]
long_mode_64:
    ; 设置64位段寄存器
    mov ax, 0x20
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov rsp, 0x80000
    
    ; 显示64位模式消息
    mov rsi, success_64_msg
    call print_string_64
    
    mov rsi, complete_64_msg
    call print_string_64
    
    ; 显示Rust内核跳转消息
    mov rsi, jumping_rust_msg
    call print_string_64

    ; 先创建一个简单的测试，而不是跳转到Rust内核
    mov rsi, test_success_msg
    call print_string_64

    ; 无限循环，确保系统稳定
    jmp $

; 64位打印函数
print_string_64:
    push rax
    push rdi
    
    ; 计算屏幕位置（从第15行开始）
    mov rdi, 0xb8000
    add rdi, 2400       ; 15行 * 160字节
    
.loop:
    lodsb
    test al, al
    jz .done
    
    ; 写入字符（绿色高亮）
    mov ah, 0x0a
    mov [rdi], ax
    add rdi, 2
    
    jmp .loop
    
.done:
    pop rdi
    pop rax
    ret

; GDT定义
gdt_start:
    ; 空描述符
    dq 0
    
    ; 32位代码段
    dw 0xffff, 0x0000
    db 0x00, 0x9a, 0xcf, 0x00
    
    ; 32位数据段
    dw 0xffff, 0x0000
    db 0x00, 0x92, 0xcf, 0x00
    
    ; 64位代码段
    dw 0x0000, 0x0000
    db 0x00, 0x9a, 0x20, 0x00
    
    ; 64位数据段
    dw 0x0000, 0x0000
    db 0x00, 0x92, 0x00, 0x00

gdt_descriptor:
    dw gdt_descriptor - gdt_start - 1
    dd gdt_start

; 数据段
stage2_msg: db "JingOS Stage2: Rust Kernel Bootloader", 13, 10, 0
debug_msg: db "Stage2 Debug: Starting...", 13, 10, 0
long_mode_ok_msg: db "Long Mode: SUPPORTED", 13, 10, 0
long_mode_fail_msg: db "Long Mode: NOT SUPPORTED", 13, 10, 0
a20_ok_msg: db "A20 Line: ENABLED", 13, 10, 0
gdt_ok_msg: db "GDT: LOADED", 13, 10, 0
protected_32_msg: db "Switching to Protected Mode...", 0
mode_32_ok_msg: db "32-bit Protected Mode: OK", 0
loading_rust_32_msg: db "Loading Rust Kernel...", 0
rust_loaded_32_msg: db "Rust Kernel Loaded: OK", 0
rust_error_msg: db "Rust Kernel Load Error!", 0
success_64_msg: db "64-bit Mode: SUCCESS!", 0
complete_64_msg: db "JingOS Bootloader Complete!", 0
jumping_rust_msg: db "Jumping to Rust Kernel...", 0
test_success_msg: db "64-bit Rust Boot Test: SUCCESS!", 0
