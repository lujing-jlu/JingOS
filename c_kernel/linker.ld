/* JingOS C内核链接器脚本 */
/* 为64位模式和我们的bootloader设计 */

ENTRY(_start)

SECTIONS
{
    /* 内核加载到128KB处 (0x20000) */
    . = 0x20000;
    
    /* 代码段 */
    .text : ALIGN(4K)
    {
        *(.text)
    }
    
    /* 只读数据段 */
    .rodata : ALIGN(4K)
    {
        *(.rodata)
    }
    
    /* 数据段 */
    .data : ALIGN(4K)
    {
        *(.data)
    }
    
    /* BSS段 */
    .bss : ALIGN(4K)
    {
        *(COMMON)
        *(.bss)
    }
}
