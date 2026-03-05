use core::arch::asm;
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};

use spin::Mutex;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::structures::paging::{OffsetPageTable, PageTableFlags};

use crate::gdt;
use crate::memory::{self, BootInfoFrameAllocator};
use crate::syscall;

pub const USERMODE_EXIT_INTERRUPT_VECTOR: u8 = 0x81;
pub const USER_CODE_START: u64 = 0x0000_6666_0000_0000;
pub const USER_STACK_START: u64 = 0x0000_6666_0000_8000;
pub const USER_STACK_TOP: u64 = USER_STACK_START + 0x2000;

const USER_FAST_SYSCALL_UNKNOWN_NUMBER: u32 = 0xFFFF_FF00;
const USER_CODE_MAX_BYTES: usize = 256;

#[derive(Debug, Clone, Copy)]
struct UserModeContext {
    entry: u64,
    stack_top: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FastSyscallDemoKind {
    SuccessPath,
    ErrorPath,
}

#[derive(Debug, Clone, Copy)]
pub struct FastSyscallDemoReport {
    kind: FastSyscallDemoKind,
    pub call0_value: u64,
    pub call0_status: u64,
    pub call1_value: u64,
    pub call1_status: u64,
}

impl FastSyscallDemoReport {
    pub fn kind_name(self) -> &'static str {
        match self.kind {
            FastSyscallDemoKind::SuccessPath => "success-path",
            FastSyscallDemoKind::ErrorPath => "error-path",
        }
    }

    pub fn expected_summary(self) -> &'static str {
        match self.kind {
            FastSyscallDemoKind::SuccessPath => {
                "call0 status=0; call1 value=42 status=0 (SYSCALL_ADD 40+2)"
            }
            FastSyscallDemoKind::ErrorPath => {
                "call0 value=0xffff_ff00 status=1 (unknown syscall); call1 value=0 status=2 (overflow)"
            }
        }
    }

    pub fn passed(self) -> bool {
        match self.kind {
            FastSyscallDemoKind::SuccessPath => {
                self.call0_status == syscall::SYSCALL_STATUS_OK
                    && self.call1_value == 42
                    && self.call1_status == syscall::SYSCALL_STATUS_OK
            }
            FastSyscallDemoKind::ErrorPath => {
                self.call0_value == USER_FAST_SYSCALL_UNKNOWN_NUMBER as u64
                    && self.call0_status == syscall::SYSCALL_STATUS_UNKNOWN_NUMBER
                    && self.call1_value == 0
                    && self.call1_status == syscall::SYSCALL_STATUS_OVERFLOW
            }
        }
    }
}

struct UserCode {
    bytes: [u8; USER_CODE_MAX_BYTES],
    len: usize,
}

impl UserCode {
    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

struct UserCodeBuilder {
    bytes: [u8; USER_CODE_MAX_BYTES],
    len: usize,
}

impl UserCodeBuilder {
    const fn new() -> Self {
        Self {
            bytes: [0; USER_CODE_MAX_BYTES],
            len: 0,
        }
    }

    fn finish(self) -> UserCode {
        UserCode {
            bytes: self.bytes,
            len: self.len,
        }
    }

    fn emit_byte(&mut self, byte: u8) -> Result<(), &'static str> {
        if self.len >= self.bytes.len() {
            return Err("user mode demo code exceeded max length");
        }

        self.bytes[self.len] = byte;
        self.len += 1;
        Ok(())
    }

    fn emit_bytes(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        if self.len + bytes.len() > self.bytes.len() {
            return Err("user mode demo code exceeded max length");
        }

        let end = self.len + bytes.len();
        self.bytes[self.len..end].copy_from_slice(bytes);
        self.len = end;
        Ok(())
    }

