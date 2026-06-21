use embassy_futures::select::{select, Either};
use embassy_rp::gpio::Output;
use embassy_rp::peripherals::USB;
use embassy_rp::uart::{BufferedUartRx, BufferedUartTx};
use embassy_rp::usb::Driver as UsbDriver;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use embassy_usb::class::hid::{HidReader, HidWriter, ReadError};
use embedded_io_async::{Read, Write as _};
use heapless::Vec;
use rp2350_hid_bridge::command::{parse_command, HidCommand};
use rp2350_hid_bridge::usb_desc;
use usbd_hid::descriptor::{KeyboardReport, MouseReport};

type UsbHidWriter<const N: usize> = HidWriter<'static, UsbDriver<'static, USB>, N>;
type UsbHidReader<const N: usize> = HidReader<'static, UsbDriver<'static, USB>, N>;

pub static CMD_CHANNEL: Channel<CriticalSectionRawMutex, HidCommand, 16> = Channel::new();
pub static LED_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[embassy_executor::task]
pub async fn led_task(mut led: Output<'static>) {
    loop {
        led.toggle();
        match select(Timer::after_millis(1000), LED_SIGNAL.wait()).await {
            Either::First(_) => {}
            Either::Second(_) => {
                for _ in 0..3 {
                    led.set_high();
                    Timer::after_millis(30).await;
                    led.set_low();
                    Timer::after_millis(30).await;
                }
            }
        }
    }
}

#[embassy_executor::task]
pub async fn uart_reader_task(mut rx: BufferedUartRx, mut tx: BufferedUartTx) {
    let _ = tx
        .write_all(
            concat!(
                "[rp2350-hid-bridge] booted v",
                env!("CARGO_PKG_VERSION"),
                "\r\n"
            )
            .as_bytes(),
        )
        .await;
    let mut buf: Vec<u8, 256> = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        if rx.read(&mut byte).await.is_err() {
            continue;
        }

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

        if byte[0] == b'\r' {
            continue;
        }

        if buf.push(byte[0]).is_err() {
            let _ = tx.write_all(b"[err] overflow\r\n").await;
            buf.clear();
        }
    }
}

#[embassy_executor::task]
pub async fn hid_writer_task(mut kb: UsbHidWriter<8>, mut mouse: UsbHidWriter<5>) {
    loop {
        let cmd = CMD_CHANNEL.receive().await;
        match cmd {
            HidCommand::Key { code, modifier } => {
                let report = KeyboardReport {
                    modifier,
                    reserved: 0,
                    leds: 0,
                    keycodes: [code, 0, 0, 0, 0, 0],
                };
                hid_write_kb(&mut kb, &report).await;
            }
            HidCommand::KeyRelease => {
                let report = KeyboardReport {
                    modifier: 0,
                    reserved: 0,
                    leds: 0,
                    keycodes: [0; 6],
                };
                hid_write_kb(&mut kb, &report).await;
            }
            HidCommand::KeyPress { code, modifier } => {
                let press = KeyboardReport {
                    modifier,
                    reserved: 0,
                    leds: 0,
                    keycodes: [code, 0, 0, 0, 0, 0],
                };
                hid_write_kb(&mut kb, &press).await;
                Timer::after_millis(50).await;
                let release = KeyboardReport {
                    modifier: 0,
                    reserved: 0,
                    leds: 0,
                    keycodes: [0; 6],
                };
                hid_write_kb(&mut kb, &release).await;
            }
            HidCommand::Mouse { x, y, buttons } => {
                let report = MouseReport {
                    buttons,
                    x,
                    y,
                    wheel: 0,
                    pan: 0,
                };
                hid_write_mouse(&mut mouse, &report).await;
            }
            HidCommand::Click { button } => {
                click_mouse(&mut mouse, button).await;
            }
            HidCommand::MoveTo { x, y } => {
                move_mouse_to(&mut mouse, x, y).await;
            }
            HidCommand::ClickAt {
                x,
                y,
                button,
                count,
            } => {
                move_mouse_to(&mut mouse, x, y).await;
                for i in 0..count {
                    if i > 0 {
                        Timer::after_millis(100).await;
                    }
                    click_mouse(&mut mouse, button).await;
                }
            }
            HidCommand::Scroll { wheel } => {
                let report = MouseReport {
                    buttons: 0,
                    x: 0,
                    y: 0,
                    wheel,
                    pan: 0,
                };
                hid_write_mouse(&mut mouse, &report).await;
            }
            HidCommand::Combo { keys, modifier } => {
                let report = KeyboardReport {
                    modifier,
                    reserved: 0,
                    leds: 0,
                    keycodes: keys,
                };
                hid_write_kb(&mut kb, &report).await;
                Timer::after_millis(30).await;
                let release = KeyboardReport {
                    modifier: 0,
                    reserved: 0,
                    leds: 0,
                    keycodes: [0; 6],
                };
                hid_write_kb(&mut kb, &release).await;
            }
            HidCommand::Delay { ms } => {
                Timer::after_millis(ms as u64).await;
            }
        }
    }
}

