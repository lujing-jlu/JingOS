use core::sync::atomic::{AtomicU64, Ordering};
use core::fmt::Write;
use pic8259::ChainedPics;
use spin::{Lazy, Mutex};
use x86_64::registers::control::Cr2;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

const PIT_INPUT_HZ: u32 = 1_193_182;
const PIT_MIN_HZ: u32 = 19;
const PIT_MAX_HZ: u32 = PIT_INPUT_HZ;
const PIT_COMMAND_PORT: u16 = 0x43;
const PIT_CHANNEL0_PORT: u16 = 0x40;

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut table = InterruptDescriptorTable::new();
    table.breakpoint.set_handler_fn(breakpoint_handler);
    table.page_fault.set_handler_fn(page_fault_handler);
    table[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
    table[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);
    table
});

static TICKS: AtomicU64 = AtomicU64::new(0);
static KEYBOARD_IRQS: AtomicU64 = AtomicU64::new(0);
const SCANCODE_BUFFER_SIZE: usize = 128;
static SCANCODE_BUFFER: Mutex<ScancodeBuffer> = Mutex::new(ScancodeBuffer::new());

static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

struct ScancodeBuffer {
    entries: [u8; SCANCODE_BUFFER_SIZE],
    read_index: usize,
    write_index: usize,
    len: usize,
    dropped: u64,
}

impl ScancodeBuffer {
    const fn new() -> Self {
        Self {
            entries: [0; SCANCODE_BUFFER_SIZE],
            read_index: 0,
            write_index: 0,
            len: 0,
            dropped: 0,
        }
    }

    fn push(&mut self, scancode: u8) {
        if self.len == SCANCODE_BUFFER_SIZE {
            self.dropped += 1;
            return;
        }

        self.entries[self.write_index] = scancode;
        self.write_index = (self.write_index + 1) % SCANCODE_BUFFER_SIZE;
        self.len += 1;
    }

    fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }

        let scancode = self.entries[self.read_index];
        self.read_index = (self.read_index + 1) % SCANCODE_BUFFER_SIZE;
        self.len -= 1;
        Some(scancode)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

pub fn init() {
    IDT.load();

    x86_64::instructions::interrupts::without_interrupts(|| unsafe {
        let mut pics = PICS.lock();
        pics.initialize();
        pics.write_masks(0b1111_1100, 0b1111_1111);
    });

    program_pit(100);
    x86_64::instructions::interrupts::enable();
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

pub fn pop_scancode() -> Option<u8> {
    x86_64::instructions::interrupts::without_interrupts(|| SCANCODE_BUFFER.lock().pop())
}

pub fn keyboard_counters() -> (u64, u64) {
    let count = KEYBOARD_IRQS.load(Ordering::Relaxed);
    let dropped = x86_64::instructions::interrupts::without_interrupts(|| SCANCODE_BUFFER.lock().dropped);
    (count, dropped)
}

fn program_pit(requested_hz: u32) {
    let hz = requested_hz.clamp(PIT_MIN_HZ, PIT_MAX_HZ);
    let divisor = (PIT_INPUT_HZ / hz).clamp(1, u16::MAX as u32) as u16;

    unsafe {
        let mut command: Port<u8> = Port::new(PIT_COMMAND_PORT);
        let mut channel0: Port<u8> = Port::new(PIT_CHANNEL0_PORT);

        command.write(0x36);
        channel0.write((divisor & 0x00ff) as u8);
        channel0.write((divisor >> 8) as u8);
    }
}

extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
    crate::println!("EXCEPTION: BREAKPOINT");
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let accessed_address = Cr2::read();
    let mut port = crate::serial_port();
    let _ = writeln!(port, "EXCEPTION: PAGE FAULT");
    let _ = writeln!(port, "  Accessed Address: {accessed_address:?}");
    let _ = writeln!(port, "  Error Code: {error_code:?}");
    let _ = writeln!(port, "  Stack Frame: {stack_frame:#?}");
    crate::println!("EXCEPTION: PAGE FAULT @ {accessed_address:?} ({error_code:?})");
    crate::exit_qemu(crate::QemuExitCode::Failed);
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    TICKS.fetch_add(1, Ordering::Relaxed);
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port: Port<u8> = Port::new(0x60);
    let scancode = unsafe { port.read() };
    KEYBOARD_IRQS.fetch_add(1, Ordering::Relaxed);
    SCANCODE_BUFFER.lock().push(scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
