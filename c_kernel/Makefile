# JingOS C内核 Makefile

# 编译器设置 (使用GNU工具链)
CC = x86_64-elf-gcc
AS = nasm
LD = x86_64-elf-ld

# 编译标志
CFLAGS = -ffreestanding \
         -fno-stack-protector \
         -fno-stack-check \
         -mno-80387 \
         -mno-mmx \
         -mno-3dnow \
         -mno-sse \
         -mno-sse2 \
         -mno-red-zone \
         -mcmodel=kernel \
         -Wall \
         -Wextra \
         -O2

# 汇编标志
ASFLAGS = -f elf64

# 链接标志
LDFLAGS = -T linker.ld \
          -static \
          -Bsymbolic \
          -nostdlib

# 目标文件
OBJS = boot.o kernel.o

# 默认目标
all: c_kernel.bin

# 编译C文件
kernel.o: kernel.c
	$(CC) $(CFLAGS) -c kernel.c -o kernel.o

# 编译汇编文件
boot.o: boot.s
	$(AS) $(ASFLAGS) boot.s -o boot.o

# 链接生成ELF文件
c_kernel.elf: $(OBJS) linker.ld
	$(LD) $(LDFLAGS) $(OBJS) -o c_kernel.elf

# 转换为二进制文件
c_kernel.bin: c_kernel.elf
	x86_64-elf-objcopy -O binary c_kernel.elf c_kernel.bin

# 清理
clean:
	rm -f *.o *.elf *.bin

# 显示信息
info: c_kernel.elf
	file c_kernel.elf
	objdump -f c_kernel.elf
	size c_kernel.elf

.PHONY: all clean info