    fn emit_mov_eax_imm32(&mut self, value: u32) -> Result<(), &'static str> {
        self.emit_byte(0xB8)?;
        self.emit_bytes(&value.to_le_bytes())
    }

    fn emit_mov_rdi_imm64(&mut self, value: u64) -> Result<(), &'static str> {
        self.emit_bytes(&[0x48, 0xBF])?;
        self.emit_bytes(&value.to_le_bytes())
    }

    fn emit_mov_rsi_imm64(&mut self, value: u64) -> Result<(), &'static str> {
        self.emit_bytes(&[0x48, 0xBE])?;
        self.emit_bytes(&value.to_le_bytes())
    }

    fn emit_mov_rdx_imm64(&mut self, value: u64) -> Result<(), &'static str> {
        self.emit_bytes(&[0x48, 0xBA])?;
        self.emit_bytes(&value.to_le_bytes())
    }

    fn emit_syscall(&mut self) -> Result<(), &'static str> {
        self.emit_bytes(&[0x0F, 0x05])
    }

    fn emit_mov_r8_rax(&mut self) -> Result<(), &'static str> {
        self.emit_bytes(&[0x49, 0x89, 0xC0])
    }

    fn emit_mov_r9_r10(&mut self) -> Result<(), &'static str> {
        self.emit_bytes(&[0x4D, 0x89, 0xD1])
    }

    fn emit_mov_r12_r10(&mut self) -> Result<(), &'static str> {
        self.emit_bytes(&[0x4D, 0x89, 0xD4])
    }

    fn emit_mov_r10_rax(&mut self) -> Result<(), &'static str> {
        self.emit_bytes(&[0x49, 0x89, 0xC2])
    }

    fn emit_interrupt(&mut self, vector: u8) -> Result<(), &'static str> {
        self.emit_bytes(&[0xCD, vector])
    }

    fn emit_hang_loop(&mut self) -> Result<(), &'static str> {
        self.emit_bytes(&[0xEB, 0xFE])
    }
}

static USER_MODE_CONTEXT: Mutex<Option<UserModeContext>> = Mutex::new(None);
static LAST_FAST_SYSCALL_REPORT: Mutex<Option<FastSyscallDemoReport>> = Mutex::new(None);
static PENDING_FAST_SYSCALL_KIND: Mutex<Option<FastSyscallDemoKind>> = Mutex::new(None);
static USERMODE_EXIT_EXPECTED: AtomicBool = AtomicBool::new(false);

const USER_INT_DEMO_CODE: [u8; 11] = [
    0xB8, 0x00, 0x00, 0x00, 0x00, // mov eax, 0      (SYSCALL_GET_TICKS)
    0xCD, 0x80, // int 0x80         (syscall interrupt)
    0xCD, 0x81, // int 0x81         (return to monitor)
    0xEB, 0xFE, // jmp $            (should not reach)
];

pub fn init_memory(
    mapper: &mut OffsetPageTable<'static>,
    frame_allocator: &mut BootInfoFrameAllocator,
) -> Result<(), &'static str> {
    let user_code_flags =
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
    let user_stack_flags =
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;

    memory::map_page_with_flags(mapper, frame_allocator, USER_CODE_START, user_code_flags)?;
    memory::map_page_with_flags(mapper, frame_allocator, USER_STACK_START, user_stack_flags)?;
    memory::map_page_with_flags(
        mapper,
        frame_allocator,
        USER_STACK_START + 0x1000,
        user_stack_flags,
    )?;

    write_demo_code(&USER_INT_DEMO_CODE);

    let mut context = USER_MODE_CONTEXT.lock();
    *context = Some(UserModeContext {
        entry: USER_CODE_START,
        stack_top: USER_STACK_TOP,
    });
    Ok(())
}

pub fn run_demo() -> Result<(), &'static str> {
    run_demo_code(&USER_INT_DEMO_CODE, None)
}

pub fn run_fast_syscall_demo() -> Result<(), &'static str> {
    let code = build_fast_syscall_success_code()?;
    run_demo_code(code.as_slice(), Some(FastSyscallDemoKind::SuccessPath))
}

pub fn run_fast_syscall_error_demo() -> Result<(), &'static str> {
    let code = build_fast_syscall_error_code()?;
    run_demo_code(code.as_slice(), Some(FastSyscallDemoKind::ErrorPath))
}

pub fn take_last_fast_syscall_report() -> Option<FastSyscallDemoReport> {
    LAST_FAST_SYSCALL_REPORT.lock().take()
}

fn run_demo_code(
    code: &[u8],
    fast_syscall_kind: Option<FastSyscallDemoKind>,
) -> Result<(), &'static str> {
    let context = match *USER_MODE_CONTEXT.lock() {
        Some(context) => context,
        None => return Err("user mode memory not initialized"),
    };

    *PENDING_FAST_SYSCALL_KIND.lock() = fast_syscall_kind;
    *LAST_FAST_SYSCALL_REPORT.lock() = None;

    write_demo_code(code);
    USERMODE_EXIT_EXPECTED.store(true, Ordering::Release);

    let selectors = gdt::selectors();
    unsafe {
        enter_user_mode(
            context.entry,
            context.stack_top,
            selectors.user_code_selector,
            selectors.user_data_selector,
        );
    }
}

