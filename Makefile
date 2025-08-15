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

# 构建Rust内核版本
rust-kernel: bootloader kernel
	@echo "创建Rust内核版本磁盘镜像..."
	# 将ELF格式的简单Rust内核转换为纯二进制
	rust-objcopy --binary-architecture=x86_64 target/x86_64-unknown-none/debug/simple-kernel --strip-all -O binary kernel.bin
	# 创建1.44MB软盘镜像
	dd if=/dev/zero of=jingos_rust.img bs=1024 count=1440
	# 写入Stage1到第一个扇区
	dd if=bootloader/stage1.bin of=jingos_rust.img bs=512 count=1 conv=notrunc
	# 写入Rust内核Stage2到第二个扇区开始
	cd bootloader && nasm -f bin src/rust_simple.s -o rust_stage2.bin
	dd if=bootloader/rust_stage2.bin of=jingos_rust.img bs=512 seek=1 conv=notrunc
	# 写入转换后的Rust内核二进制到第10个扇区开始
	dd if=kernel.bin of=jingos_rust.img bs=512 seek=10 conv=notrunc

# 运行Rust内核版本
run-rust: rust-kernel
	@echo "启动 JingOS Rust 内核版本..."
	qemu-system-x86_64 \
		-drive format=raw,file=jingos_rust.img \
		-serial stdio

# 构建C内核版本
c-kernel: bootloader
	@echo "构建 C 风格内核..."
	cd bootloader && nasm -f bin src/c_kernel.s -o c_kernel.bin
	@echo "创建C内核版本磁盘镜像..."
	# 创建1.44MB软盘镜像
	dd if=/dev/zero of=jingos_c.img bs=1024 count=1440
	# 写入Stage1到第一个扇区
	dd if=bootloader/stage1.bin of=jingos_c.img bs=512 count=1 conv=notrunc
	# 写入C内核Stage2到第二个扇区开始
	cd bootloader && nasm -f bin src/rust_simple.s -o c_stage2.bin
	dd if=bootloader/c_stage2.bin of=jingos_c.img bs=512 seek=1 conv=notrunc
	# 写入C风格内核到第10个扇区开始
	dd if=bootloader/c_kernel.bin of=jingos_c.img bs=512 seek=10 conv=notrunc

# 运行C内核版本
run-c: c-kernel
	@echo "启动 JingOS C 内核版本..."
	qemu-system-x86_64 \
		-drive format=raw,file=jingos_c.img \
		-serial stdio

# 显示所有可用的版本
versions:
	@echo "🎉 JingOS - 可用的版本："
	@echo ""
	@echo "1. 📱 原始测试版本:"
	@echo "   make run          # 显示红色'Jing'内核标识"
	@echo ""
	@echo "2. 🔧 简单内核版本:"
	@echo "   make run-rust     # 显示黄色'KERN'测试"
	@echo ""
	@echo "3. 🎨 C风格内核版本:"
	@echo "   make run-c        # 完整的彩色C内核界面"
	@echo ""
	@echo "✅ 所有版本都支持完整的64位长模式启动！"

# 清理
clean:
	@echo "清理构建文件..."
	cargo clean
	rm -f *.img
	rm -f bootloader/*.bin
	cd c_kernel && make clean 2>/dev/null || true

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
