; C内核的汇编启动代码
; 为64位模式设计

[bits 64]

; 外部符号
extern kernel_main

; 内核入口点
global _start
_start:
    ; 设置64位段寄存器
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    
    ; 设置栈指针
    mov rsp, 0x80000
    
    ; 调用C内核主函数
    call kernel_main
    
    ; 如果kernel_main返回，无限循环
.hang:
    hlt
    jmp .hang
