use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::process::{Command, Stdio, exit};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const DEFAULT_SERIAL_SCRIPT_DELAY_MS: u64 = 6000;
const SERIAL_MONITOR_READY_MARKER: &[u8] = b"[[JINGOS_MONITOR_READY]]";

#[derive(Debug, Clone)]
struct SerialScriptConfig {
    path: String,
    delay_ms: u64,
}

#[derive(Debug, Clone)]
enum SerialScriptAction {
    Send(Vec<u8>),
    SleepMs(u64),
}

fn print_usage(program: &str) {
    eprintln!(
        "Usage: {program} [uefi|bios] [--show|--headless] [--serial-only] [--serial-script <file>] [--serial-delay-ms <ms>]"
    );
}

fn parse_u64_option(program: &str, option: &str, value: &str) -> u64 {
    match value.parse::<u64>() {
        Ok(parsed) => parsed,
        Err(_) => {
            eprintln!("invalid value for {option}: {value}");
            print_usage(program);
            exit(1);
        }
    }
}

fn spawn_serial_output_forwarder(child: &mut std::process::Child) -> mpsc::Receiver<()> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let Some(mut child_stdout) = child.stdout.take() else {
        eprintln!("serial script requested, but child stdout is unavailable");
        exit(1);
    };

    thread::spawn(move || {
        let mut host_stdout = std::io::stdout();
        let mut read_buffer = [0_u8; 1024];
        let mut marker_window = Vec::with_capacity(SERIAL_MONITOR_READY_MARKER.len());
        let mut ready_sent = false;

        loop {
            let read_len = match child_stdout.read(&mut read_buffer) {
                Ok(0) => break,
                Ok(size) => size,
                Err(error) => {
                    eprintln!("failed to read qemu serial output: {error}");
                    break;
                }
            };

            if let Err(error) = host_stdout.write_all(&read_buffer[..read_len]) {
                eprintln!("failed to forward qemu serial output: {error}");
                break;
            }
            let _ = host_stdout.flush();

            if ready_sent {
                continue;
            }

            for byte in &read_buffer[..read_len] {
                if marker_window.len() == SERIAL_MONITOR_READY_MARKER.len() {
                    marker_window.remove(0);
                }
                marker_window.push(*byte);

                if marker_window == SERIAL_MONITOR_READY_MARKER {
                    let _ = ready_tx.send(());
                    ready_sent = true;
                    break;
                }
            }
        }
    });

    ready_rx
}

fn parse_sleep_directive(
    path: &str,
    line_number: usize,
    directive: &str,
    value: &str,
    scale_ms: u64,
) -> SerialScriptAction {
    let parsed = match value.parse::<u64>() {
        Ok(number) => number,
        Err(_) => {
            eprintln!(
                "invalid sleep value in `{path}` at line {} for `{directive}`: {value}",
                line_number
            );
            exit(1);
        }
    };

    SerialScriptAction::SleepMs(parsed.saturating_mul(scale_ms))
}

fn strip_inline_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'#' {
            continue;
        }

        if index == 0 || bytes[index - 1].is_ascii_whitespace() {
            return line[..index].trim_end();
        }
    }

    line
}

fn normalize_raw_script_bytes(bytes: Vec<u8>) -> Vec<u8> {
    if bytes.contains(&b'\r') {
        return bytes;
    }

    let mut normalized = Vec::with_capacity(bytes.len());
    for byte in bytes {
        if byte == b'\n' {
            normalized.push(b'\r');
        } else {
            normalized.push(byte);
        }
    }
    normalized
}

fn load_serial_script(path: &str) -> Vec<SerialScriptAction> {
    let bytes = match fs::read(path) {
        Ok(content) => content,
        Err(error) => {
            eprintln!("failed to read serial script `{path}`: {error}");
            exit(1);
        }
    };

    let text = match core::str::from_utf8(&bytes) {
        Ok(content) => content,
        Err(_) => {
            return vec![SerialScriptAction::Send(normalize_raw_script_bytes(bytes))];
        }
    };

    let mut actions = Vec::new();

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let trimmed_line = raw_line.trim();

        if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
            continue;
        }

        let line = strip_inline_comment(trimmed_line).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(value) = line.strip_prefix("sleep ") {
            actions.push(parse_sleep_directive(
                path,
                line_number,
                "sleep",
                value.trim(),
                1,
            ));
            continue;
        }

        if let Some(value) = line.strip_prefix("sleep_ms ") {
            actions.push(parse_sleep_directive(
                path,
                line_number,
                "sleep_ms",
                value.trim(),
                1,
            ));
            continue;
        }

        if let Some(value) = line.strip_prefix("sleep_s ") {
            actions.push(parse_sleep_directive(
                path,
                line_number,
                "sleep_s",
                value.trim(),
                1000,
            ));
            continue;
        }

        let mut command = line.as_bytes().to_vec();
        command.push(b'\r');
        actions.push(SerialScriptAction::Send(command));
    }

    if actions.is_empty() {
        eprintln!("serial script `{path}` has no executable actions");
        exit(1);
    }

    actions
}

