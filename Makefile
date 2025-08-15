# JingOS 构建系统 - 使用自定义bootloader

.PHONY: all build run debug clean bootloader kernel

# 默认目标
all: build

# 构建多阶段bootloader
bootloader:
	@echo "构建多阶段 Bootloader..."
	cd bootloader && nasm -f bin src/stage1.s -o stage1.bin
	cd bootloader && nasm -f bin src/stage2_clean.s -o stage2.bin
	cd bootloader && nasm -f bin src/test_kernel.s -o test_kernel.bin
	# 合并两个阶段
	cd bootloader && cat stage1.bin stage2.bin > bootloader.bin

# 构建内核
kernel:
	@echo "构建 JingOS 内核..."
	cargo build --target x86_64-unknown-none --package jing-kernel

# 创建完整的磁盘镜像
build: bootloader kernel
	@echo "创建磁盘镜像..."
	# 创建1.44MB软盘镜像
	dd if=/dev/zero of=jingos.img bs=1024 count=1440
	# 写入Stage1到第一个扇区
	dd if=bootloader/stage1.bin of=jingos.img bs=512 count=1 conv=notrunc
	# 写入Stage2到第二个扇区开始
	dd if=bootloader/stage2.bin of=jingos.img bs=512 seek=1 conv=notrunc
	# 写入测试内核到第10个扇区开始
	dd if=bootloader/test_kernel.bin of=jingos.img bs=512 seek=32 conv=notrunc

# 在QEMU中运行
run: build
	@echo "在 QEMU 中启动 JingOS..."
	qemu-system-x86_64 \
		-drive format=raw,file=jingos.img \
		-serial stdio

# 调试模式运行
debug: build
	@echo "调试模式启动 JingOS..."
	qemu-system-x86_64 \
		-drive format=raw,file=jingos.img \
		-serial stdio \
		-s -S

# 清理
clean:
	@echo "清理构建文件..."
	cargo clean
	rm -f jingos.img
	rm -f bootloader/bootloader.bin

# 帮助
help:
	@echo "JingOS 构建系统"
	@echo ""
	@echo "可用目标:"
	@echo "  build       - 构建完整系统"
	@echo "  bootloader  - 只构建bootloader"
	@echo "  kernel      - 只构建内核"
	@echo "  run         - 在QEMU中运行"
	@echo "  debug       - 调试模式运行"
	@echo "  clean       - 清理构建文件"
