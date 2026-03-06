use core::arch::asm;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::__cpuid;
use core::ptr::{addr_of, addr_of_mut};

use spin::Mutex;
use x86_64::VirtAddr;
use x86_64::registers::model_specific::{Efer, EferFlags, KernelGsBase, LStar, SFMask, Star};
use x86_64::registers::rflags::RFlags;

use crate::gdt::Selectors;
use crate::memory;

pub const SYSCALL_INTERRUPT_VECTOR: u8 = 0x80;

pub const SYSCALL_GET_TICKS: u64 = 0;
pub const SYSCALL_GET_UPTIME_SECONDS: u64 = 1;
pub const SYSCALL_ADD: u64 = 2;
pub const SYSCALL_GET_USABLE_MEMORY: u64 = 3;
pub const SYSCALL_GET_USABLE_FRAMES: u64 = 4;

pub const SYSCALL_STATUS_OK: u64 = 0;
pub const SYSCALL_STATUS_UNKNOWN_NUMBER: u64 = 1;
pub const SYSCALL_STATUS_OVERFLOW: u64 = 2;
pub const SYSCALL_STATUS_CONTEXT_UNAVAILABLE: u64 = 3;

const FAST_SYSCALL_STACK_SIZE: usize = 4096 * 4;
const ENABLE_FAST_SYSCALL_SCE: bool = true;

static mut FAST_SYSCALL_STACK: [u8; FAST_SYSCALL_STACK_SIZE] = [0; FAST_SYSCALL_STACK_SIZE];

#[repr(C)]
struct FastSyscallCpuLocal {
    kernel_rsp: u64,
    user_rsp: u64,
}

static mut FAST_SYSCALL_CPU_LOCAL: FastSyscallCpuLocal = FastSyscallCpuLocal {
    kernel_rsp: 0,
    user_rsp: 0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallAbiMode {
    InterruptRegistersV2,
}

pub fn abi_mode() -> SyscallAbiMode {
    SyscallAbiMode::InterruptRegistersV2
}

pub fn abi_mode_name() -> &'static str {
    match abi_mode() {
        SyscallAbiMode::InterruptRegistersV2 => "interrupt-register-v2",
    }
}

pub fn abi_mode_details() -> &'static str {
    match abi_mode() {
        SyscallAbiMode::InterruptRegistersV2 => {
            "int 0x80 + rax(number), rdi/rsi/rdx(args), rax(return), rcx(status)"
        }
    }
}

pub fn fast_abi_mode_details() -> &'static str {
    "syscall/sysret + rax(number), rdi/rsi/rdx(args), rax(return), r10(status)"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastSyscallStage {
    Uninitialized,
    UnsupportedCpu,
    Scaffolded,
    MsrProgrammedNoSce,
    SceEnabledExperimental,
}

