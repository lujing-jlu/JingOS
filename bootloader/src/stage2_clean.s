; JingOS 多阶段 Bootloader - 第二阶段
; 功能：完整的64位模式切换和内核加载

[bits 16]
[org 0x1000]

; 在开始处添加一个标识，确保我们到达了这里
db 'S', '2', 'O', 'K'

stage2_start:
    ; 清除中断
    cli
    
    ; 设置段寄存器
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    
    ; 重新启用中断（需要BIOS服务）
    sti
    
    ; 显示第二阶段启动消息
    mov si, stage2_msg
    call print_string
    
    ; 显示调试消息
    mov si, debug_msg
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
    
    ; 显示即将切换到保护模式的消息
    mov si, switching_msg
    call print_string
    
    ; 切换到保护模式
    cli                     ; 确保中断关闭
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    
    ; 立即跳转到32位代码段
    jmp 0x08:protected_mode_32

; 16位函数
print_string:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0e
    mov bh, 0
    mov bl, 0x0f
    int 0x10
    jmp print_string
.done:
    ret

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
    jz .no_long_mode
    
    ; 检查扩展功能
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb .no_long_mode
    
    ; 检查长模式位
    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    jz .no_long_mode
    
    ; 支持长模式
    mov si, lm_support_msg
    call print_string
    ret

.no_long_mode:
    mov si, lm_no_support_msg
    call print_string
    ; 继续执行，但只使用32位模式
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
protected_mode_32:
    ; 设置段寄存器 - 使用数据段选择子
    mov ax, 0x10        ; 数据段选择子
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    
    ; 设置栈指针到安全位置
    mov esp, 0x90000
    
    ; 显示32位模式消息
    mov esi, protected_msg
    call print_string_32
    
    ; 显示成功消息
    mov esi, success_32_msg
    call print_string_32
    
    ; 尝试切换到64位模式
    call try_long_mode
    
    ; 如果返回到这里，说明64位切换失败，显示错误并停止
    mov esi, fallback_32_msg
    call print_string_32
    
    ; 无限循环
    jmp $

; 尝试切换到64位模式
try_long_mode:
    ; 显示64位切换消息
    mov esi, trying_64_msg
    call print_string_32
    
    ; 设置页表
    call setup_paging
    
    ; 显示页表设置完成
    mov esi, paging_msg
    call print_string_32
    
    ; 启用长模式
    ; 加载页表
    mov eax, 0x2000
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
    
    ; 显示长模式启用消息
    mov esi, long_mode_msg
    call print_string_32
    
    ; 跳转到64位代码
    jmp 0x18:long_mode_64

; 设置页表
setup_paging:
    ; 清除页表区域 (0x2000-0x5FFF, 16KB)
    mov edi, 0x2000
    mov ecx, 0x1000     ; 4096 dwords = 16KB
    xor eax, eax
    rep stosd
    
    ; 设置PML4[0] -> PDPT (0x3000)
    mov dword [0x2000], 0x3003
    mov dword [0x2004], 0
    
    ; 设置PDPT[0] -> PD (0x4000)
    mov dword [0x3000], 0x4003
    mov dword [0x3004], 0
    
    ; 设置PD[0-7] -> 2MB页面，映射前16MB
    mov edi, 0x4000
    mov eax, 0x83       ; 2MB页面，存在，可写
    mov ecx, 8          ; 映射8个2MB页面 = 16MB
.map_loop:
    mov [edi], eax
    mov dword [edi + 4], 0
    add edi, 8          ; 下一个PD条目
    add eax, 0x200000   ; 下一个2MB页面
    loop .map_loop
    
    ret

; 32位打印函数
print_string_32:
    push eax
    push ebx
    push edi
    push esi
    
    ; 获取当前屏幕位置
    mov edi, 0xb8000
    mov eax, [screen_pos_32]
    add edi, eax
    
