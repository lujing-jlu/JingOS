; JingOS 自定义 Bootloader
; 16位实模式启动代码

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

    ; 显示A20启用消息
    mov si, a20_msg
    call print_string

    ; 启用A20地址线
    call enable_a20

    ; 显示GDT加载消息
    mov si, gdt_msg
    call print_string

    ; 加载GDT
    lgdt [gdt_descriptor]

    ; 显示保护模式切换消息
    mov si, pmode_msg
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
    ; 使用键盘控制器方法
    call wait_8042
    mov al, 0xad
    out 0x64, al

    call wait_8042
    mov al, 0xd0
    out 0x64, al

    call wait_8042_data
    in al, 0x60
    push ax

    call wait_8042
    mov al, 0xd1
    out 0x64, al

    call wait_8042
    pop ax
    or al, 2
    out 0x60, al

    call wait_8042
    mov al, 0xae
    out 0x64, al

    call wait_8042
    ret

wait_8042:
    in al, 0x64
    test al, 2
    jnz wait_8042
    ret

wait_8042_data:
    in al, 0x64
    test al, 1
    jz wait_8042_data
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

    ; 显示内核加载消息
    mov esi, kernel_load_msg
    call print_string_32

    ; 先加载内核到内存
    call load_kernel_32

    ; 显示长模式切换消息
    mov esi, long_mode_msg
    call print_string_32

    ; 设置页表并切换到长模式
    call setup_paging
    call enable_long_mode

    ; 显示跳转消息
    mov esi, jump_msg
    call print_string_32

    ; 跳转到64位内核
    jmp 0x18:0x100000

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
    mov eax, 0x2003  ; 指向PDPT，存在+可写
    stosd

    ; 设置PDPT
    mov edi, 0x2000
    mov eax, 0x3003  ; 指向PD，存在+可写
    stosd

    ; 设置PD - 映射前2MB到物理地址0
    mov edi, 0x3000
    mov eax, 0x83    ; 2MB页面，存在+可写
    stosd

    ; 映射1MB处的内核
    mov eax, 0x100083  ; 1MB + 2MB页面标志
    stosd

    ; 继续映射其他页面
    mov ecx, 510
    mov eax, 0x400083  ; 从4MB开始
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

; 32位模式下的内核加载 (简化版)
load_kernel_32:
    ; 在32位模式下，我们假设内核已经通过dd命令加载到磁盘
    ; 这里只是一个占位符，实际的内核加载在磁盘镜像创建时完成
    ret

; 16位模式下的内核加载 (备用)
load_kernel:
    ; 使用BIOS中断13h读取磁盘
    mov ah, 0x02        ; 读取扇区功能
    mov al, 32          ; 读取32个扇区 (16KB)
    mov ch, 0           ; 柱面0
    mov cl, 2           ; 从第2个扇区开始
    mov dh, 0           ; 磁头0
    mov dl, 0x80        ; 驱动器号
    mov bx, 0x1000      ; 目标地址段
    mov es, bx
    mov bx, 0           ; 目标地址偏移
    int 0x13            ; 调用BIOS
    jc load_error       ; 如果出错跳转
    ret

load_error:
    ; 显示错误信息
    mov si, error_msg
    call print_string
    jmp $               ; 无限循环

; 数据段
boot_msg:
    db "JingOS Bootloader Starting...", 13, 10, 0

a20_msg:
    db "Enabling A20...", 13, 10, 0

gdt_msg:
    db "Loading GDT...", 13, 10, 0

pmode_msg:
    db "Switching to Protected Mode...", 13, 10, 0

protected_msg:
    db "Protected Mode OK", 0

kernel_load_msg:
    db "Loading Kernel...", 0

long_mode_msg:
    db "Switching to Long Mode...", 0

jump_msg:
    db "Jumping to Kernel...", 0

error_msg:
    db "Kernel Load Error!", 13, 10, 0

; GDT
align 8
gdt_start:
    ; 空描述符
    dq 0

    ; 代码段 (32位)
    dw 0xffff    ; 段限制
    dw 0         ; 基址低16位
    db 0         ; 基址中8位
    db 0x9a      ; 访问字节
    db 0xcf      ; 标志和段限制高4位
    db 0         ; 基址高8位

    ; 数据段 (32位)
    dw 0xffff
    dw 0
    db 0
    db 0x92
    db 0xcf
    db 0

    ; 代码段 (64位)
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