fn main() {
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    let args: Vec<String> = env::args().collect();
    let program = &args[0];

    let mut use_uefi = None;
    let mut show_display = false;
    let mut serial_only = false;
    let mut serial_script_path: Option<String> = None;
    let mut serial_delay_ms: Option<u64> = None;

    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "uefi" => {
                if use_uefi.replace(true).is_some() {
                    print_usage(program);
                    exit(1);
                }
            }
            "bios" => {
                if use_uefi.replace(false).is_some() {
                    print_usage(program);
                    exit(1);
                }
            }
            "--show" => show_display = true,
            "--headless" => show_display = false,
            "--serial-only" => serial_only = true,
            "--serial-script" => {
                index += 1;
                if index >= args.len() {
                    eprintln!("missing value for --serial-script");
                    print_usage(program);
                    exit(1);
                }
                serial_script_path = Some(args[index].clone());
                serial_only = true;
            }
            "--serial-delay-ms" => {
                index += 1;
                if index >= args.len() {
                    eprintln!("missing value for --serial-delay-ms");
                    print_usage(program);
                    exit(1);
                }
                serial_delay_ms = Some(parse_u64_option(
                    program,
                    "--serial-delay-ms",
                    args[index].as_str(),
                ));
            }
            "-h" | "--help" => {
                println!(
                    "Usage: {program} [uefi|bios] [--show|--headless] [--serial-only] [--serial-script <file>] [--serial-delay-ms <ms>]"
                );
                println!("  --show             enable QEMU window output");
                println!("  --headless         disable QEMU window output");
                println!("  --serial-only      disable QEMU monitor; stdin/stdout only for serial");
                println!("  --serial-script    send commands from script file to serial stdin");
                println!(
                    "  --serial-delay-ms  base wait for monitor-ready marker before fallback send (default {} ms)",
                    DEFAULT_SERIAL_SCRIPT_DELAY_MS
                );
                println!(
                    "  script directives  sleep <ms> | sleep_ms <ms> | sleep_s <sec> | # comments"
                );
                println!(
                    "  script comments    supports full-line '#' and inline ' ... # ...' comments"
                );
                exit(0);
            }
            _ => {
                print_usage(program);
                exit(1);
            }
        }
        index += 1;
    }

    let use_uefi = match use_uefi {
        Some(mode) => mode,
        None => {
            print_usage(program);
            exit(1);
        }
    };

    let serial_script = serial_script_path.map(|path| SerialScriptConfig {
        path,
        delay_ms: serial_delay_ms.unwrap_or(DEFAULT_SERIAL_SCRIPT_DELAY_MS),
    });

    let mut command = Command::new("qemu-system-x86_64");
    if serial_only {
        command.arg("-serial").arg("stdio");
    } else {
        command.arg("-serial").arg("mon:stdio");
    }
    if !show_display {
        command.arg("-display").arg("none");
    }
    command
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");

    if serial_script.is_some() {
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
    }

    if use_uefi {
        let prebuilt =
            Prebuilt::fetch(Source::LATEST, "target/ovmf").expect("failed to fetch OVMF images");

        let code = prebuilt.get_file(Arch::X64, FileType::Code);
        let vars = prebuilt.get_file(Arch::X64, FileType::Vars);

        command
            .arg("-drive")
            .arg(format!("format=raw,file={uefi_path}"));
        command.arg("-drive").arg(format!(
            "if=pflash,format=raw,unit=0,file={},readonly=on",
            code.display()
        ));
        command.arg("-drive").arg(format!(
            "if=pflash,format=raw,unit=1,file={},snapshot=on",
            vars.display()
        ));
    } else {
        command
            .arg("-drive")
            .arg(format!("format=raw,file={bios_path}"));
    }

    let mut child = command.spawn().expect("failed to start qemu-system-x86_64");

    let serial_ready_rx = if serial_script.is_some() {
        Some(spawn_serial_output_forwarder(&mut child))
    } else {
        None
    };

    if let Some(config) = serial_script {
        let script_actions = load_serial_script(&config.path);
        let Some(mut child_stdin) = child.stdin.take() else {
            eprintln!("serial script requested, but child stdin is unavailable");
            exit(1);
        };
        let Some(serial_ready_rx) = serial_ready_rx else {
            eprintln!("serial script requested, but ready-marker channel is unavailable");
            exit(1);
        };

        thread::spawn(move || {
            let marker_wait_ms = config.delay_ms.saturating_mul(5);
            match serial_ready_rx.recv_timeout(Duration::from_millis(marker_wait_ms)) {
                Ok(()) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    eprintln!(
                        "monitor-ready marker not observed within {} ms; sending script anyway",
                        marker_wait_ms
                    );
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    eprintln!("serial output stream closed before ready marker; sending script anyway");
                }
            }

            for action in script_actions {
                match action {
                    SerialScriptAction::Send(bytes) => {
                        if let Err(error) = child_stdin.write_all(&bytes) {
                            eprintln!("failed to write serial script action: {error}");
                            return;
                        }
                        if let Err(error) = child_stdin.flush() {
                            eprintln!("failed to flush serial script action: {error}");
                            return;
                        }
                    }
                    SerialScriptAction::SleepMs(milliseconds) => {
                        thread::sleep(Duration::from_millis(milliseconds));
                    }
                }
            }
        });
    }

    let status = child.wait().expect("failed to wait for qemu");

    let process_code = match status.code().unwrap_or(1) {
        0x10 | 0x21 => 0,
        0x11 | 0x23 => 1,
        _ => 2,
    };
    exit(process_code);
}
