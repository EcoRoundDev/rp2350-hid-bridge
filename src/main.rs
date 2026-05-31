#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{UART0, USB};
use embassy_rp::uart::{BufferedInterruptHandler, BufferedUartRx, BufferedUartTx, Config as UartConfig};
use embassy_rp::usb::{Driver as UsbDriver, InterruptHandler as UsbInterruptHandler};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use embassy_usb::class::hid::{HidBootProtocol, HidSubclass, HidWriter, State as HidState};
use embassy_usb::{Builder, Config as UsbConfig};
use embedded_io_async::{Read, Write as _};
use heapless::Vec;
use static_cell::StaticCell;
use usbd_hid::descriptor::{KeyboardReport, MouseReport, SerializedDescriptor};
use {defmt_rtt as _, panic_halt as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    UART0_IRQ => BufferedInterruptHandler<UART0>;
});

#[derive(Clone)]
enum HidCommand {
    Key { code: u8, modifier: u8 },
    KeyRelease,
    KeyPress { code: u8, modifier: u8 },
    Mouse { x: i8, y: i8, buttons: u8 },
    Click { button: u8 },
    MoveTo { x: i32, y: i32 },
    ClickAt { x: i32, y: i32, button: u8, count: u8 },
    Scroll { wheel: i8 },
    Combo { keys: [u8; 6], modifier: u8 },
    Delay { ms: u16 },
}

static CMD_CHANNEL: Channel<CriticalSectionRawMutex, HidCommand, 16> = Channel::new();
static LED_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[embassy_executor::task]
async fn led_task(mut led: Output<'static>) {
    loop {
        led.toggle();
        match embassy_futures::select::select(
            Timer::after_millis(1000), LED_SIGNAL.wait(),
        ).await {
            embassy_futures::select::Either::First(_) => {}
            embassy_futures::select::Either::Second(_) => {
                for _ in 0..3 {
                    led.set_high(); Timer::after_millis(30).await;
                    led.set_low(); Timer::after_millis(30).await;
                }
            }
        }
    }
}

#[embassy_executor::task]
async fn uart_reader_task(mut rx: BufferedUartRx, mut tx: BufferedUartTx) {
    let _ = tx.write_all(concat!("[rp2350-hid-bridge] booted v", env!("CARGO_PKG_VERSION"), "\r\n").as_bytes()).await;
    let mut buf: Vec<u8, 256> = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if rx.read(&mut byte).await.is_err() { continue; }
        if byte[0] == b'\n' {
            if !buf.is_empty() {
                match parse_command(&buf) {
                    Some(cmd) => {
                        if CMD_CHANNEL.try_send(cmd).is_ok() {
                            let _ = tx.write_all(b"[ok]\r\n").await;
                            LED_SIGNAL.signal(());
                        } else {
                            let _ = tx.write_all(b"[err] busy\r\n").await;
                        }
                    }
                    None => {
                        let _ = tx.write_all(b"[err] ").await;
                        let _ = tx.write_all(&buf).await;
                        let _ = tx.write_all(b"\r\n").await;
                    }
                }
                buf.clear();
            }
            continue;
        }
        if byte[0] == b'\r' { continue; }
        if buf.push(byte[0]).is_err() {
            let _ = tx.write_all(b"[err] overflow\r\n").await;
            buf.clear();
        }
    }
}

