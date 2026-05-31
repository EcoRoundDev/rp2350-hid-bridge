# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Flash

```bash
cargo build --release          # build firmware
cargo run --release            # build + flash via elf2uf2-rs (board must be in BOOT mode)
```

Requires nightly Rust with `thumbv8m.main-none-eabihf` target (configured in rust-toolchain.toml).

## Architecture

Bare-metal `#![no_std]` firmware for the RP2350A (Cortex-M33) on a Waveshare RP2350-Plus-16MB-M board. Uses Embassy async runtime for task scheduling.

**Purpose:** Receives JSON commands over UART (GP0/GP1, 115200 baud) and translates them into USB HID keyboard/mouse reports sent to a host PC.

**Key dependencies:**
- `embassy-rp` — HAL for RP2350 peripherals (GPIO, UART, USB)
- `embassy-usb` + `usbd-hid` — USB device stack with HID class
- `serde` + `serde-json-core` — no_std JSON parsing for UART commands
- `defmt` + `defmt-rtt` — structured logging over RTT (debug probe)
- `heapless` — fixed-capacity collections (no heap allocator)

**Memory layout:** Defined in `memory.x` — BOOT2 (256B), FLASH (4MB), RAM (520K). Copied to OUT_DIR by `build.rs` for the linker.

## Constraints

- No heap: use `heapless` containers with fixed capacities.
- No `std`: only `core` and `alloc`-free crates.
- All async tasks run on Embassy's single-threaded executor.
- Log with `defmt::info!` / `defmt::debug!` etc., not `println!`.
