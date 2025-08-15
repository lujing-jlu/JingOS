; JingOS 64位逐步测试 Bootloader
; 在32位基础上逐步添加64位功能

[bits 16]
[org 0x7c00]

start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    
    call wait_bios
    call clear_screen
    
    mov si, msg16
    call print_string
    
    call enable_a20
    mov si, a20_msg
    call print_string
    
    lgdt [gdt_descriptor]
    mov si, gdt_msg
    call print_string
    
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    jmp 0x08:protected_mode

wait_bios:
    mov cx, 0x1000
.loop1:
    mov dx, 0x1000
.loop2:
    nop
    dec dx
    jnz .loop2
    dec cx
    jnz .loop1
    ret

clear_screen:
    mov ax, 0x0003
    int 0x10
    ret

enable_a20:
    in al, 0x92
    or al, 2
    out 0x92, al
    ret

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

[bits 32]
protected_mode:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x90000
    
    ; 显示32位成功
    mov esi, msg32
    call print32
    
    ; 步骤1: 设置页表
    mov esi, step1_msg
    call print32
    call setup_paging
    
    ; 步骤2: 加载页表到CR3
    mov esi, step2_msg
    call print32
    mov eax, 0x1000
    mov cr3, eax
    
    ; 步骤3: 启用PAE
    mov esi, step3_msg
    call print32
    mov eax, cr4
    or eax, 0x20
    mov cr4, eax
    
    ; 步骤4: 启用长模式位
    mov esi, step4_msg
    call print32
    mov ecx, 0xc0000080
    rdmsr
    or eax, 0x100
    wrmsr
    
    ; 步骤5: 启用分页
    mov esi, step5_msg
    call print32
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax
    
    ; 如果到这里没有崩溃，显示成功消息
    mov esi, success_msg
    call print32
    
    ; 尝试跳转到64位代码
    jmp 0x18:long_mode

print32:
    mov edi, 0xb8000
    add edi, [pos]
    mov ah, 0x0f
.loop:
    lodsb
    test al, al
    jz .done
    stosw
    jmp .loop
.done:
    mov eax, edi
    sub eax, 0xb8000
    mov [pos], eax
    add dword [pos], 160
    ret

setup_paging:
    ; 清除页表区域
    mov edi, 0x1000
    mov ecx, 0x1000
    xor eax, eax
    rep stosd
    
    ; 设置最简单的页表结构
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

[bits 64]
long_mode:
    ; 如果能到这里，说明64位切换成功
    mov rsi, lm_success_msg
    call print64
    jmp $

print64:
    mov rdi, 0xb8000
    add rdi, [pos]
    mov ah, 0x0a  ; 绿色
.loop:
    lodsb
    test al, al
    jz .done
    stosw
    jmp .loop
.done:
    ret

; 数据
pos: dd 0

msg16: db "16bit OK", 13, 10, 0
a20_msg: db "A20 OK", 13, 10, 0
gdt_msg: db "GDT OK", 13, 10, 0
msg32: db "32bit OK", 0
step1_msg: db "S1:Paging", 0
step2_msg: db "S2:CR3", 0
step3_msg: db "S3:PAE", 0
step4_msg: db "S4:LME", 0
step5_msg: db "S5:PG", 0
success_msg: db "Steps OK!", 0
lm_success_msg: db "64bit OK!", 0

align 8
gdt:
    dq 0
    ; 32位代码
    dw 0xffff, 0
    db 0, 0x9a, 0xcf, 0
    ; 32位数据
    dw 0xffff, 0
    db 0, 0x92, 0xcf, 0
    ; 64位代码
    dw 0xffff, 0
    db 0, 0x9a, 0xaf, 0

gdt_descriptor:
    dw gdt_descriptor - gdt - 1
    dd gdt

times 510-($-start) db 0
dw 0xaa55
