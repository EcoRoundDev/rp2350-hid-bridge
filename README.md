# rp2350-hid-bridge

UART to USB HID bridge firmware for **Waveshare RP2350-Plus-16MB-M**.

Receives JSON commands over UART and translates them into USB HID keyboard/mouse reports. The USB device now enumerates with a Logitech G102 LIGHTSYNC identity and includes a vendor-defined HID report interface shaped for Logitech HID++ short/long reports.

## Hardware

| Item | Spec |
|------|------|
| Board | Waveshare RP2350-Plus-16MB-M |
| Chip | RP2350A (Dual Cortex-M33, 150MHz) |
| Flash | 16MB |
| USB | USB-C (Device mode, HID) |
| UART | GP0 (TX) / GP1 (RX), 115200 baud |

## Wiring

```text
RP2350-Plus          CP210x USB-UART
-----------          ---------------
GP0 (TX)  ---------- RX
GP1 (RX)  ---------- TX
GND       ---------- GND
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

## GUI Test Tool

A small host-side Tkinter GUI is included for manual firmware testing over the UART bridge.

```powershell
pip install -r requirements-dev.txt
python tools\hid_bridge_gui.py
```

1. Connect the USB-UART adapter to GP0/GP1/GND as shown above.
2. Open the firmware USB HID connection to the target computer.
3. Select the UART COM port in the GUI, keep baud at `115200`, and click `Open`.
4. Use the mouse buttons, keyboard buttons, shortcut buttons, text box, or custom JSON box to send commands.

The Keyboard area also includes modifier/system shortcuts: `Alt`, `Win`, `Alt+Tab`, `Alt+F4`, `Win+D`, `Win+R`, and `Win+E`, plus `Alt Down` / `Win Down` for hold-and-release testing.

The text box sends one `keypress` command per character using the USB HID keyboard usage table and a US keyboard layout. ASCII letters, digits, punctuation, space, tab, backspace, and Enter are supported. Unicode text such as Chinese characters cannot be sent as raw HID keycodes with this firmware protocol; use the host OS input method or send explicit HID keycodes instead.

The default text-send delay is `60ms`, which matches the firmware's `keypress` hold timing and helps avoid filling the 16-command queue.

## Project Structure

```text
src/
  main.rs       firmware entry point, USB/UART setup, task spawning
  tasks.rs      Embassy tasks for LED, UART reader, HID writer, USB device, HID++ drain
  command.rs    host-testable JSON command parsing and HidCommand model
  usb_desc.rs   Logitech G102 USB identity and HID++-shaped report descriptor constants
  hidpp.rs      firmware-side HID request handler for the vendor-defined HID interface
  lib.rs        no_std library surface for testable modules
Cargo.toml      package metadata, host-testable deps, ARM-only firmware deps
memory.x        RP2350 memory layout (FLASH 16MB, RAM 512K)
build.rs        linker script setup
rust-toolchain.toml pinned nightly toolchain + target
CLAUDE.md       AI coding assistant context
```

## Logitech/G Hub Notes

- USB VID/PID: `0x046D:0xC092` (Logitech / G102 LIGHTSYNC-style identity).
- USB strings: manufacturer `Logitech`, product `G102 LIGHTSYNC Gaming Mouse`.
- The firmware exposes standard keyboard and mouse HID interfaces plus a vendor-defined HID report descriptor with report IDs `0x10` and `0x11`.
- This is not a complete Logitech HID++ firmware implementation. G Hub may still require model-specific HID++ responses for DPI, lighting, profile, or feature queries before showing the device as fully connected and configurable.
- HID write timeout: 200ms (prevents blocking if USB is not ready).
- CMD channel capacity: 16 commands (returns `[err] busy` when full).
- LED: blinks every 1s (heartbeat), rapid flash on command received.
- Coordinates for `move_to`/`click_at` are from the virtual desktop top-left corner.

## Tests

```bash
cargo test --target x86_64-pc-windows-msvc --lib
cargo check
```

The library tests run on the host target and cover command parsing plus the G102/HID++ descriptor constants. The default `cargo check` uses `.cargo/config.toml` and checks the RP2350 firmware target.
