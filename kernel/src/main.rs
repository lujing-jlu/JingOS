#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]

extern crate alloc;

use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::info::MemoryRegions;
use bootloader_api::{BootInfo, entry_point};
use core::alloc::Layout;
use core::fmt::Write;
use keyboard::{KeyEvent, KeyboardDecoder};
use spin::Mutex;
use x86_64::VirtAddr;
use x86_64::instructions::port::Port;
use x86_64::structures::paging::FrameAllocator;

mod framebuffer;
mod gdt;
mod heap;
mod interrupts;
mod keyboard;
mod memory;
mod syscall;
mod task;
mod user_program;
mod usermode;

const BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

macro_rules! console_print {
    ($port:expr, $($arg:tt)*) => {{
        let _ = write!($port, $($arg)*);
        print!($($arg)*);
    }};
}

macro_rules! console_println {
    ($port:expr) => {{
        let _ = writeln!($port);
        println!();
    }};
    ($port:expr, $($arg:tt)*) => {{
        let _ = writeln!($port, $($arg)*);
        println!($($arg)*);
    }};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    use x86_64::instructions::{nop, port::Port};

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }

    loop {
        nop();
    }
}

pub fn serial_port() -> uart_16550::SerialPort {
    let mut port = unsafe { uart_16550::SerialPort::new(0x3F8) };
    port.init();
    port
}

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

#[derive(Clone, Copy)]
struct MonitorContext {
    memory_regions: &'static MemoryRegions,
    memory_summary: memory::MemorySummary,
    physical_memory_offset: Option<u64>,
}

static MONITOR_CONTEXT: Mutex<Option<MonitorContext>> = Mutex::new(None);

fn set_monitor_context(
    memory_regions: &'static MemoryRegions,
    memory_summary: memory::MemorySummary,
    physical_memory_offset: Option<u64>,
) {
    let mut context = MONITOR_CONTEXT.lock();
    *context = Some(MonitorContext {
        memory_regions,
        memory_summary,
        physical_memory_offset,
    });
}

pub fn resume_monitor_after_usermode_exit() -> ! {
    let context = match *MONITOR_CONTEXT.lock() {
        Some(context) => context,
        None => {
            let mut port = serial_port();
            let _ = writeln!(
                port,
                "usermode exit requested, but monitor context is missing"
            );
            exit_qemu(QemuExitCode::Failed);
        }
    };

    let mut port = serial_port();
    console_println!(port, "returned from ring3 via int 0x81; resuming monitor");

    if let Some(report) = usermode::take_last_fast_syscall_report() {
        if report.passed() {
            console_println!(
                port,
                "fast-syscall report ({}): ok call0={} (status={}) call1={} (status={})",
                report.kind_name(),
                report.call0_value,
                report.call0_status,
                report.call1_value,
                report.call1_status
            );
        } else {
            console_println!(
                port,
                "fast-syscall report ({}): unexpected call0={} (status={}) call1={} (status={})",
                report.kind_name(),
                report.call0_value,
                report.call0_status,
                report.call1_value,
                report.call1_status
            );
            console_println!(port, "fast-syscall expected: {}", report.expected_summary());
        }
    }

    x86_64::instructions::interrupts::enable();
    input_loop(
        &mut port,
        context.memory_regions,
        context.memory_summary,
        context.physical_memory_offset,
    )
}

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    let mut port = serial_port();
    writeln!(port, "jingOS kernel started: boot info = {boot_info:?}").unwrap();
    writeln!(port, "Hello from Rust kernel!").unwrap();

    let memory_summary = memory::summarize_memory(&boot_info.memory_regions);
    memory::set_kernel_memory_summary(memory_summary);
    let mut frame_allocator =
        unsafe { memory::BootInfoFrameAllocator::init(&boot_info.memory_regions) };
    let first_frames = [
        frame_allocator.allocate_frame(),
        frame_allocator.allocate_frame(),
        frame_allocator.allocate_frame(),
    ];
    let physical_memory_offset = boot_info.physical_memory_offset.into_option();
    let paging_probe = if let Some(offset) = physical_memory_offset {
        let mut mapper = unsafe { memory::init_offset_page_table(VirtAddr::new(offset)) };
        let paging_result = memory::map_demo_page(&mut mapper, &mut frame_allocator);
        let heap_result = heap::init_heap(&mut mapper, &mut frame_allocator)
            .map_err(|_| "heap init mapping failed")
            .map(|()| heap::probe_allocations());
        let usermode_result =
            usermode::init_memory(&mut mapper, &mut frame_allocator).map_err(|error| error);
        Some((paging_result, heap_result, usermode_result))
    } else {
        None
    };
    let allocated_frames = frame_allocator.allocated_frames();
    let remaining_frames = frame_allocator.remaining_usable_frames_estimate();

    writeln!(
        port,
        "Memory: regions={} usable_regions={} usable_bytes={} reserved_bytes={} largest_usable={}",
        memory_summary.total_regions,
        memory_summary.usable_regions,
        memory_summary.usable_bytes,
        memory_summary.reserved_bytes,
        memory_summary.largest_usable_region_bytes
    )
    .unwrap();
    writeln!(
        port,
        "Frame allocator: usable_frames={} allocated={} remaining_est={} first_frames={first_frames:?}",
        memory_summary.usable_frames_4k,
        allocated_frames,
        remaining_frames
    )
    .unwrap();
    writeln!(
        port,
        "Physical memory mapping offset = {:?}",
        physical_memory_offset
    )
    .unwrap();
    match paging_probe {
        Some((Ok(value), _, _)) => {
            writeln!(port, "Paging probe: demo page mapped, value={value:#x}").unwrap();
        }
        Some((Err(error), _, _)) => {
            writeln!(port, "Paging probe failed: {error}").unwrap();
        }
        None => {
            writeln!(port, "Paging probe skipped: no physical memory mapping").unwrap();
        }
    }
    match paging_probe {
        Some((_, Ok((boxed_value, vector_sum)), _)) => {
            writeln!(
                port,
                "Heap probe: Box={boxed_value:#x}, Vec sum={vector_sum}"
            )
            .unwrap();
        }
        Some((_, Err(error), _)) => {
            writeln!(port, "Heap probe failed: {error}").unwrap();
        }
        None => {
            writeln!(port, "Heap probe skipped: no physical memory mapping").unwrap();
        }
    }
    match paging_probe {
        Some((_, _, Ok(()))) => {
            writeln!(port, "User mode memory initialized").unwrap();
        }
        Some((_, _, Err(error))) => {
            writeln!(port, "User mode memory init failed: {error}").unwrap();
        }
        None => {
            writeln!(
                port,
                "User mode memory init skipped: no physical memory mapping"
            )
            .unwrap();
        }
    }

    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        framebuffer::init(framebuffer);
        println!("jingOS framebuffer console ready");
        println!(
            "Memory: regions={} usable={} usable_bytes={} reserved_bytes={}",
            memory_summary.total_regions,
            memory_summary.usable_regions,
            memory_summary.usable_bytes,
            memory_summary.reserved_bytes
        );
        println!(
            "Frames: usable={} allocated={} remaining~={} first={first_frames:?}",
            memory_summary.usable_frames_4k, allocated_frames, remaining_frames
        );
        println!("Phys mem offset = {:?}", physical_memory_offset);
        match paging_probe {
            Some((Ok(value), _, _)) => println!("Paging probe ok: demo value={value:#x}"),
            Some((Err(error), _, _)) => println!("Paging probe failed: {error}"),
            None => println!("Paging probe skipped: no physical memory mapping"),
        }
        match paging_probe {
            Some((_, Ok((boxed_value, vector_sum)), _)) => {
                println!("Heap probe ok: box={boxed_value:#x}, vec_sum={vector_sum}");
            }
            Some((_, Err(error), _)) => println!("Heap probe failed: {error}"),
            None => println!("Heap probe skipped: no physical memory mapping"),
        }
        match paging_probe {
            Some((_, _, Ok(()))) => println!("User mode memory initialized"),
            Some((_, _, Err(error))) => println!("User mode memory init failed: {error}"),
            None => println!("User mode memory init skipped: no physical memory mapping"),
        }
    }

    println!("jingOS kernel started");
    println!("Hello from Rust kernel!");
    gdt::init();
    println!("GDT + TSS initialized");
    syscall::init_fast_syscall_scaffold(gdt::selectors());
    let fast_syscall_status = syscall::fast_syscall_status();
    println!(
        "fast syscall scaffold: stage={} cpu_support={} sce_enabled={}",
        syscall::fast_syscall_stage_name(fast_syscall_status.stage),
        fast_syscall_status.cpu_support,
        fast_syscall_status.sce_enabled
    );
    println!("Initializing IDT + PIC + PIT...");

    interrupts::init();
    task::init();
    set_monitor_context(
        &boot_info.memory_regions,
        memory_summary,
        physical_memory_offset,
    );
    println!("IRQ0(timer) and IRQ1(keyboard) enabled");
    println!("Keyboard input window started (press ESC to exit)");

    match wait_for_ticks(5, 500_000_000) {
        Some(ticks) => {
            let _ = writeln!(port, "Timer interrupts active, observed ticks = {ticks}");
            println!("Timer interrupts active, observed ticks = {ticks}");
            input_loop(
                &mut port,
                &boot_info.memory_regions,
                memory_summary,
                physical_memory_offset,
            );
        }
        None => {
            let _ = writeln!(port, "Timer interrupts not observed within timeout");
            println!("Timer interrupts not observed within timeout");
            exit_qemu(QemuExitCode::Failed);
        }
    }
}