#[embassy_executor::task]
pub async fn hidpp_task(mut reader: UsbHidReader<{ usb_desc::HIDPP_LONG_REPORT_TOTAL_LEN }>) {
    let mut buf = [0u8; usb_desc::HIDPP_LONG_REPORT_TOTAL_LEN];

    loop {
        match reader.read(&mut buf).await {
            Ok(_) => {}
            Err(ReadError::Disabled) => reader.ready().await,
            Err(ReadError::BufferOverflow) | Err(ReadError::Sync(_)) => {}
        }
    }
}

#[embassy_executor::task]
pub async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, UsbDriver<'static, USB>>) {
    usb.run().await;
}

async fn click_mouse(mouse: &mut UsbHidWriter<5>, button: u8) {
    let press = MouseReport {
        buttons: button,
        x: 0,
        y: 0,
        wheel: 0,
        pan: 0,
    };
    hid_write_mouse(mouse, &press).await;
    Timer::after_millis(50).await;
    let release = MouseReport {
        buttons: 0,
        x: 0,
        y: 0,
        wheel: 0,
        pan: 0,
    };
    hid_write_mouse(mouse, &release).await;
}

async fn hid_write_kb(kb: &mut UsbHidWriter<8>, report: &KeyboardReport) {
    match select(kb.write_serialize(report), Timer::after_millis(200)).await {
        Either::First(_) => {}
        Either::Second(_) => {}
    }
}

async fn hid_write_mouse(mouse: &mut UsbHidWriter<5>, report: &MouseReport) {
    match select(mouse.write_serialize(report), Timer::after_millis(200)).await {
        Either::First(_) => {}
        Either::Second(_) => {}
    }
}

async fn move_mouse_to(mouse: &mut UsbHidWriter<5>, x: i32, y: i32) {
    let x = x.clamp(0, 8000);
    let y = y.clamp(0, 8000);

    for _ in 0..50 {
        let report = MouseReport {
            buttons: 0,
            x: -127,
            y: -127,
            wheel: 0,
            pan: 0,
        };
        hid_write_mouse(mouse, &report).await;
    }
    Timer::after_millis(50).await;

    let mut remaining_x = x;
    let mut remaining_y = y;
    while remaining_x > 0 || remaining_y > 0 {
        let dx = remaining_x.min(127) as i8;
        let dy = remaining_y.min(127) as i8;
        remaining_x -= dx as i32;
        remaining_y -= dy as i32;
        let report = MouseReport {
            buttons: 0,
            x: dx,
            y: dy,
            wheel: 0,
            pan: 0,
        };
        hid_write_mouse(mouse, &report).await;
    }
    Timer::after_millis(200).await;
}
