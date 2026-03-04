use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};
use std::env;
use std::process::{Command, exit};

fn main() {
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    let args: Vec<String> = env::args().collect();
    let program = &args[0];

    let mut use_uefi = None;
    let mut show_display = false;

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "uefi" => {
                if use_uefi.replace(true).is_some() {
                    eprintln!("Usage: {program} [uefi|bios] [--show]");
                    exit(1);
                }
            }
            "bios" => {
                if use_uefi.replace(false).is_some() {
                    eprintln!("Usage: {program} [uefi|bios] [--show]");
                    exit(1);
                }
            }
            "--show" => show_display = true,
            "--headless" => show_display = false,
            "-h" | "--help" => {
                println!("Usage: {program} [uefi|bios] [--show]");
                println!("  --show      enable QEMU window output");
                println!("  --headless  disable QEMU window output");
                exit(0);
            }
            _ => {
                eprintln!("Usage: {program} [uefi|bios] [--show]");
                exit(1);
            }
        }
    }

    let use_uefi = match use_uefi {
        Some(mode) => mode,
        None => {
            eprintln!("Usage: {program} [uefi|bios] [--show]");
            exit(1);
        }
    };

    let mut command = Command::new("qemu-system-x86_64");
    command.arg("-serial").arg("mon:stdio");
    if !show_display {
        command.arg("-display").arg("none");
    }
    command
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");

    if use_uefi {
        let prebuilt =
            Prebuilt::fetch(Source::LATEST, "target/ovmf").expect("failed to fetch OVMF images");

        let code = prebuilt.get_file(Arch::X64, FileType::Code);
        let vars = prebuilt.get_file(Arch::X64, FileType::Vars);

        command.arg("-drive").arg(format!("format=raw,file={uefi_path}"));
        command.arg("-drive").arg(format!(
            "if=pflash,format=raw,unit=0,file={},readonly=on",
            code.display()
        ));
        command.arg("-drive").arg(format!(
            "if=pflash,format=raw,unit=1,file={},snapshot=on",
            vars.display()
        ));
    } else {
        command.arg("-drive").arg(format!("format=raw,file={bios_path}"));
    }

    let mut child = command
        .spawn()
        .expect("failed to start qemu-system-x86_64");
    let status = child.wait().expect("failed to wait for qemu");

    let process_code = match status.code().unwrap_or(1) {
        0x10 | 0x21 => 0,
        0x11 | 0x23 => 1,
        _ => 2,
    };
    exit(process_code);
}