fn wait_for_ticks(target: u64, max_spins: u64) -> Option<u64> {
    for _ in 0..max_spins {
        let ticks = interrupts::ticks();
        if ticks >= target {
            return Some(ticks);
        }
        core::hint::spin_loop();
    }

    None
}

fn poll_serial_byte() -> Option<u8> {
    unsafe {
        let mut line_status: Port<u8> = Port::new(0x3FD);
        if line_status.read() & 0x01 == 0 {
            return None;
        }

        let mut data: Port<u8> = Port::new(0x3F8);
        Some(data.read())
    }
}

fn serial_byte_to_key_event(byte: u8) -> Option<KeyEvent> {
    match byte {
        b'\r' | b'\n' => Some(KeyEvent::Enter),
        0x08 | 0x7F => Some(KeyEvent::Backspace),
        b'\t' => Some(KeyEvent::Tab),
        0x1B => Some(KeyEvent::Escape),
        0x20..=0x7E => Some(KeyEvent::Char(byte as char)),
        _ => None,
    }
}

const INPUT_LINE_MAX: usize = 128;
const HISTORY_CAPACITY: usize = 16;
const MONITOR_PROMPT: &str = "jingos> ";
const MONITOR_READY_MARKER: &str = "[[JINGOS_MONITOR_READY]]";
const COMMAND_NAMES: [&str; 27] = [
    "help",
    "status",
    "ticks",
    "uptime",
    "irq",
    "mem",
    "maps",
    "heap",
    "vm",
    "vmmap",
    "fault",
    "syscall",
    "sysabi",
    "tasks",
    "tasknew",
    "taskstep",
    "taskrun",
    "tasksleep",
    "userdemo",
    "usermode",
    "usermode_syscall",
    "usermode_syscall_fail",
    "echo",
    "history",
    "clear",
    "exit",
    "quit",
];

#[derive(Clone, Copy)]
struct InputLine {
    bytes: [u8; INPUT_LINE_MAX],
    len: usize,
    cursor: usize,
}

impl InputLine {
    const fn new() -> Self {
        Self {
            bytes: [0; INPUT_LINE_MAX],
            len: 0,
            cursor: 0,
        }
    }

    fn insert_char(&mut self, character: char) -> bool {
        if !character.is_ascii() || self.len >= self.bytes.len() {
            return false;
        }

        for index in (self.cursor..self.len).rev() {
            self.bytes[index + 1] = self.bytes[index];
        }
        self.bytes[self.cursor] = character as u8;
        self.len += 1;
        self.cursor += 1;
        true
    }