#[embassy_executor::task]
async fn hid_writer_task(
    mut kb: HidWriter<'static, UsbDriver<'static, USB>, 8>,
    mut mouse: HidWriter<'static, UsbDriver<'static, USB>, 5>,
) {
    loop {
        let cmd = CMD_CHANNEL.receive().await;
        match cmd {
            HidCommand::Key { code, modifier } => {
                let report = KeyboardReport {
                    modifier, reserved: 0, leds: 0,
                    keycodes: [code, 0, 0, 0, 0, 0],
                };
                hid_write_kb(&mut kb, &report).await;
            }
            HidCommand::KeyRelease => {
                let report = KeyboardReport {
                    modifier: 0, reserved: 0, leds: 0, keycodes: [0; 6],
                };
                hid_write_kb(&mut kb, &report).await;
            }
            HidCommand::KeyPress { code, modifier } => {
                let press = KeyboardReport {
                    modifier, reserved: 0, leds: 0,
                    keycodes: [code, 0, 0, 0, 0, 0],
                };
                hid_write_kb(&mut kb, &press).await;
                Timer::after_millis(50).await;
                let release = KeyboardReport {
                    modifier: 0, reserved: 0, leds: 0, keycodes: [0; 6],
                };
                hid_write_kb(&mut kb, &release).await;
            }
            HidCommand::Mouse { x, y, buttons } => {
                let report = MouseReport { buttons, x, y, wheel: 0, pan: 0 };
                hid_write_mouse(&mut mouse, &report).await;
            }
            HidCommand::Click { button } => {
                let press = MouseReport { buttons: button, x: 0, y: 0, wheel: 0, pan: 0 };
                hid_write_mouse(&mut mouse, &press).await;
                Timer::after_millis(50).await;
                let release = MouseReport { buttons: 0, x: 0, y: 0, wheel: 0, pan: 0 };
                hid_write_mouse(&mut mouse, &release).await;
            }
            HidCommand::MoveTo { x, y } => {
                move_mouse_to(&mut mouse, x, y).await;
            }
            HidCommand::ClickAt { x, y, button, count } => {
                move_mouse_to(&mut mouse, x, y).await;
                for i in 0..count {
                    if i > 0 { Timer::after_millis(100).await; }
                    let press = MouseReport { buttons: button, x: 0, y: 0, wheel: 0, pan: 0 };
                    hid_write_mouse(&mut mouse, &press).await;
                    Timer::after_millis(50).await;
                    let rel = MouseReport { buttons: 0, x: 0, y: 0, wheel: 0, pan: 0 };
                    hid_write_mouse(&mut mouse, &rel).await;
                }
            }
            HidCommand::Scroll { wheel } => {
                let report = MouseReport { buttons: 0, x: 0, y: 0, wheel, pan: 0 };
                hid_write_mouse(&mut mouse, &report).await;
            }
            HidCommand::Combo { keys, modifier } => {
                let report = KeyboardReport {
                    modifier, reserved: 0, leds: 0, keycodes: keys,
                };
                hid_write_kb(&mut kb, &report).await;
                Timer::after_millis(30).await;
                let release = KeyboardReport {
                    modifier: 0, reserved: 0, leds: 0, keycodes: [0; 6],
                };
                hid_write_kb(&mut kb, &release).await;
            }
            HidCommand::Delay { ms } => {
                Timer::after_millis(ms as u64).await;
            }
        }
    }
}

async fn hid_write_kb(
    kb: &mut HidWriter<'static, UsbDriver<'static, USB>, 8>,
    report: &KeyboardReport,
) {
    match embassy_futures::select::select(
        kb.write_serialize(report),
        Timer::after_millis(200),
    ).await {
        embassy_futures::select::Either::First(_) => {}
        embassy_futures::select::Either::Second(_) => {}
    }
}

async fn hid_write_mouse(
    mouse: &mut HidWriter<'static, UsbDriver<'static, USB>, 5>,
    report: &MouseReport,
) {
    match embassy_futures::select::select(
        mouse.write_serialize(report),
        Timer::after_millis(200),
    ).await {
        embassy_futures::select::Either::First(_) => {}
        embassy_futures::select::Either::Second(_) => {}
    }
}

async fn move_mouse_to(
    mouse: &mut HidWriter<'static, UsbDriver<'static, USB>, 5>,
    x: i32,
    y: i32,
) {
    let x = x.clamp(0, 8000);
    let y = y.clamp(0, 8000);

    // Push to virtual desktop top-left
    // 50 * 127 = 6350px, covers any multi-monitor setup
    for _ in 0..50 {
        let report = MouseReport { buttons: 0, x: -127, y: -127, wheel: 0, pan: 0 };
        hid_write_mouse(mouse, &report).await;
    }
    Timer::after_millis(50).await;

    // Move to target position in steps of 127
    let mut remaining_x = x;
    let mut remaining_y = y;
    while remaining_x > 0 || remaining_y > 0 {
        let dx = remaining_x.min(127) as i8;
        let dy = remaining_y.min(127) as i8;
        remaining_x -= dx as i32;
        remaining_y -= dy as i32;
        let report = MouseReport { buttons: 0, x: dx, y: dy, wheel: 0, pan: 0 };
        hid_write_mouse(mouse, &report).await;
    }
    Timer::after_millis(200).await;
}

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, UsbDriver<'static, USB>>) {
    usb.run().await;
}

fn trim_bytes(data: &[u8]) -> &[u8] {
    let start = data.iter().position(|&b| b > b' ').unwrap_or(data.len());
    let end = data.iter().rposition(|&b| b > b' ').map(|i| i + 1).unwrap_or(start);
    &data[start..end]
}

