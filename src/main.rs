#![no_std]
#![no_main]

mod hidpp;
mod tasks;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{UART0, USB};
use embassy_rp::uart::{BufferedInterruptHandler, Config as UartConfig};
use embassy_rp::usb::{Driver as UsbDriver, InterruptHandler as UsbInterruptHandler};
use embassy_usb::class::hid::{
    HidBootProtocol, HidReaderWriter, HidSubclass, HidWriter, State as HidState,
};
use embassy_usb::{Builder, Config as UsbConfig};
use rp2350_hid_bridge::usb_desc;
use static_cell::StaticCell;
use usbd_hid::descriptor::{KeyboardReport, MouseReport, SerializedDescriptor};
use {defmt_rtt as _, panic_halt as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    UART0_IRQ => BufferedInterruptHandler<UART0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let led = Output::new(p.PIN_25, Level::Low);
    spawner.spawn(tasks::led_task(led).unwrap());

    let driver = UsbDriver::new(p.USB, Irqs);
    let mut config = UsbConfig::new(
        usb_desc::LOGITECH_VENDOR_ID,
        usb_desc::G102_LIGHTSYNC_PRODUCT_ID,
    );
    config.manufacturer = Some(usb_desc::MANUFACTURER);
    config.product = Some(usb_desc::PRODUCT);
    config.serial_number = Some(usb_desc::SERIAL_NUMBER);
    config.max_power = usb_desc::MAX_POWER_MA;
    config.max_packet_size_0 = 64;

    static CONFIG_DESC: StaticCell<[u8; 512]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static KB_STATE: StaticCell<HidState<'static>> = StaticCell::new();
    static MOUSE_STATE: StaticCell<HidState<'static>> = StaticCell::new();
    static HIDPP_STATE: StaticCell<HidState<'static>> = StaticCell::new();
    static HIDPP_HANDLER: StaticCell<hidpp::HidppRequestHandler> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0; 512]),
        BOS_DESC.init([0; 256]),
        MSOS_DESC.init([0; 256]),
        CONTROL_BUF.init([0; 64]),
    );

    let kb_config = embassy_usb::class::hid::Config {
        report_descriptor: KeyboardReport::desc(),
        request_handler: None,
        poll_ms: usb_desc::HID_POLL_MS,
        max_packet_size: 8,
        hid_subclass: HidSubclass::No,
        hid_boot_protocol: HidBootProtocol::None,
    };
    let kb_writer = HidWriter::<_, 8>::new(&mut builder, KB_STATE.init(HidState::new()), kb_config);

    let mouse_config = embassy_usb::class::hid::Config {
        report_descriptor: MouseReport::desc(),
        request_handler: None,
        poll_ms: usb_desc::HID_POLL_MS,
        max_packet_size: 5,
        hid_subclass: HidSubclass::No,
        hid_boot_protocol: HidBootProtocol::None,
    };
    let mouse_writer = HidWriter::<_, 5>::new(
        &mut builder,
        MOUSE_STATE.init(HidState::new()),
        mouse_config,
    );

    let hidpp_config = embassy_usb::class::hid::Config {
        report_descriptor: usb_desc::HIDPP_REPORT_DESCRIPTOR,
        request_handler: Some(HIDPP_HANDLER.init(hidpp::HidppRequestHandler::new())),
        poll_ms: usb_desc::HID_POLL_MS,
        max_packet_size: usb_desc::HIDPP_LONG_REPORT_TOTAL_LEN as u16,
        hid_subclass: HidSubclass::No,
        hid_boot_protocol: HidBootProtocol::None,
    };
    let hidpp = HidReaderWriter::<
        _,
        { usb_desc::HIDPP_LONG_REPORT_TOTAL_LEN },
        { usb_desc::HIDPP_LONG_REPORT_TOTAL_LEN },
    >::new(
        &mut builder,
        HIDPP_STATE.init(HidState::new()),
        hidpp_config,
    );
    let (hidpp_reader, _hidpp_writer) = hidpp.split();

    let usb = builder.build();

    static TX_BUF: StaticCell<[u8; 256]> = StaticCell::new();
    static RX_BUF: StaticCell<[u8; 1024]> = StaticCell::new();
    let tx_buf = &mut TX_BUF.init([0; 256])[..];
    let rx_buf = &mut RX_BUF.init([0; 1024])[..];

    let uart = embassy_rp::uart::BufferedUart::new(
        p.UART0,
        p.PIN_0,
        p.PIN_1,
        Irqs,
        tx_buf,
        rx_buf,
        UartConfig::default(),
    );
    let (tx, rx) = uart.split();

    spawner.spawn(tasks::usb_task(usb).unwrap());
    spawner.spawn(tasks::uart_reader_task(rx, tx).unwrap());
    spawner.spawn(tasks::hid_writer_task(kb_writer, mouse_writer).unwrap());
    spawner.spawn(tasks::hidpp_task(hidpp_reader).unwrap());
}