.loop:
    lodsb               ; 加载字符到AL
    test al, al         ; 检查是否为字符串结束
    jz .done
    
    ; 写入字符（AL）和属性（0x0F = 白色）
    mov ah, 0x0f
    mov [edi], ax       ; 直接写入字符和属性
    add edi, 2          ; 移动到下一个字符位置
    
    jmp .loop
    
.done:
    ; 计算新的屏幕位置
    mov eax, edi
    sub eax, 0xb8000
    
    ; 移动到下一行的开始
    mov ebx, 160        ; 每行160字节（80字符 * 2字节）
    mov edx, 0
    div ebx             ; EAX = 行号, EDX = 列偏移
    inc eax             ; 移动到下一行
    mul ebx             ; EAX = 下一行的字节偏移
    
    mov [screen_pos_32], eax
    
    pop esi
    pop edi
    pop ebx
    pop eax
    ret



; 64位长模式代码
[bits 64]
long_mode_64:
    ; 设置64位段寄存器
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    
    ; 设置64位栈
    mov rsp, 0x90000
    
    ; 显示64位成功消息
    mov rsi, success_64_msg
    call print_string_64
    
    ; 显示完成消息
    mov rsi, complete_64_msg
    call print_string_64

    ; 显示系统就绪消息
    mov rsi, system_ready_msg
    call print_string_64

    ; 显示内核跳转消息
    mov rsi, jumping_kernel_msg
    call print_string_64

    ; 先在内存中创建一个简单的测试代码
    call create_test_kernel

    ; 跳转到内存中的测试代码
    mov rax, 0x10000    ; 测试内核地址
    jmp rax

; 64位打印函数
print_string_64:
    push rax
    push rdi
    
    ; 计算屏幕位置
    mov rdi, 0xb8000
    mov rax, [screen_pos_64]
    add rdi, rax
    
.loop:
    lodsb
    test al, al
    jz .done
    
    ; 写入字符（绿色）
    mov ah, 0x0a
    mov [rdi], ax
    add rdi, 2
    
    jmp .loop
    
.done:
    ; 更新屏幕位置到下一行
    mov rax, rdi
    sub rax, 0xb8000
    
    ; 移动到下一行
    mov rbx, 160
    mov rdx, 0
    div rbx
    inc rax
    mul rbx
    
    mov [screen_pos_64], rax
    
    pop rdi
    pop rax
    ret