pub fn fast_syscall_stage_name(stage: FastSyscallStage) -> &'static str {
    match stage {
        FastSyscallStage::Uninitialized => "uninitialized",
        FastSyscallStage::UnsupportedCpu => "unsupported-cpu",
        FastSyscallStage::Scaffolded => "scaffolded-not-enabled",
        FastSyscallStage::MsrProgrammedNoSce => "msr-programmed-no-sce",
        FastSyscallStage::SceEnabledExperimental => "sce-enabled-experimental",
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FastSyscallSelectorPlan {
    pub kernel_cs: u16,
    pub kernel_ss: u16,
    pub user_cs: u16,
    pub user_ss: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct FastSyscallStatus {
    pub stage: FastSyscallStage,
    pub cpu_support: bool,
    pub sce_enabled: bool,
    pub selectors: Option<FastSyscallSelectorPlan>,
    pub lstar: Option<u64>,
    pub kernel_gs_base: Option<u64>,
    pub note: &'static str,
}

const DEFAULT_FAST_SYSCALL_STATUS: FastSyscallStatus = FastSyscallStatus {
    stage: FastSyscallStage::Uninitialized,
    cpu_support: false,
    sce_enabled: false,
    selectors: None,
    lstar: None,
    kernel_gs_base: None,
    note: "not initialized",
};

static FAST_SYSCALL_STATUS: Mutex<FastSyscallStatus> = Mutex::new(DEFAULT_FAST_SYSCALL_STATUS);

pub fn init_fast_syscall_scaffold(selectors: Selectors) {
    let cpu_support = cpu_supports_syscall();
    let sce_before = Efer::read().contains(EferFlags::SYSTEM_CALL_EXTENSIONS);
    let mut status = FAST_SYSCALL_STATUS.lock();

    if !cpu_support {
        *status = FastSyscallStatus {
            stage: FastSyscallStage::UnsupportedCpu,
            cpu_support: false,
            sce_enabled: sce_before,
            selectors: None,
            lstar: None,
            kernel_gs_base: None,
            note: "cpu does not report SYSCALL/SYSRET support",
        };
        return;
    }

    let (plan, lstar, kernel_gs_base) = match program_fast_syscall_msrs(selectors) {
        Ok(values) => values,
        Err(note) => {
            *status = FastSyscallStatus {
                stage: FastSyscallStage::Scaffolded,
                cpu_support: true,
                sce_enabled: sce_before,
                selectors: Some(FastSyscallSelectorPlan {
                    kernel_cs: selectors.kernel_code_selector.0,
                    kernel_ss: selectors.kernel_data_selector.0,
                    user_cs: selectors.user_code_selector.0,
                    user_ss: selectors.user_data_selector.0,
                }),
                lstar: None,
                kernel_gs_base: None,
                note,
            };
            return;
        }
    };

    if ENABLE_FAST_SYSCALL_SCE {
        unsafe {
            Efer::update(|flags| flags.insert(EferFlags::SYSTEM_CALL_EXTENSIONS));
        }
    }

    let sce_after = Efer::read().contains(EferFlags::SYSTEM_CALL_EXTENSIONS);
    if sce_after {
        *status = FastSyscallStatus {
            stage: FastSyscallStage::SceEnabledExperimental,
            cpu_support: true,
            sce_enabled: true,
            selectors: Some(plan),
            lstar: Some(lstar),
            kernel_gs_base: Some(kernel_gs_base),
            note: "SCE enabled (experimental); int 0x80 remains default monitor ABI while syscall path is validated",
        };
    } else {
        *status = FastSyscallStatus {
            stage: FastSyscallStage::MsrProgrammedNoSce,
            cpu_support: true,
            sce_enabled: false,
            selectors: Some(plan),
            lstar: Some(lstar),
            kernel_gs_base: Some(kernel_gs_base),
            note: "STAR/LSTAR/SFMASK programmed; SCE not enabled (CPU/firmware policy)",
        };
    }
}

fn program_fast_syscall_msrs(
    selectors: Selectors,
) -> Result<(FastSyscallSelectorPlan, u64, u64), &'static str> {
    let plan = FastSyscallSelectorPlan {
        kernel_cs: selectors.kernel_code_selector.0,
        kernel_ss: selectors.kernel_data_selector.0,
        user_cs: selectors.user_code_selector.0,
        user_ss: selectors.user_data_selector.0,
    };

    Star::write(
        selectors.user_code_selector,
        selectors.user_data_selector,
        selectors.kernel_code_selector,
        selectors.kernel_data_selector,
    )
    .map_err(|_| "STAR selector layout rejected by x86_64 crate")?;

    let lstar = crate::interrupts::fast_syscall_entry_address();
    LStar::write(lstar);
    SFMask::write(RFlags::INTERRUPT_FLAG | RFlags::TRAP_FLAG | RFlags::DIRECTION_FLAG);

    unsafe {
        let stack_start = VirtAddr::from_ptr(addr_of!(FAST_SYSCALL_STACK) as *const u8);
        let stack_top = (stack_start + FAST_SYSCALL_STACK_SIZE as u64).as_u64() & !0xf;

        let cpu_local_ptr = addr_of_mut!(FAST_SYSCALL_CPU_LOCAL);
        (*cpu_local_ptr).kernel_rsp = stack_top;
        (*cpu_local_ptr).user_rsp = 0;

        let kernel_gs_base = VirtAddr::from_ptr(cpu_local_ptr as *const FastSyscallCpuLocal);
        KernelGsBase::write(kernel_gs_base);
        Ok((plan, lstar.as_u64(), kernel_gs_base.as_u64()))
    }
}

pub fn fast_syscall_status() -> FastSyscallStatus {
    *FAST_SYSCALL_STATUS.lock()
}

fn cpu_supports_syscall() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        let max_extended = __cpuid(0x8000_0000).eax;
        if max_extended < 0x8000_0001 {
            return false;
        }

        let extended_features = __cpuid(0x8000_0001);
        (extended_features.edx & (1 << 11)) != 0
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SyscallRegisters {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallError {
    UnknownNumber(u64),
    Overflow,
    ContextUnavailable,
}

pub fn dispatch(number: u64, arg0: u64, arg1: u64, _arg2: u64) -> Result<u64, SyscallError> {
    match number {
        SYSCALL_GET_TICKS => Ok(crate::interrupts::ticks()),
        SYSCALL_GET_UPTIME_SECONDS => Ok(crate::interrupts::ticks() / crate::interrupts::pit_hz() as u64),
        SYSCALL_ADD => arg0.checked_add(arg1).ok_or(SyscallError::Overflow),
        SYSCALL_GET_USABLE_MEMORY => {
            let Some(summary) = memory::kernel_memory_summary() else {
                return Err(SyscallError::ContextUnavailable);
            };
            Ok(summary.usable_bytes)
        }
        SYSCALL_GET_USABLE_FRAMES => {
            let Some(summary) = memory::kernel_memory_summary() else {
                return Err(SyscallError::ContextUnavailable);
            };
            Ok(summary.usable_frames_4k as u64)
        }
        _ => Err(SyscallError::UnknownNumber(number)),
    }
}

fn encode_error(error: SyscallError) -> (u64, u64) {
    match error {
        SyscallError::UnknownNumber(number) => (number, SYSCALL_STATUS_UNKNOWN_NUMBER),
        SyscallError::Overflow => (0, SYSCALL_STATUS_OVERFLOW),
        SyscallError::ContextUnavailable => (0, SYSCALL_STATUS_CONTEXT_UNAVAILABLE),
    }
}

fn decode_response(status: u64, value: u64) -> Result<u64, SyscallError> {
    match status {
        SYSCALL_STATUS_OK => Ok(value),
        SYSCALL_STATUS_UNKNOWN_NUMBER => Err(SyscallError::UnknownNumber(value)),
        SYSCALL_STATUS_OVERFLOW => Err(SyscallError::Overflow),
        SYSCALL_STATUS_CONTEXT_UNAVAILABLE => Err(SyscallError::ContextUnavailable),
        _ => Err(SyscallError::ContextUnavailable),
    }
}

pub fn invoke_via_interrupt(
    number: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
) -> Result<u64, SyscallError> {
    let result_value: u64;
    let status: u64;

    unsafe {
        asm!(
            "int {vector}",
            vector = const SYSCALL_INTERRUPT_VECTOR,
            inlateout("rax") number => result_value,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            lateout("rcx") status,
            options(nostack)
        );
    }

    decode_response(status, result_value)
}

pub fn handle_interrupt_from_registers(registers: &mut SyscallRegisters) {
    let number = registers.rax;
    let arg0 = registers.rdi;
    let arg1 = registers.rsi;
    let arg2 = registers.rdx;

    match dispatch(number, arg0, arg1, arg2) {
        Ok(value) => {
            registers.rax = value;
            registers.rcx = SYSCALL_STATUS_OK;
        }
        Err(error) => {
            let (value, status) = encode_error(error);
            registers.rax = value;
            registers.rcx = status;
        }
    }
}

pub fn syscall_name(number: u64) -> &'static str {
    match number {
        SYSCALL_GET_TICKS => "get_ticks",
        SYSCALL_GET_UPTIME_SECONDS => "get_uptime_seconds",
        SYSCALL_ADD => "add",
        SYSCALL_GET_USABLE_MEMORY => "get_usable_memory",
        SYSCALL_GET_USABLE_FRAMES => "get_usable_frames",
        _ => "unknown",
    }
}

pub fn error_name(error: SyscallError) -> &'static str {
    match error {
        SyscallError::UnknownNumber(_) => "unknown syscall number",
        SyscallError::Overflow => "arithmetic overflow",
        SyscallError::ContextUnavailable => "syscall context unavailable",
    }
}
