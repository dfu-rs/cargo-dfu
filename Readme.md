# cargo-dfu

This crate provides a cargo subcommand to flash ELF binaries via dfu
Most STM chips will probably work with this, although you might need to add the vid and pid to the vendor map

## Installation

You can install this utility with cargo:

```bash
cargo install cargo-dfu
```

## Usage

You can use it like cargo build or cargo-flash with the option of giving the vid and pid:

```bash
cargo dfu <args> --vid <vid> --pid <pid>
```

### Examples

#### flash the debug version of the current crate

```bash
cargo dfu 
```

#### specifying the chip

```bash
cargo dfu --chip stm32
```

#### specifying the vid and pid

```bash
cargo dfu --vid 0x483 --pid 0xdf11
```

## Add chip definitions
feel free to open a PR to add chips to this

## Notes
some chips like the gd32vf103 need additional udev rules therefor you need to copy the rules from the udev file into /etc/udev/rules.d/
```lang=bash
sudo cp udev.rules /etc/udev/rules.d/cargo-dfu.rules
```

## Roadmap
- [ ] make this crate multi-platform (PR to either the dfu crate to use rusb or the usbapi to add platform support)
- [ ] check if multiple chips are connected 