    fn insert_or_overwrite_char(&mut self, character: char, overwrite_mode: bool) -> bool {
        if !character.is_ascii() {
            return false;
        }

        if overwrite_mode && self.cursor < self.len {
            self.bytes[self.cursor] = character as u8;
            self.cursor += 1;
            return true;
        }

        self.insert_char(character)
    }

    fn append_str(&mut self, text: &str) -> bool {
        if !text.is_ascii() || self.len + text.len() > self.bytes.len() {
            return false;
        }

        let insert_len = text.len();
        for index in (self.cursor..self.len).rev() {
            self.bytes[index + insert_len] = self.bytes[index];
        }

        self.bytes[self.cursor..self.cursor + insert_len].copy_from_slice(text.as_bytes());
        self.len += insert_len;
        self.cursor += insert_len;
        true
    }

    fn set_from_str(&mut self, text: &str) -> bool {
        if !text.is_ascii() || text.len() > self.bytes.len() {
            return false;
        }
        self.clear();
        self.bytes[..text.len()].copy_from_slice(text.as_bytes());
        self.len = text.len();
        self.cursor = self.len;
        true
    }

    fn backspace(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }

        let removed = self.cursor - 1;
        for index in removed + 1..self.len {
            self.bytes[index - 1] = self.bytes[index];
        }
        self.len -= 1;
        self.cursor -= 1;
        true
    }

    fn delete(&mut self) -> bool {
        if self.cursor >= self.len {
            return false;
        }

        for index in self.cursor + 1..self.len {
            self.bytes[index - 1] = self.bytes[index];
        }
        self.len -= 1;
        true
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }

    fn as_prefix_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.cursor]).unwrap_or("")
    }

    fn len(&self) -> usize {
        self.len
    }

    fn is_cursor_at_end(&self) -> bool {
        self.cursor == self.len
    }

    fn move_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        true
    }

    fn move_right(&mut self) -> bool {
        if self.cursor >= self.len {
            return false;
        }
        self.cursor += 1;
        true
    }

    fn move_home(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor = 0;
        true
    }

    fn move_end(&mut self) -> bool {
        if self.cursor == self.len {
            return false;
        }
        self.cursor = self.len;
        true
    }

    fn move_word_left(&mut self) -> bool {
        let previous = self.previous_word_boundary();
        if previous == self.cursor {
            return false;
        }
        self.cursor = previous;
        true
    }

    fn move_word_right(&mut self) -> bool {
        let next = self.next_word_boundary();
        if next == self.cursor {
            return false;
        }
        self.cursor = next;
        true
    }

    fn delete_word_left(&mut self) -> bool {
        let start = self.previous_word_boundary();
        if start == self.cursor {
            return false;
        }

        self.delete_range(start, self.cursor);
        self.cursor = start;
        true
    }

    fn delete_word_right(&mut self) -> bool {
        let end = self.next_word_boundary();
        if end == self.cursor {
            return false;
        }

        self.delete_range(self.cursor, end);
        true
    }

    fn delete_range(&mut self, start: usize, end: usize) {
        let removed = end.saturating_sub(start);
        if removed == 0 {
            return;
        }

        for index in end..self.len {
            self.bytes[index - removed] = self.bytes[index];
        }
        self.len -= removed;
        if self.cursor > self.len {
            self.cursor = self.len;
        }
    }

    fn previous_word_boundary(&self) -> usize {
        if self.cursor == 0 {
            return 0;
        }

        let mut position = self.cursor;
        while position > 0 && self.bytes[position - 1].is_ascii_whitespace() {
            position -= 1;
        }
        while position > 0 && Self::is_word_byte(self.bytes[position - 1]) {
            position -= 1;
        }

        if position == self.cursor {
            while position > 0
                && !self.bytes[position - 1].is_ascii_whitespace()
                && !Self::is_word_byte(self.bytes[position - 1])
            {
                position -= 1;
            }
        }

        if position == self.cursor {
            position.saturating_sub(1)
        } else {
            position
        }
    }

    fn next_word_boundary(&self) -> usize {
        if self.cursor >= self.len {
            return self.len;
        }

        let mut position = self.cursor;
        while position < self.len && self.bytes[position].is_ascii_whitespace() {
            position += 1;
        }
        while position < self.len && Self::is_word_byte(self.bytes[position]) {
            position += 1;
        }

        if position == self.cursor {
            while position < self.len
                && !self.bytes[position].is_ascii_whitespace()
                && !Self::is_word_byte(self.bytes[position])
            {
                position += 1;
            }
        }

        if position == self.cursor {
            (position + 1).min(self.len)
        } else {
            position
        }
    }

    fn is_word_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn clear(&mut self) {
        self.len = 0;
        self.cursor = 0;
    }
}

#[derive(Clone, Copy)]
struct HistoryEntry {
    bytes: [u8; INPUT_LINE_MAX],
    len: usize,
}

impl HistoryEntry {
    const fn empty() -> Self {
        Self {
            bytes: [0; INPUT_LINE_MAX],
            len: 0,
        }
    }

