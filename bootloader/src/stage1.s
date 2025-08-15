; JingOS 多阶段 Bootloader - 第一阶段
; 功能：基本初始化，加载第二阶段bootloader

[bits 16]
[org 0x7c00]

stage1_start:
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
    
    ; 清屏
    mov ax, 0x0003
    int 0x10
    
    ; 显示第一阶段启动消息
    mov si, stage1_msg
    call print_string
    
    ; 加载第二阶段bootloader
    call load_stage2
    
    ; 显示加载完成消息
    mov si, load_ok_msg
    call print_string

    ; 等待一下让用户看到消息
    mov cx, 0x1000
wait_loop:
    nop
    loop wait_loop

    ; 显示即将跳转的消息
    mov si, jump_msg
    call print_string

    ; 跳转到第二阶段
    ; 使用远跳转，确保段寄存器正确
    jmp 0x0000:0x1000

; 打印字符串函数
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

; 加载第二阶段bootloader
load_stage2:
    ; 设置磁盘读取参数
    mov ah, 0x02        ; 读取扇区功能
    mov al, 8           ; 读取8个扇区（4KB，足够第二阶段）
    mov ch, 0           ; 柱面0
    mov cl, 2           ; 从第2个扇区开始（第1个是stage1）
    mov dh, 0           ; 磁头0
    mov dl, 0x80        ; 驱动器号（硬盘）

    ; 设置目标地址 0x0000:0x1000 (物理地址0x1000)
    mov bx, 0x0000
    mov es, bx
    mov bx, 0x1000

    ; 调用BIOS中断
    int 0x13
    jc load_error

    ret

load_error:
    mov si, error_msg
    call print_string

    ; 显示错误代码
    mov si, error_code_msg
    call print_string

    ; 显示AH寄存器的值（错误代码）
    mov al, ah
    call print_hex_byte

    ; 无限循环
    jmp $

; 打印十六进制字节
print_hex_byte:
    push ax
    shr al, 4
    call print_hex_digit
    pop ax
    and al, 0x0f
    call print_hex_digit
    ret

print_hex_digit:
    cmp al, 9
    jle .digit
    add al, 'A' - 10
    jmp .print
.digit:
    add al, '0'
.print:
    mov ah, 0x0e
    int 0x10
    ret

; 数据段
stage1_msg:
    db "JingOS Stage1: Loading Stage2...", 13, 10, 0

load_ok_msg:
    db "Stage2 Loaded, Jumping...", 13, 10, 0

error_msg:
    db "ERROR: Failed to load Stage2!", 13, 10, 0

error_code_msg:
    db "Error code: 0x", 0

jump_msg:
    db "Jumping to Stage2...", 13, 10, 0

; 填充到510字节
times 510-($-stage1_start) db 0

; 引导签名
dw 0xaa55
