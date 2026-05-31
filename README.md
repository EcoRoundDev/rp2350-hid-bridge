# rp2350-hid-bridge

UART to USB HID bridge firmware for **Waveshare RP2350-Plus-16MB-M**.

Receives JSON commands over UART, translates them into USB HID keyboard/mouse reports. Built with Rust + Embassy async framework.

## Hardware

| Item | Spec |
|------|------|
| Board | Waveshare RP2350-Plus-16MB-M |
| Chip | RP2350A (Dual Cortex-M33, 150MHz) |
| Flash | 16MB |
| USB | USB-C (Device mode, HID) |
| UART | GP0 (TX) / GP1 (RX), 115200 baud |

## Wiring

```
RP2350-Plus          CP210x USB-UART
-----------          ---------------
GP0 (TX)  ────────── RX
GP1 (RX)  ────────── TX
GND       ────────── GND
```

## Build

```bash
# Prerequisites
rustup target add thumbv8m.main-none-eabihf
cargo install elf2uf2-rs

# Build
cargo build --release

# Flash (hold BOOT button, plug USB, then run)
cargo run --release
```

## Commands

All commands are JSON, one per line (terminated with `\n`). Responses are echoed back over UART.

### Mouse

```json
{"type":"mouse","x":50,"y":0,"buttons":0}
{"type":"click","button":1}
{"type":"scroll","wheel":5}
{"type":"move_to","x":2699,"y":314}
{"type":"click_at","x":2699,"y":314}
{"type":"click_at","x":2699,"y":314,"count":2}
```

| Field | Description |
|-------|-------------|
| x, y | `mouse`: relative movement (-128 to 127) |
| x, y | `move_to`/`click_at`: absolute screen pixel coordinate (from virtual desktop top-left) |
| buttons | Bitmask: 1=left, 2=right, 4=middle |
| button | For click/click_at: 1=left, 2=right, 4=middle (default: 1) |
| wheel | Scroll amount (positive=up, negative=down) |
| count | For click_at: number of clicks (default: 1) |

`move_to` and `click_at` work by pushing the cursor to the screen corner then moving relatively to the target. Coordinates are clamped to 0-8000.

### Keyboard

```json
{"type":"keypress","code":4,"modifier":0}
{"type":"key","code":4,"modifier":0}
{"type":"key_release"}
{"type":"combo","keys":[4,5,6],"modifier":1}
```

| Field | Description |
|-------|-------------|
| code | USB HID keycode (e.g. 4=A, 5=B, 44=Space) |
| modifier | Bitmask: 1=LCtrl, 2=LShift, 4=LAlt, 8=LGui |
| keys | Array of up to 6 keycodes (for combo) |

| Command | Description |
|---------|-------------|
| keypress | Press + release (50ms hold) |
| key | Press and hold (use key_release to release) |
| key_release | Release all keys |
| combo | Press multiple keys simultaneously (30ms hold) |

### Utility

```json
{"type":"delay","ms":100}
```

| Command | Description |
|---------|-------------|
| delay | Firmware-side wait (ms), does not send HID report |

## Responses

| Response | Meaning |
|----------|---------|
| `[ok]` | Command parsed and queued |
| `[err] busy` | Command queue full, try again later |
| `[err] <raw data>` | JSON parse failed, echoes raw input |
| `[err] overflow` | UART buffer overflow (>256 bytes) |
| `[rp2350-hid-bridge] booted v<version>` | Firmware started (includes version from Cargo.toml) |

## Project Structure

```
src/
  main.rs            — entry point, task spawning, command parsing
Cargo.toml           — package metadata and dependencies
memory.x             — RP2350 memory layout (FLASH 16MB, RAM 512K)
build.rs             — linker script setup
rust-toolchain.toml  — pinned nightly toolchain + target
CLAUDE.md            — AI coding assistant context
```

## Notes

- USB VID/PID: `0x2E8A:0xBA01` (Raspberry Pi VID, custom PID)
- HID write timeout: 200ms (prevents blocking if USB not ready)
- CMD channel capacity: 16 commands (returns `[err] busy` when full)
- LED: blinks every 1s (heartbeat), rapid flash on command received
- Coordinates for `move_to`/`click_at` are from the virtual desktop top-left corner