    fn set_from_line(&mut self, line: &InputLine) {
        self.len = line.len;
        self.bytes[..line.len].copy_from_slice(&line.bytes[..line.len]);
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

struct CommandHistory {
    entries: [HistoryEntry; HISTORY_CAPACITY],
    len: usize,
    next_insert: usize,
    browsing_age: Option<usize>,
}

impl CommandHistory {
    const fn new() -> Self {
        Self {
            entries: [HistoryEntry::empty(); HISTORY_CAPACITY],
            len: 0,
            next_insert: 0,
            browsing_age: None,
        }
    }

    fn push_line(&mut self, line: &InputLine) {
        if line.is_empty() {
            return;
        }
        if self
            .entry_by_age(0)
            .map(|entry| entry.as_str() == line.as_str())
            .unwrap_or(false)
        {
            self.browsing_age = None;
            return;
        }

        self.entries[self.next_insert].set_from_line(line);
        self.next_insert = (self.next_insert + 1) % HISTORY_CAPACITY;
        if self.len < HISTORY_CAPACITY {
            self.len += 1;
        }
        self.browsing_age = None;
    }

    fn previous(&mut self) -> Option<&HistoryEntry> {
        if self.len == 0 {
            return None;
        }
        let next_age = match self.browsing_age {
            None => 0,
            Some(age) => (age + 1).min(self.len - 1),
        };
        self.browsing_age = Some(next_age);
        self.entry_by_age(next_age)
    }

    fn next(&mut self) -> Option<&HistoryEntry> {
        let age = self.browsing_age?;
        if age == 0 {
            self.browsing_age = None;
            return None;
        }
        let next_age = age - 1;
        self.browsing_age = Some(next_age);
        self.entry_by_age(next_age)
    }

    fn reset_browsing(&mut self) {
        self.browsing_age = None;
    }

    fn entry_by_age(&self, age_from_newest: usize) -> Option<&HistoryEntry> {
        if age_from_newest >= self.len {
            return None;
        }
        let newest_index = (self.next_insert + HISTORY_CAPACITY - 1) % HISTORY_CAPACITY;
        let index = (newest_index + HISTORY_CAPACITY - age_from_newest) % HISTORY_CAPACITY;
        Some(&self.entries[index])
    }

    fn print_oldest_first(&self, port: &mut uart_16550::SerialPort) {
        if self.len == 0 {
            console_println!(port, "history is empty");
            return;
        }

        for index in 0..self.len {
            let age = self.len - 1 - index;
            if let Some(entry) = self.entry_by_age(age) {
                console_println!(port, "{:>2}: {}", index + 1, entry.as_str());
            }
        }
    }
}

fn input_loop(
    port: &mut uart_16550::SerialPort,
    memory_regions: &'static MemoryRegions,
    memory_summary: memory::MemorySummary,
    physical_memory_offset: Option<u64>,
) -> ! {
    const INPUT_WINDOW_TICKS: u64 = 800;

    let mut decoder = KeyboardDecoder::new();
    let start_ticks = interrupts::ticks();
    let mut deadline = start_ticks.saturating_add(INPUT_WINDOW_TICKS);
    let mut decoded_events = 0_u64;
    let mut line = InputLine::new();
    let mut history = CommandHistory::new();
    let mut rendered_len = MONITOR_PROMPT.len();
    let mut overwrite_mode = false;

    console_println!(port);
    console_println!(port, "{MONITOR_READY_MARKER}");
    console_println!(port, "jingos monitor ready. type 'help' for commands.");
    console_print!(port, "{MONITOR_PROMPT}");

    loop {
        while let Some(scancode) = interrupts::pop_scancode() {
            if let Some(event) = decoder.feed(scancode) {
                decoded_events += 1;
                deadline = interrupts::ticks().saturating_add(INPUT_WINDOW_TICKS);
                if handle_key_event(
                    port,
                    scancode,
                    event,
                    &mut line,
                    &mut history,
                    &mut rendered_len,
                    &mut overwrite_mode,
                    memory_regions,
                    memory_summary,
                    physical_memory_offset,
                ) {
                    let (keyboard_irqs, dropped) = interrupts::keyboard_counters();
                    console_println!(
                        port,
                        "Exiting monitor: keyboard_irqs={keyboard_irqs}, dropped={dropped}, decoded={decoded_events}"
                    );
                    exit_qemu(QemuExitCode::Success);
                }
            }
        }

        while let Some(serial_byte) = poll_serial_byte() {
            if let Some(event) = serial_byte_to_key_event(serial_byte) {
                decoded_events += 1;
                deadline = interrupts::ticks().saturating_add(INPUT_WINDOW_TICKS);
                if handle_key_event(
                    port,
                    serial_byte,
                    event,
                    &mut line,
                    &mut history,
                    &mut rendered_len,
                    &mut overwrite_mode,
                    memory_regions,
                    memory_summary,
                    physical_memory_offset,
                ) {
                    let (keyboard_irqs, dropped) = interrupts::keyboard_counters();
                    console_println!(
                        port,
                        "Exiting monitor: keyboard_irqs={keyboard_irqs}, dropped={dropped}, decoded={decoded_events}"
                    );
                    exit_qemu(QemuExitCode::Success);
                }
            }
        }

        if interrupts::ticks() >= deadline {
            let (keyboard_irqs, dropped) = interrupts::keyboard_counters();
            console_println!(
                port,
                "Input window ended: keyboard_irqs={keyboard_irqs}, dropped={dropped}, decoded={decoded_events}"
            );
            exit_qemu(QemuExitCode::Success);
        }

        x86_64::instructions::hlt();
    }
}

fn handle_key_event(
    port: &mut uart_16550::SerialPort,
    scancode: u8,
    event: KeyEvent,
    line: &mut InputLine,
    history: &mut CommandHistory,
    rendered_len: &mut usize,
    overwrite_mode: &mut bool,
    memory_regions: &'static MemoryRegions,
    memory_summary: memory::MemorySummary,
    physical_memory_offset: Option<u64>,
) -> bool {
    match event {
        KeyEvent::Char(character) => {
            if line.insert_or_overwrite_char(character, *overwrite_mode) {
                history.reset_browsing();
                redraw_prompt_with_line(port, line, rendered_len);
            } else {
                console_print!(port, "<FULL>");
            }
            false
        }
        KeyEvent::Ctrl(character) => {
            console_print!(port, "<C-{character}>");
            false
        }
        KeyEvent::Alt(character) => {
            console_print!(port, "<M-{character}>");
            false
        }
        KeyEvent::CtrlAlt(character) => {
            console_print!(port, "<C-M-{character}>");
            false
        }
        KeyEvent::Enter => {
            console_println!(port);
            history.push_line(line);
            let should_exit = execute_command(
                port,
                line.as_str(),
                memory_regions,
                memory_summary,
                physical_memory_offset,
                history,
            );
            line.clear();
            history.reset_browsing();
            if !should_exit {
                console_print!(port, "{MONITOR_PROMPT}");
                *rendered_len = MONITOR_PROMPT.len();
            }
            should_exit
        }
        KeyEvent::Tab => {
            complete_command(port, line, rendered_len);
            false
        }
        KeyEvent::Backspace => {
            if line.backspace() {
                history.reset_browsing();
                redraw_prompt_with_line(port, line, rendered_len);
            }
            false
        }
        KeyEvent::ArrowUp => {
            if let Some(entry) = history.previous() {
                let _ = line.set_from_str(entry.as_str());
                redraw_prompt_with_line(port, line, rendered_len);
            }
            false
        }
        KeyEvent::ArrowDown => {
            match history.next() {
                Some(entry) => {
                    let _ = line.set_from_str(entry.as_str());
                }
                None => line.clear(),
            }
            redraw_prompt_with_line(port, line, rendered_len);
            false
        }
        KeyEvent::ArrowLeft => {
            if line.move_left() {
                redraw_prompt_with_line(port, line, rendered_len);
            }
            false
        }
        KeyEvent::ArrowRight => {
            if line.move_right() {
                redraw_prompt_with_line(port, line, rendered_len);
            }
            false
        }
        KeyEvent::WordLeft => {
            if line.move_word_left() {
                redraw_prompt_with_line(port, line, rendered_len);
            }
            false
        }
        KeyEvent::WordRight => {
            if line.move_word_right() {
                redraw_prompt_with_line(port, line, rendered_len);
            }
            false
        }
        KeyEvent::Insert => {
            *overwrite_mode = !*overwrite_mode;
            console_println!(port);
            console_println!(
                port,
                "edit mode: {}",
                if *overwrite_mode {
                    "overwrite"
                } else {
                    "insert"
                }
            );
            redraw_prompt_with_line(port, line, rendered_len);
            false
        }
        KeyEvent::Delete => {
            if line.delete() {
                history.reset_browsing();
                redraw_prompt_with_line(port, line, rendered_len);
            }
            false
        }
        KeyEvent::WordDeleteLeft => {
            if line.delete_word_left() {
                history.reset_browsing();
                redraw_prompt_with_line(port, line, rendered_len);
            }
            false
        }
        KeyEvent::WordDeleteRight => {
            if line.delete_word_right() {
                history.reset_browsing();
                redraw_prompt_with_line(port, line, rendered_len);
            }
            false
        }
        KeyEvent::Home => {
            if line.move_home() {
                redraw_prompt_with_line(port, line, rendered_len);
            }
            false
        }
        KeyEvent::End => {
            if line.move_end() {
                redraw_prompt_with_line(port, line, rendered_len);
            }
            false
        }
        KeyEvent::PageUp => {
            console_print!(port, "<PGUP>");
            false
        }
        KeyEvent::PageDown => {
            console_print!(port, "<PGDN>");
            false
        }
        KeyEvent::Function(number) => {
            console_print!(port, "<F{number}>");
            false
        }
        KeyEvent::Escape => {
            console_println!(port, "<ESC>");
            true
        }
        KeyEvent::Unknown(code) => {
            console_println!(port, "<SC:{:#04x} RAW:{:#04x}>", code, scancode);
            false
        }
    }
}

fn run_task_payload(port: &mut uart_16550::SerialPort, kind: task::TaskKind) {
    match kind {
        task::TaskKind::UserDemo => match user_program::run_user_demo() {
            Ok(result) => {
                console_println!(
                    port,
                    "  user_demo result: ticks={} uptime={}s sum={} usable_bytes={} usable_frames={}",
                    result.ticks,
                    result.uptime_seconds,
                    result.sum,
                    result.usable_bytes,
                    result.usable_frames
                );
            }
            Err(error) => {
                console_println!(port, "  user_demo task failed: {}", syscall::error_name(error));
            }
        },
        task::TaskKind::FastSyscallSuccess => {
            console_println!(
                port,
                "  fast_syscall_success task: entering ring3 success-path syscall demo"
            );
            match usermode::run_fast_syscall_demo() {
                Ok(()) => {
                    console_println!(port, "  fast_syscall_success returned unexpectedly");
                }
                Err(error) => {
                    console_println!(port, "  fast_syscall_success task failed: {error}");
                }
            }
        }
        task::TaskKind::FastSyscallError => {
            console_println!(
                port,
                "  fast_syscall_error task: entering ring3 error-path syscall demo"
            );
            match usermode::run_fast_syscall_error_demo() {
                Ok(()) => {
                    console_println!(port, "  fast_syscall_error returned unexpectedly");
                }
                Err(error) => {
                    console_println!(port, "  fast_syscall_error task failed: {error}");
                }
            }
        }
        task::TaskKind::KernelMonitor => {}
    }
}

fn run_task_step_with_label(port: &mut uart_16550::SerialPort, label: &str) -> bool {
    match task::step(interrupts::ticks()) {
        Some(report) => {
            console_println!(
                port,
                "{label}: ran id={} kind={} runs={} (state reset to ready)",
                report.id,
                task::task_kind_name(report.kind),
                report.run_count
            );
            run_task_payload(port, report.kind);
            true
        }
        None => {
            console_println!(port, "{label}: no ready task");
            false
        }
    }
}

fn execute_command(
    port: &mut uart_16550::SerialPort,
    command_line: &str,
    memory_regions: &'static MemoryRegions,
    memory_summary: memory::MemorySummary,
    physical_memory_offset: Option<u64>,
    history: &CommandHistory,
) -> bool {
    let mut parts = command_line.split_whitespace();
    let Some(command) = parts.next() else {
        return false;
    };

    match command {
        "help" => {
            console_println!(
                port,
                "commands: help, status, ticks, uptime, irq, mem, maps [n|all], heap, vm [addr], vmmap [addr], fault [addr], syscall <n> [a0] [a1] [a2], sysabi, tasks, tasknew [userdemo|monitor|fastsyscall|fastsyscall_fail], taskstep, taskrun [n], tasksleep <id> [ticks], userdemo, usermode, usermode_syscall, usermode_syscall_fail, echo, history, clear, exit"
            );
        }
        "status" => {
            let (keyboard_irqs, dropped) = interrupts::keyboard_counters();
            console_println!(
                port,
                "ticks={} keyboard_irqs={} dropped={} phys_offset={:?}",
                interrupts::ticks(),
                keyboard_irqs,
                dropped,
                physical_memory_offset
            );
        }
        "ticks" => {
            console_println!(port, "ticks={}", interrupts::ticks());
        }
        "uptime" => {
            let ticks = interrupts::ticks();
            let seconds = ticks / 100;
            let centiseconds = ticks % 100;
            console_println!(
                port,
                "uptime={}s.{:02} (ticks={ticks})",
                seconds,
                centiseconds
            );
        }
        "irq" => {
            let (keyboard_irqs, dropped) = interrupts::keyboard_counters();
            console_println!(
                port,
                "keyboard_irqs={keyboard_irqs}, dropped_scancodes={dropped}"
            );
        }
        "mem" => {
            console_println!(
                port,
                "regions={} usable_regions={} usable_bytes={} reserved_bytes={} largest_usable={} usable_frames_4k={}",
                memory_summary.total_regions,
                memory_summary.usable_regions,
                memory_summary.usable_bytes,
                memory_summary.reserved_bytes,
                memory_summary.largest_usable_region_bytes,
                memory_summary.usable_frames_4k
            );
        }
        "maps" => {
            let limit = match parts.next() {
                None => Some(12_usize),
                Some("all") => None,
                Some(raw) => {
                    let Some(parsed) = parse_u64(raw) else {
                        console_println!(port, "invalid maps limit: {raw}");
                        return false;
                    };
                    Some(parsed as usize)
                }
            };

            console_println!(
                port,
                "memory map: total_regions={} (usable={} reserved_bytes={})",
                memory_summary.total_regions,
                memory_summary.usable_regions,
                memory_summary.reserved_bytes
            );

            let mut shown = 0_usize;
            for (index, region) in memory_regions.iter().enumerate() {
                if let Some(max_entries) = limit {
                    if shown >= max_entries {
                        break;
                    }
                }

                let size = region.end.saturating_sub(region.start);
                console_println!(
                    port,
                    "{:>3}: [{:#014x}..{:#014x}) size={:#010x} kind={}",
                    index,
                    region.start,
                    region.end,
                    size,
                    memory::memory_region_kind_name(region.kind)
                );
                shown += 1;
            }

            if shown < memory_regions.len() {
                console_println!(
                    port,
                    "... {} more regions (use `maps all`)",
                    memory_regions.len() - shown
                );
            }
        }
        "heap" => {
            let (boxed_value, vector_sum) = heap::probe_allocations();
            console_println!(
                port,
                "heap_probe: box={boxed_value:#x}, vec_sum={vector_sum}"
            );
        }
        "vm" => {
            let Some(offset) = physical_memory_offset else {
                console_println!(port, "physical memory mapping not enabled");
                return false;
            };

            if let Some(raw_addr) = parts.next() {
                let Some(virtual_address) = parse_u64(raw_addr) else {
                    console_println!(port, "invalid address: {raw_addr}");
                    return false;
                };
                match memory::translate_virtual_address(offset, virtual_address) {
                    Some(physical_address) => {
                        console_println!(
                            port,
                            "vm: virt={virtual_address:#x} -> phys={physical_address:#x}"
                        );
                    }
                    None => {
                        console_println!(port, "vm: virt={virtual_address:#x} is unmapped");
                    }
                }
            } else {
                console_println!(
                    port,
                    "vm usage: vm <addr>  (no args shows important mappings)"
                );
                print_vm_translation(port, offset, memory::DEMO_PAGE_START, "demo_page");
                print_vm_translation(port, offset, heap::HEAP_START, "heap_start");
                print_vm_translation(port, offset, heap::HEAP_START + 4096, "heap_next_page");
                let stack_probe = (&parts as *const _ as u64) & !0xfff;
                print_vm_translation(port, offset, stack_probe, "stack_probe_page");
                let code_probe = execute_command as *const () as usize as u64;
                print_vm_translation(port, offset, code_probe & !0xfff, "code_probe_page");
            }
        }
        "vmmap" => {
            let Some(offset) = physical_memory_offset else {
                console_println!(port, "physical memory mapping not enabled");
                return false;
            };

            if let Some(raw_addr) = parts.next() {
                let Some(virtual_address) = parse_u64(raw_addr) else {
                    console_println!(port, "invalid address: {raw_addr}");
                    return false;
                };
                print_vm_walk(port, offset, virtual_address, "target");
            } else {
                console_println!(
                    port,
                    "vmmap usage: vmmap <addr>  (no args shows important mappings)"
                );
                print_vm_walk(port, offset, memory::DEMO_PAGE_START, "demo_page");
                print_vm_walk(port, offset, heap::HEAP_START, "heap_start");
                print_vm_walk(port, offset, heap::HEAP_START + 4096, "heap_next_page");
                let stack_probe = (&parts as *const _ as u64) & !0xfff;
                print_vm_walk(port, offset, stack_probe, "stack_probe_page");
                let code_probe = execute_command as *const () as usize as u64;
                print_vm_walk(port, offset, code_probe & !0xfff, "code_probe_page");
            }
        }
        "fault" => {
            let fault_address = match parts.next() {
                Some(text) => match parse_u64(text) {
                    Some(address) => address,
                    None => {
                        console_println!(port, "invalid fault address: {text}");
                        return false;
                    }
                },
                None => 0xDEAD_BEEF,
            };
            console_println!(
                port,
                "triggering page fault by reading {fault_address:#x}..."
            );
            let pointer = fault_address as *const u64;
            let _ = unsafe { core::ptr::read_volatile(pointer) };
        }
        "syscall" => {
            let Some(number_text) = parts.next() else {
                console_println!(port, "usage: syscall <number> [arg0] [arg1] [arg2]");
                return false;
            };

            let Some(number) = parse_u64(number_text) else {
                console_println!(port, "invalid syscall number: {number_text}");
                return false;
            };

            let arg0 = match parts.next() {
                Some(text) => match parse_u64(text) {
                    Some(value) => value,
                    None => {
                        console_println!(port, "invalid arg0: {text}");
                        return false;
                    }
                },
                None => 0,
            };
            let arg1 = match parts.next() {
                Some(text) => match parse_u64(text) {
                    Some(value) => value,
                    None => {
                        console_println!(port, "invalid arg1: {text}");
                        return false;
                    }
                },
                None => 0,
            };
            let arg2 = match parts.next() {
                Some(text) => match parse_u64(text) {
                    Some(value) => value,
                    None => {
                        console_println!(port, "invalid arg2: {text}");
                        return false;
                    }
                },
                None => 0,
            };

            match syscall::invoke_via_interrupt(number, arg0, arg1, arg2) {
                Ok(value) => {
                    console_println!(
                        port,
                        "syscall {}({arg0}, {arg1}, {arg2}) -> {value:#x}",
                        syscall::syscall_name(number)
                    );
                }
                Err(error) => {
                    console_println!(port, "syscall error: {}", syscall::error_name(error));
                }
            }
        }
        "sysabi" => {
            console_println!(
                port,
                "syscall ABI mode: {} (vector={}, implementation={})",
                syscall::abi_mode_name(),
                syscall::SYSCALL_INTERRUPT_VECTOR,
                syscall::abi_mode_details()
            );

            console_println!(
                port,
                "fast syscall ABI: {}",
                syscall::fast_abi_mode_details()
            );

            let fast = syscall::fast_syscall_status();
            console_println!(
                port,
                "fast syscall scaffold: stage={} cpu_support={} sce_enabled={} note={}",
                syscall::fast_syscall_stage_name(fast.stage),
                fast.cpu_support,
                fast.sce_enabled,
                fast.note
            );

            if let Some(plan) = fast.selectors {
                console_println!(
                    port,
                    "fast syscall selector plan: kernel_cs={:#x} kernel_ss={:#x} user_cs={:#x} user_ss={:#x}",
                    plan.kernel_cs,
                    plan.kernel_ss,
                    plan.user_cs,
                    plan.user_ss
                );
            }

            if let Some(lstar) = fast.lstar {
                console_println!(port, "fast syscall lstar plan: {lstar:#x}");
            }

            if let Some(kernel_gs_base) = fast.kernel_gs_base {
                console_println!(port, "fast syscall kernel_gs_base: {kernel_gs_base:#x}");
            }
        }
        "tasks" => {
            let snapshot = task::snapshot();
            console_println!(
                port,
                "scheduler: total={} ready={} running={} sleeping={} finished={} capacity={} next_id={}",
                snapshot.total,
                snapshot.ready,
                snapshot.running,
                snapshot.sleeping,
                snapshot.finished,
                snapshot.capacity,
                snapshot.next_id
            );

            let mut entries: [Option<task::TaskInfo>; task::MAX_TASKS] = [None; task::MAX_TASKS];
            let count = task::list(&mut entries);
            if count == 0 {
                console_println!(port, "scheduler table is empty");
            } else {
                for entry in entries.iter().take(count).flatten() {
                    if let Some(until_tick) = entry.sleep_until_tick {
                        console_println!(
                            port,
                            "  task id={} kind={} state={} runs={} sleep_until_tick={}",
                            entry.id,
                            task::task_kind_name(entry.kind),
                            task::task_state_name(entry.state),
                            entry.run_count,
                            until_tick
                        );
                    } else {
                        console_println!(
                            port,
                            "  task id={} kind={} state={} runs={}",
                            entry.id,
                            task::task_kind_name(entry.kind),
                            task::task_state_name(entry.state),
                            entry.run_count
                        );
                    }
                }
            }
        }
        "tasknew" => {
            let kind_text = parts.next().unwrap_or("userdemo");
            let Some(kind) = task::parse_task_kind(kind_text) else {
                console_println!(
                    port,
                    "tasknew usage: tasknew [userdemo|monitor|fastsyscall|fastsyscall_fail] (got: {kind_text})"
                );
                return false;
            };

            match task::spawn(kind) {
                Ok(id) => {
                    console_println!(
                        port,
                        "task created: id={} kind={}",
                        id,
                        task::task_kind_name(kind)
                    );
                }
                Err(error) => {
                    console_println!(port, "tasknew failed: {error}");
                }
            }
        }
        "taskstep" => {
            run_task_step_with_label(port, "taskstep");
        }
        "taskrun" => {
            const DEFAULT_TASKRUN_STEPS: u64 = 4;
            const MAX_TASKRUN_STEPS: u64 = 256;

            let steps_requested = match parts.next() {
                None => DEFAULT_TASKRUN_STEPS,
                Some(raw_steps) => {
                    let Some(parsed) = parse_u64(raw_steps) else {
                        console_println!(port, "taskrun usage: taskrun [steps] (got: {raw_steps})");
                        return false;
                    };
                    if parsed == 0 {
                        console_println!(port, "taskrun steps must be > 0");
                        return false;
                    }
                    parsed.min(MAX_TASKRUN_STEPS)
                }
            };

            let mut executed = 0_u64;
            for _ in 0..steps_requested {
                if run_task_step_with_label(port, "taskrun") {
                    executed = executed.saturating_add(1);
                } else {
                    break;
                }
            }

            console_println!(
                port,
                "taskrun: requested={} executed={}",
                steps_requested,
                executed
            );
        }
        "tasksleep" => {
            const DEFAULT_TASKSLEEP_TICKS: u64 = 100;

            let Some(raw_task_id) = parts.next() else {
                console_println!(port, "tasksleep usage: tasksleep <id> [ticks]");
                return false;
            };

            let Some(task_id) = parse_u64(raw_task_id) else {
                console_println!(port, "tasksleep usage: tasksleep <id> [ticks] (got id={raw_task_id})");
                return false;
            };

            let sleep_ticks = match parts.next() {
                None => DEFAULT_TASKSLEEP_TICKS,
                Some(raw_ticks) => {
                    let Some(parsed) = parse_u64(raw_ticks) else {
                        console_println!(port, "tasksleep usage: tasksleep <id> [ticks] (got ticks={raw_ticks})");
                        return false;
                    };
                    if parsed == 0 {
                        console_println!(port, "tasksleep ticks must be > 0");
                        return false;
                    }
                    parsed
                }
            };

            let now_tick = interrupts::ticks();
            match task::sleep(task_id, sleep_ticks, now_tick) {
                Ok(until_tick) => {
                    console_println!(
                        port,
                        "tasksleep: id={} sleep_ticks={} now_tick={} until_tick={}",
                        task_id,
                        sleep_ticks,
                        now_tick,
                        until_tick
                    );
                }
                Err(error) => {
                    console_println!(port, "tasksleep failed: {error}");
                }
            }
        }
        "userdemo" => match user_program::run_user_demo() {
            Ok(report) => {
                console_println!(
                    port,
                    "userdemo: ticks={} uptime={}s sum={} usable_bytes={} usable_frames={}",
                    report.ticks,
                    report.uptime_seconds,
                    report.sum,
                    report.usable_bytes,
                    report.usable_frames
                );
            }
            Err(error) => {
                console_println!(port, "userdemo failed: {}", syscall::error_name(error));
            }
        },
        "usermode" => {
            console_println!(
                port,
                "launching ring3 demo (expects int 0x80 then int 0x81 return)..."
            );
            match usermode::run_demo() {
                Ok(()) => {
                    console_println!(port, "usermode returned unexpectedly");
                }
                Err(error) => {
                    console_println!(port, "usermode failed: {error}");
                }
            }
        }
        "usermode_syscall" => {
            console_println!(
                port,
                "launching ring3 fast-syscall demo (success path: rax(value)/r10(status), then int 0x81 return)..."
            );
            match usermode::run_fast_syscall_demo() {
                Ok(()) => {
                    console_println!(port, "usermode_syscall returned unexpectedly");
                }
                Err(error) => {
                    console_println!(port, "usermode_syscall failed: {error}");
                }
            }
        }
        "usermode_syscall_fail" => {
            console_println!(
                port,
                "launching ring3 fast-syscall demo (error path: unknown/overflow statuses), then int 0x81 return..."
            );
            match usermode::run_fast_syscall_error_demo() {
                Ok(()) => {
                    console_println!(port, "usermode_syscall_fail returned unexpectedly");
                }
                Err(error) => {
                    console_println!(port, "usermode_syscall_fail failed: {error}");
                }
            }
        }
        "echo" => {
            let text = command_line.strip_prefix("echo").unwrap_or("").trim_start();
            console_println!(port, "{text}");
        }
        "history" => {
            history.print_oldest_first(port);
        }
        "clear" => {
            framebuffer::clear();
            console_println!(port, "screen cleared");
        }
        "exit" | "quit" => return true,
        _ => {
            console_println!(port, "unknown command: {command}");
        }
    }

    false
}

fn print_vm_translation(
    port: &mut uart_16550::SerialPort,
    physical_memory_offset: u64,
    virtual_address: u64,
    label: &str,
) {
    match memory::translate_virtual_address(physical_memory_offset, virtual_address) {
        Some(physical_address) => {
            console_println!(
                port,
                "vm[{label}]: virt={virtual_address:#x} -> phys={physical_address:#x}"
            );
        }
        None => {
            console_println!(port, "vm[{label}]: virt={virtual_address:#x} is unmapped");
        }
    }
}

fn print_vm_walk(
    port: &mut uart_16550::SerialPort,
    physical_memory_offset: u64,
    virtual_address: u64,
    label: &str,
) {
    let walk = memory::walk_virtual_address(physical_memory_offset, virtual_address);
    console_println!(
        port,
        "vmmap[{label}]: virt={:#x} canonical={} idx(p4/p3/p2/p1)=({}/{}/{}/{}) off={:#05x}",
        walk.virtual_address,
        walk.canonical,
        walk.p4_index,
        walk.p3_index,
        walk.p2_index,
        walk.p1_index,
        walk.page_offset
    );
    console_println!(
        port,
        "  flags: p4={:#x} p3={:#x} p2={:#x} p1={:#x}",
        walk.p4_flags_bits,
        walk.p3_flags_bits,
        walk.p2_flags_bits,
        walk.p1_flags_bits
    );

    match (walk.physical_address, walk.page_size) {
        (Some(physical), Some(page_size)) => {
            console_println!(
                port,
                "  map: phys={physical:#x} page_size={}",
                memory::vm_page_size_name(page_size)
            );
        }
        _ => {
            console_println!(port, "  map: unmapped");
        }
    }
}

fn redraw_prompt_with_line(
    port: &mut uart_16550::SerialPort,
    line: &InputLine,
    rendered_len: &mut usize,
) {
    let visible_len = MONITOR_PROMPT.len() + line.len();
    console_print!(port, "\r{MONITOR_PROMPT}{}", line.as_str());

    if *rendered_len > visible_len {
        for _ in 0..(*rendered_len - visible_len) {
            console_print!(port, " ");
        }
    }

    console_print!(port, "\r{MONITOR_PROMPT}{}", line.as_prefix_str());
    *rendered_len = visible_len;
}

fn complete_command(
    port: &mut uart_16550::SerialPort,
    line: &mut InputLine,
    rendered_len: &mut usize,
) {
    let text = line.as_str();
    if text.contains(' ') || !line.is_cursor_at_end() {
        return;
    }

    let mut matched_count = 0_usize;
    let mut first_match = "";
    for command in COMMAND_NAMES {
        if command.starts_with(text) {
            matched_count += 1;
            if matched_count == 1 {
                first_match = command;
            }
        }
    }

    match matched_count {
        0 => {}
        1 => {
            if first_match.len() > text.len() {
                let suffix = &first_match[text.len()..];
                if line.append_str(suffix) {
                    redraw_prompt_with_line(port, line, rendered_len);
                }
            }
        }
        _ => {
            console_println!(port);
            console_print!(port, "matches:");
            for command in COMMAND_NAMES {
                if command.starts_with(text) {
                    console_print!(port, " {command}");
                }
            }
            console_println!(port);
            redraw_prompt_with_line(port, line, rendered_len);
        }
    }
}

fn parse_u64(text: &str) -> Option<u64> {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok();
    }
    text.parse::<u64>().ok()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let _ = writeln!(serial_port(), "KERNEL PANIC: {info}");
    println!("KERNEL PANIC: {info}");
    exit_qemu(QemuExitCode::Failed);
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {layout:?}");
}
