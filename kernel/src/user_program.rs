use crate::syscall::{
    SYSCALL_ADD, SYSCALL_GET_TICKS, SYSCALL_GET_UPTIME_SECONDS, SYSCALL_GET_USABLE_FRAMES,
    SYSCALL_GET_USABLE_MEMORY, SyscallError, invoke_via_interrupt,
};

#[derive(Debug, Clone, Copy)]
pub struct UserDemoReport {
    pub ticks: u64,
    pub uptime_seconds: u64,
    pub sum: u64,
    pub usable_bytes: u64,
    pub usable_frames: u64,
}

pub fn run_user_demo() -> Result<UserDemoReport, SyscallError> {
    let ticks_value = invoke_via_interrupt(SYSCALL_GET_TICKS, 0, 0, 0)?;
    let uptime_seconds = invoke_via_interrupt(SYSCALL_GET_UPTIME_SECONDS, 0, 0, 0)?;
    let sum = invoke_via_interrupt(SYSCALL_ADD, 40, 2, 0)?;
    let usable_bytes = invoke_via_interrupt(SYSCALL_GET_USABLE_MEMORY, 0, 0, 0)?;
    let usable_frames = invoke_via_interrupt(SYSCALL_GET_USABLE_FRAMES, 0, 0, 0)?;

    Ok(UserDemoReport {
        ticks: ticks_value,
        uptime_seconds,
        sum,
        usable_bytes,
        usable_frames,
    })
}
