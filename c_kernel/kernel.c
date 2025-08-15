// JingOS C内核 - 简单测试版本
// 专门为我们的64位bootloader设计

// VGA缓冲区地址
#define VGA_BUFFER 0xB8000
#define VGA_WIDTH 80
#define VGA_HEIGHT 25

// 颜色定义
#define COLOR_BLACK 0
#define COLOR_BLUE 1
#define COLOR_GREEN 2
#define COLOR_CYAN 3
#define COLOR_RED 4
#define COLOR_MAGENTA 5
#define COLOR_BROWN 6
#define COLOR_LIGHT_GREY 7
#define COLOR_DARK_GREY 8
#define COLOR_LIGHT_BLUE 9
#define COLOR_LIGHT_GREEN 10
#define COLOR_LIGHT_CYAN 11
#define COLOR_LIGHT_RED 12
#define COLOR_LIGHT_MAGENTA 13
#define COLOR_LIGHT_BROWN 14
#define COLOR_WHITE 15

// 创建颜色属性
static inline unsigned char vga_entry_color(unsigned char fg, unsigned char bg) {
    return fg | bg << 4;
}

// 创建VGA条目
static inline unsigned short vga_entry(unsigned char uc, unsigned char color) {
    return (unsigned short) uc | (unsigned short) color << 8;
}

// 全局变量
static unsigned short* vga_buffer = (unsigned short*) VGA_BUFFER;
static int terminal_row = 0;
static int terminal_column = 0;
static unsigned char terminal_color = 0;

// 初始化终端
void terminal_initialize(void) {
    terminal_row = 0;
    terminal_column = 0;
    terminal_color = vga_entry_color(COLOR_LIGHT_GREEN, COLOR_BLACK);
    
    // 清屏
    for (int y = 0; y < VGA_HEIGHT; y++) {
        for (int x = 0; x < VGA_WIDTH; x++) {
            const int index = y * VGA_WIDTH + x;
            vga_buffer[index] = vga_entry(' ', terminal_color);
        }
    }
}

// 设置颜色
void terminal_setcolor(unsigned char color) {
    terminal_color = color;
}

// 在指定位置放置字符
void terminal_putentryat(char c, unsigned char color, int x, int y) {
    const int index = y * VGA_WIDTH + x;
    vga_buffer[index] = vga_entry(c, color);
}

// 输出字符
void terminal_putchar(char c) {
    if (c == '\n') {
        terminal_column = 0;
        terminal_row++;
    } else {
        terminal_putentryat(c, terminal_color, terminal_column, terminal_row);
        terminal_column++;
    }
    
    if (terminal_column == VGA_WIDTH) {
        terminal_column = 0;
        terminal_row++;
    }
    
    if (terminal_row == VGA_HEIGHT) {
        terminal_row = 0;
    }
}

// 输出字符串
void terminal_write(const char* data) {
    while (*data) {
        terminal_putchar(*data);
        data++;
    }
}

// 内核主函数 - 由bootloader调用
void kernel_main(void) {
    // 初始化终端
    terminal_initialize();
    
    // 显示欢迎信息
    terminal_setcolor(vga_entry_color(COLOR_LIGHT_RED, COLOR_BLACK));
    terminal_write("=== JingOS C Kernel v1.0 ===\n");
    
    terminal_setcolor(vga_entry_color(COLOR_LIGHT_GREEN, COLOR_BLACK));
    terminal_write("64-bit Long Mode: ACTIVE\n");
    terminal_write("C Kernel: SUCCESS!\n");
    terminal_write("Bootloader Integration: OK\n");
    
    terminal_setcolor(vga_entry_color(COLOR_LIGHT_CYAN, COLOR_BLACK));
    terminal_write("\nC Kernel Features:\n");
    terminal_write("- VGA Text Mode\n");
    terminal_write("- Color Support\n");
    terminal_write("- String Output\n");
    terminal_write("- 64-bit Compatible\n");
    
    terminal_setcolor(vga_entry_color(COLOR_LIGHT_MAGENTA, COLOR_BLACK));
    terminal_write("\nJingOS C Kernel is running!\n");
    
    terminal_setcolor(vga_entry_color(COLOR_WHITE, COLOR_BLACK));
    terminal_write("Press Ctrl+C to exit QEMU\n");
    
    // 无限循环
    while (1) {
        __asm__ volatile ("hlt");
    }
}
