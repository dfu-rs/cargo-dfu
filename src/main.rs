mod utils;

use crate::utils::{elf_to_bin, flash_bin, vendor_map};
use colored::Colorize;
use rusb::{open_device_with_vid_pid, GlobalContext};

use clap::Parser;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;
// use structopt::StructOpt;

fn main() {
    // Initialize the logging backend.
    pretty_env_logger::init();

    // Get commandline options.
    // Skip the first arg which is the calling application name.
    let opt = Opt::parse_from(std::env::args().skip(1));

    if opt.list_chips {
        for vendor in vendor_map() {
            println!("{}", vendor.0);
        }
        return;
    }

    // Try and get the cargo project information.
    let project = cargo_project::Project::query(".").expect("Couldn't parse the Cargo.toml");

    // Decide what artifact to use.
    let artifact = if let Some(bin) = &opt.bin {
        cargo_project::Artifact::Bin(bin)
    } else if let Some(example) = &opt.example {
        cargo_project::Artifact::Example(example)
    } else {
        cargo_project::Artifact::Bin(project.name())
    };

    // Decide what profile to use.
    let profile = if opt.release {
        cargo_project::Profile::Release
    } else {
        cargo_project::Profile::Dev
    };

    // Try and get the artifact path.
    let path = project
        .path(
            artifact,
            profile,
            opt.target
                .as_deref()
                .map(|target| target.trim_end_matches(".json")),
            "x86_64-unknown-linux-gnu",
        )
        .expect("Couldn't find the build result");

    // Remove first two args which is the calling application name and the `dfu` command from cargo.
    let mut args: Vec<_> = std::env::args().skip(2).collect();

    // todo, keep as iter. difficult because we want to filter map remove two items at once.
    // Remove our args as cargo build does not understand them.
    let flags = ["--pid", "--vid", "--chip"].iter();
    for flag in flags {
        if let Some(index) = args.iter().position(|x| x == flag) {
            args.remove(index);
            args.remove(index);
        }
    }

    let status = Command::new("cargo")
        .arg("build")
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    if !status.success() {
        exit_with_process_status(status)
    }

    let Some(d) = (if let (Some(v), Some(p)) = (opt.vid, opt.pid) {
        open_device_with_vid_pid(v, p)
    } else if let Some(c) = opt.chip {
        println!("    {} for a connected {}.", "Searching".green().bold(), c);

        let mut device: Option<rusb::DeviceHandle<GlobalContext>> = None;

        let vendor = vendor_map();

        if let Some(products) = vendor.get(&c) {
            for (v, p) in products {
                if let Some(d) = open_device_with_vid_pid(*v, *p) {
                    device = Some(d);
                    break;
                }
            }
        }

        device
    } else {
        println!(
            "    {} for a connected device with known vid/pid pair.",
            "Searching".green().bold(),
        );

        let devices: Vec<_> = rusb::devices()
            .expect("Error with Libusb")
            .iter()
            .map(|d| d.device_descriptor().unwrap())
            .collect();

        let mut device: Option<rusb::DeviceHandle<GlobalContext>> = None;

        for d in devices {
            for vendor in vendor_map() {
                if vendor.1.contains(&(d.vendor_id(), d.product_id())) {
                    if let Some(d) = open_device_with_vid_pid(d.vendor_id(), d.product_id()) {
                        device = Some(d);
                        break;
                    }
                }
            }
        }

        device
    }) else {
        println!(
            "    {} finding connected devices, have you placed it into bootloader mode?",
            "Error".red().bold()
        );
        std::process::exit(101);
    };

    println!(
        "    {} {} {}",
        "Found ".green().bold(),
        d.read_manufacturer_string_ascii(&d.device().device_descriptor().unwrap())
            .unwrap(),
        d.read_product_string_ascii(&d.device().device_descriptor().unwrap())
            .unwrap()
    );

    println!("    {} {:?}", "Flashing".green().bold(), path);

    let (binary, _) = elf_to_bin(path).unwrap();

    // Start timer.
    let instant = Instant::now();

    // if let Err(e) = flash_bin(&binary, &d.device()) {
    //     println!("    {} flashing binary: {:?}", "Error".red().bold(), e);
    // }

    match flash_bin(&binary, &d.device()) {
        Err(utils::UtilError::Dfu(dfu_libusb::Error::LibUsb(rusb::Error::NoDevice))) => {
            // works for me?
        }
        Err(e) => println!("    {} flashing binary: {:?}", "Error".red().bold(), e),
        _ => (),
    }

    // Stop timer.
    let elapsed = instant.elapsed();
    println!(
        "    {} in {}s",
        "Finished".green().bold(),
        elapsed.as_millis() as f32 / 1000.0
    );
}

#[cfg(unix)]
fn exit_with_process_status(status: std::process::ExitStatus) -> ! {
    use std::os::unix::process::ExitStatusExt;
    let status = status.code().or_else(|| status.signal()).unwrap_or(1);
    std::process::exit(status)
}

#[cfg(not(unix))]
fn exit_with_process_status(status: std::process::ExitStatus) -> ! {
    let status = status.code().unwrap_or(1);
    std::process::exit(status)
}

fn parse_hex_16(input: &str) -> Result<u16, std::num::ParseIntError> {
    input.strip_prefix("0x").map_or_else(
        || input.parse(),
        |stripped| u16::from_str_radix(stripped, 16),
    )
}

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Opt {
    // `cargo build` arguments
    #[clap(name = "binary", long = "bin")]
    bin: Option<String>,
    #[clap(name = "example", long = "example")]
    example: Option<String>,
    #[clap(name = "package", short = 'p', long = "package")]
    package: Option<String>,
    #[clap(name = "release", long = "release")]
    release: bool,
    #[clap(name = "target", long = "target")]
    target: Option<String>,
    #[clap(name = "PATH", long = "manifest-path", parse(from_os_str))]
    manifest_path: Option<PathBuf>,
    #[clap(long)]
    no_default_features: bool,
    #[clap(long)]
    all_features: bool,
    #[clap(long)]
    features: Vec<String>,

    #[clap(name = "pid", long = "pid", parse(try_from_str = parse_hex_16))]
    pid: Option<u16>,
    #[clap(name = "vid", long = "vid",  parse(try_from_str = parse_hex_16))]
    vid: Option<u16>,

    #[clap(name = "chip", long = "chip")]
    chip: Option<String>,
    #[clap(name = "list-chips", long = "list-chips")]
    list_chips: bool,
}
