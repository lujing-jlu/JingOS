; JingOS 最小64位调试 Bootloader
[bits 16]
[org 0x7c00]

start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    
    ; 显示启动
    mov si, msg1
    call print
    
    ; 检查长模式支持
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb error
    
    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    jz error
    
    ; CPU支持长模式
    mov si, msg2
    call print
    
    ; 启用A20
    in al, 0x92
    or al, 2
    out 0x92, al
    
    ; 加载GDT
    lgdt [gdt_desc]
    
    ; 进入保护模式
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    jmp 0x08:pm32

error:
    mov si, err_msg
    call print
    jmp $

print:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0e
    int 0x10
    jmp print
.done:
    ret

[bits 32]
pm32:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov esp, 0x90000
    
    ; 显示32位OK
    mov esi, msg3
    call print32
    
    ; 设置最简页表
    ; 清除页表区域
    mov edi, 0x1000
    mov ecx, 0x1000
    xor eax, eax
    rep stosd
    
    ; PML4[0] = PDPT地址
    mov dword [0x1000], 0x2003
    ; PDPT[0] = PD地址  
    mov dword [0x2000], 0x3003
    ; PD[0] = 2MB页面
    mov dword [0x3000], 0x83
    
    ; 显示页表OK
    mov esi, msg4
    call print32
    
    ; 启用长模式
    mov eax, 0x1000
    mov cr3, eax
    
    mov eax, cr4
    or eax, 0x20
    mov cr4, eax
    
    mov ecx, 0xc0000080
    rdmsr
    or eax, 0x100
    wrmsr
    
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax
    
    ; 显示长模式启用
    mov esi, msg5
    call print32
    
    ; 跳转到64位
    jmp 0x18:lm64

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

[bits 64]
lm64:
    ; 64位成功
    mov rsi, msg6
    call print64
    jmp $

print64:
    mov rdi, 0xb8000
    add rdi, [pos]
    mov ah, 0x0a
.loop:
    lodsb
    test al, al
    jz .done
    stosw
    jmp .loop
.done:
    ret

pos: dd 0

msg1: db "Start", 13, 10, 0
msg2: db "CPU OK", 13, 10, 0
msg3: db "32bit OK", 0
msg4: db "Page OK", 0
msg5: db "LM Enable", 0
msg6: db "64bit SUCCESS!", 0
err_msg: db "CPU Error", 13, 10, 0

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

gdt_desc:
    dw gdt_desc - gdt - 1
    dd gdt

times 510-($-start) db 0
dw 0xaa55