fn build_fast_syscall_success_code() -> Result<UserCode, &'static str> {
    let mut builder = UserCodeBuilder::new();

    emit_fast_syscall_capture_slot0(&mut builder, syscall::SYSCALL_GET_TICKS as u32, 0, 0, 0)?;
    emit_fast_syscall_capture_slot1(&mut builder, syscall::SYSCALL_ADD as u32, 40, 2, 0)?;

    builder.emit_interrupt(USERMODE_EXIT_INTERRUPT_VECTOR)?;
    builder.emit_hang_loop()?;

    Ok(builder.finish())
}

fn build_fast_syscall_error_code() -> Result<UserCode, &'static str> {
    let mut builder = UserCodeBuilder::new();

    emit_fast_syscall_capture_slot0(&mut builder, USER_FAST_SYSCALL_UNKNOWN_NUMBER, 0, 0, 0)?;
    emit_fast_syscall_capture_slot1(&mut builder, syscall::SYSCALL_ADD as u32, u64::MAX, 1, 0)?;

    builder.emit_interrupt(USERMODE_EXIT_INTERRUPT_VECTOR)?;
    builder.emit_hang_loop()?;

    Ok(builder.finish())
}

fn emit_fast_syscall_capture_slot0(
    builder: &mut UserCodeBuilder,
    number: u32,
    arg0: u64,
    arg1: u64,
    arg2: u64,
) -> Result<(), &'static str> {
    builder.emit_mov_eax_imm32(number)?;
    builder.emit_mov_rdi_imm64(arg0)?;
    builder.emit_mov_rsi_imm64(arg1)?;
    builder.emit_mov_rdx_imm64(arg2)?;
    builder.emit_syscall()?;
    builder.emit_mov_r8_rax()?;
    builder.emit_mov_r9_r10()?;
    Ok(())
}

fn emit_fast_syscall_capture_slot1(
    builder: &mut UserCodeBuilder,
    number: u32,
    arg0: u64,
    arg1: u64,
    arg2: u64,
) -> Result<(), &'static str> {
    builder.emit_mov_eax_imm32(number)?;
    builder.emit_mov_rdi_imm64(arg0)?;
    builder.emit_mov_rsi_imm64(arg1)?;
    builder.emit_mov_rdx_imm64(arg2)?;
    builder.emit_syscall()?;
    builder.emit_mov_r12_r10()?;
    builder.emit_mov_r10_rax()?;
    Ok(())
}

fn write_demo_code(code: &[u8]) {
    let destination = USER_CODE_START as *mut u8;
    unsafe {
        for index in 0..4096 {
            destination.add(index).write_volatile(0x90);
        }

        for (index, byte) in code.iter().copied().enumerate() {
            destination.add(index).write_volatile(byte);
        }
    }
}

pub fn handle_exit_interrupt_from_registers(registers: &mut syscall::SyscallRegisters) {
    if !USERMODE_EXIT_EXPECTED.swap(false, Ordering::AcqRel) {
        let mut port = crate::serial_port();
        let _ = writeln!(port, "ignored unexpected int 0x81 (no usermode session)");
        return;
    }

    let pending_kind = PENDING_FAST_SYSCALL_KIND.lock().take();
    if let Some(kind) = pending_kind {
        *LAST_FAST_SYSCALL_REPORT.lock() = Some(FastSyscallDemoReport {
            kind,
            call0_value: registers.r8,
            call0_status: registers.r9,
            call1_value: registers.r10,
            call1_status: registers.r12,
        });
    } else {
        *LAST_FAST_SYSCALL_REPORT.lock() = None;
    }

    crate::resume_monitor_after_usermode_exit()
}

unsafe fn enter_user_mode(
    user_entry: u64,
    user_stack_top: u64,
    user_code_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
) -> ! {
    unsafe {
        asm!(
            "mov ds, {user_data:x}",
            "mov es, {user_data:x}",
            "mov fs, {user_data:x}",
            "mov gs, {user_data:x}",
            "push {user_ss}",
            "push {user_rsp}",
            "pushfq",
            "push {user_cs}",
            "push {user_rip}",
            "iretq",
            user_data = in(reg) u64::from(user_data_selector.0),
            user_ss = in(reg) u64::from(user_data_selector.0),
            user_rsp = in(reg) user_stack_top,
            user_cs = in(reg) u64::from(user_code_selector.0),
            user_rip = in(reg) user_entry,
            options(noreturn)
        );
    }
}