fn parse_command(data: &[u8]) -> Option<HidCommand> {
    let trimmed = trim_bytes(data);
    #[derive(serde::Deserialize)]
    struct Cmd<'a> {
        #[serde(rename = "type")]
        cmd_type: &'a str,
        #[serde(default)]
        code: u8,
        #[serde(default)]
        modifier: u8,
        #[serde(default)]
        x: i32,
        #[serde(default)]
        y: i32,
        #[serde(default)]
        buttons: u8,
        #[serde(default)]
        button: u8,
        #[serde(default)]
        wheel: i8,
        #[serde(default)]
        keys: [u8; 6],
        #[serde(default)]
        ms: u16,
        #[serde(default = "default_count")]
        count: u8,
    }
    fn default_count() -> u8 { 1 }
    let (cmd, _) = serde_json_core::from_slice::<Cmd>(trimmed).ok()?;
    match cmd.cmd_type {
        "key" => Some(HidCommand::Key { code: cmd.code, modifier: cmd.modifier }),
        "key_release" => Some(HidCommand::KeyRelease),
        "keypress" => Some(HidCommand::KeyPress { code: cmd.code, modifier: cmd.modifier }),
        "mouse" => Some(HidCommand::Mouse {
            x: cmd.x.clamp(-128, 127) as i8,
            y: cmd.y.clamp(-128, 127) as i8,
            buttons: cmd.buttons,
        }),
        "click" => Some(HidCommand::Click { button: if cmd.button == 0 { 1 } else { cmd.button } }),
        "move_to" => Some(HidCommand::MoveTo { x: cmd.x, y: cmd.y }),
        "click_at" => Some(HidCommand::ClickAt {
            x: cmd.x, y: cmd.y,
            button: if cmd.button == 0 { 1 } else { cmd.button },
            count: if cmd.count == 0 { 1 } else { cmd.count },
        }),
        "scroll" => Some(HidCommand::Scroll { wheel: cmd.wheel }),
        "combo" => Some(HidCommand::Combo { keys: cmd.keys, modifier: cmd.modifier }),
        "delay" => Some(HidCommand::Delay { ms: cmd.ms }),
        _ => None,
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let led = Output::new(p.PIN_25, Level::Low);
    spawner.spawn(led_task(led).unwrap());

    let driver = UsbDriver::new(p.USB, Irqs);

    let mut config = UsbConfig::new(0x2E8A, 0xBA01);
    config.manufacturer = Some("RPI");
    config.product = Some("RP2350 HID Bridge");
    config.serial_number = Some("RP2350-002");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static KB_STATE: StaticCell<HidState<'static>> = StaticCell::new();
    static MOUSE_STATE: StaticCell<HidState<'static>> = StaticCell::new();
    let kb_state = KB_STATE.init(HidState::new());
    let mouse_state = MOUSE_STATE.init(HidState::new());

    let mut builder = Builder::new(
        driver, config,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        MSOS_DESC.init([0; 256]),
        CONTROL_BUF.init([0; 64]),
    );

    let kb_config = embassy_usb::class::hid::Config {
        report_descriptor: KeyboardReport::desc(),
        request_handler: None,
        poll_ms: 10,
        max_packet_size: 8,
        hid_subclass: HidSubclass::No,
        hid_boot_protocol: HidBootProtocol::None,
    };
    let kb_writer = HidWriter::<_, 8>::new(&mut builder, kb_state, kb_config);

    let mouse_config = embassy_usb::class::hid::Config {
        report_descriptor: MouseReport::desc(),
        request_handler: None,
        poll_ms: 10,
        max_packet_size: 5,
        hid_subclass: HidSubclass::No,
        hid_boot_protocol: HidBootProtocol::None,
    };
    let mouse_writer = HidWriter::<_, 5>::new(&mut builder, mouse_state, mouse_config);

    let usb = builder.build();

    static TX_BUF: StaticCell<[u8; 256]> = StaticCell::new();
    static RX_BUF: StaticCell<[u8; 1024]> = StaticCell::new();
    let tx_buf = &mut TX_BUF.init([0; 256])[..];
    let rx_buf = &mut RX_BUF.init([0; 1024])[..];

    let uart_config = UartConfig::default();
    let uart = embassy_rp::uart::BufferedUart::new(
        p.UART0, p.PIN_0, p.PIN_1, Irqs, tx_buf, rx_buf, uart_config,
    );
    let (tx, rx) = uart.split();

    spawner.spawn(usb_task(usb).unwrap());
    spawner.spawn(uart_reader_task(rx, tx).unwrap());
    spawner.spawn(hid_writer_task(kb_writer, mouse_writer).unwrap());
}