; 在内存中创建简单的测试内核
create_test_kernel:
    push rax
    push rdi
    push rsi

    ; 目标地址 0x10000
    mov rdi, 0x10000

    ; 创建清屏和显示内核信息的代码

    ; 1. 清屏代码：mov rdi, 0xb8000 (48 bf 00 80 0b 00 00 00 00 00)
    mov al, 0x48
    stosb
    mov al, 0xbf
    stosb
    mov al, 0x00
    stosb
    mov al, 0x80
    stosb
    mov al, 0x0b
    stosb
    mov al, 0x00
    stosb
    mov al, 0x00
    stosb
    mov al, 0x00
    stosb
    mov al, 0x00
    stosb
    mov al, 0x00
    stosb

    ; mov rcx, 2000 (48 c7 c1 d0 07 00 00)
    mov al, 0x48
    stosb
    mov al, 0xc7
    stosb
    mov al, 0xc1
    stosb
    mov al, 0xd0
    stosb
    mov al, 0x07
    stosb
    mov al, 0x00
    stosb
    mov al, 0x00
    stosb

    ; mov ax, 0x0720 (66 b8 20 07)
    mov al, 0x66
    stosb
    mov al, 0xb8
    stosb
    mov al, 0x20
    stosb
    mov al, 0x07
    stosb

    ; rep stosw (f3 66 ab)
    mov al, 0xf3
    stosb
    mov al, 0x66
    stosb
    mov al, 0xab
    stosb

    ; 2. 显示内核标题：mov rdi, 0xb8000 (48 bf 00 80 0b 00 00 00 00 00)
    mov al, 0x48
    stosb
    mov al, 0xbf
    stosb
    mov al, 0x00
    stosb
    mov al, 0x80
    stosb
    mov al, 0x0b
    stosb
    mov al, 0x00
    stosb
    mov al, 0x00
    stosb
    mov al, 0x00
    stosb
    mov al, 0x00
    stosb
    mov al, 0x00
    stosb

    ; 写入 "JingOS Kernel v3.0" 的前几个字符
    ; mov word [rdi], 0x0c4a (66 c7 07 4a 0c) - 'J' 红色
    mov al, 0x66
    stosb
    mov al, 0xc7
    stosb
    mov al, 0x07
    stosb
    mov al, 0x4a
    stosb
    mov al, 0x0c
    stosb

    ; mov word [rdi+2], 0x0c69 (66 c7 47 02 69 0c) - 'i' 红色
    mov al, 0x66
    stosb
    mov al, 0xc7
    stosb
    mov al, 0x47
    stosb
    mov al, 0x02
    stosb
    mov al, 0x69
    stosb
    mov al, 0x0c
    stosb

    ; mov word [rdi+4], 0x0c6e (66 c7 47 04 6e 0c) - 'n' 红色
    mov al, 0x66
    stosb
    mov al, 0xc7
    stosb
    mov al, 0x47
    stosb
    mov al, 0x04
    stosb
    mov al, 0x6e
    stosb
    mov al, 0x0c
    stosb

    ; mov word [rdi+6], 0x0c67 (66 c7 47 06 67 0c) - 'g' 红色
    mov al, 0x66
    stosb
    mov al, 0xc7
    stosb
    mov al, 0x47
    stosb
    mov al, 0x06
    stosb
    mov al, 0x67
    stosb
    mov al, 0x0c
    stosb

    ; jmp $ (eb fe)
    mov al, 0xeb
    stosb
    mov al, 0xfe
    stosb

    pop rsi
    pop rdi
    pop rax
    ret



; 数据段
screen_pos: dd 0
screen_pos_32: dd 1600      ; 从第10行开始显示32位消息
screen_pos_64: dq 2400      ; 从第15行开始显示64位消息

stage2_msg: db "JingOS Stage2: 64-bit Bootloader", 13, 10, 0
debug_msg: db "Stage2 Debug: Starting...", 13, 10, 0
lm_support_msg: db "Long Mode: SUPPORTED", 13, 10, 0
lm_no_support_msg: db "Long Mode: NOT SUPPORTED", 13, 10, 0
a20_msg: db "A20 Line: ENABLED", 13, 10, 0
gdt_msg: db "GDT: LOADED", 13, 10, 0
switching_msg: db "Switching to Protected Mode...", 13, 10, 0
protected_msg: db "32-bit Protected Mode: OK", 0
success_32_msg: db "32-bit Mode: SUCCESS!", 0
trying_64_msg: db "Trying 64-bit Long Mode...", 0
paging_msg: db "Paging: SETUP", 0
long_mode_msg: db "Long Mode: ENABLED", 0
fallback_32_msg: db "Fallback: 32-bit Mode", 0
success_64_msg: db "64-bit Mode: SUCCESS!", 0
complete_64_msg: db "JingOS Bootloader Complete!", 0
system_ready_msg: db "System Ready - 64-bit Mode Active!", 0
jumping_kernel_msg: db "Jumping to Test Kernel...", 0

; GDT - 全局描述符表
align 8
gdt_start:
    ; 空描述符 (必须)
    dq 0x0000000000000000
    
    ; 32位代码段 (选择子 0x08)
    ; 基址=0, 限制=4GB, 可执行, 可读, 32位
    dq 0x00cf9a000000ffff
    
    ; 32位数据段 (选择子 0x10)  
    ; 基址=0, 限制=4GB, 可写, 32位
    dq 0x00cf92000000ffff
    
    ; 64位代码段 (选择子 0x18)
    ; 基址=0, 限制=4GB, 可执行, 64位
    dq 0x00af9a000000ffff

gdt_descriptor:
    dw gdt_descriptor - gdt_start - 1    ; GDT大小
    dd gdt_start                         ; GDT地址
