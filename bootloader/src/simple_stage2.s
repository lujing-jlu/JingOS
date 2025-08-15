; JingOS 简化 Stage2 - 只做基本测试

[bits 16]
[org 0x1000]

; 在开始处添加一个标识，确保我们到达了这里
db 'S', '2', 'O', 'K'

stage2_start:
    ; 设置段寄存器
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    
    ; 显示Stage2启动消息
    mov si, stage2_msg
    call print_string
    
    ; 显示测试消息
    mov si, test_msg
    call print_string
    
    ; 无限循环
    jmp $

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

; 数据段
stage2_msg:
    db "Stage2: Hello from Stage2!", 13, 10, 0

test_msg:
    db "Stage2: Basic test OK", 13, 10, 0
