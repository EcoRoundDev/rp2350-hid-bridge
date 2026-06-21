pub const LOGITECH_VENDOR_ID: u16 = 0x046d;
pub const G102_LIGHTSYNC_PRODUCT_ID: u16 = 0xc092;
pub const MANUFACTURER: &str = "Logitech";
pub const PRODUCT: &str = "G102 LIGHTSYNC Gaming Mouse";
pub const SERIAL_NUMBER: &str = "RP2350-G102-001";
pub const MAX_POWER_MA: u16 = 100;
pub const HID_POLL_MS: u8 = 1;

pub const HIDPP_SHORT_REPORT_ID: u8 = 0x10;
pub const HIDPP_LONG_REPORT_ID: u8 = 0x11;
pub const HIDPP_SHORT_REPORT_PAYLOAD_LEN: u8 = 0x06;
pub const HIDPP_LONG_REPORT_PAYLOAD_LEN: u8 = 0x13;
pub const HIDPP_SHORT_REPORT_TOTAL_LEN: usize = 1 + HIDPP_SHORT_REPORT_PAYLOAD_LEN as usize;
pub const HIDPP_LONG_REPORT_TOTAL_LEN: usize = 1 + HIDPP_LONG_REPORT_PAYLOAD_LEN as usize;

pub const HIDPP_REPORT_DESCRIPTOR: &[u8] = &[
    0x06,
    0x00,
    0xff, // Usage Page (Vendor Defined 0xff00)
    0x09,
    0x01, // Usage (0x01)
    0xa1,
    0x01, // Collection (Application)
    0x15,
    0x00, // Logical Minimum (0)
    0x26,
    0xff,
    0x00, // Logical Maximum (255)
    0x75,
    0x08, // Report Size (8 bits)
    0x85,
    HIDPP_SHORT_REPORT_ID, // Report ID 0x10
    0x09,
    HIDPP_SHORT_REPORT_ID, // Usage
    0x95,
    HIDPP_SHORT_REPORT_PAYLOAD_LEN, // Report Count (6 bytes payload)
    0x81,
    0x02, // Input (Data, Variable, Absolute)
    0x09,
    HIDPP_SHORT_REPORT_ID, // Usage
    0x95,
    HIDPP_SHORT_REPORT_PAYLOAD_LEN, // Report Count (6 bytes payload)
    0x91,
    0x02, // Output (Data, Variable, Absolute)
    0x09,
    HIDPP_SHORT_REPORT_ID, // Usage
    0x95,
    HIDPP_SHORT_REPORT_PAYLOAD_LEN, // Report Count (6 bytes payload)
    0xb1,
    0x02, // Feature (Data, Variable, Absolute)
    0x85,
    HIDPP_LONG_REPORT_ID, // Report ID 0x11
    0x09,
    HIDPP_LONG_REPORT_ID, // Usage
    0x95,
    HIDPP_LONG_REPORT_PAYLOAD_LEN, // Report Count (19 bytes payload)
    0x81,
    0x02, // Input (Data, Variable, Absolute)
    0x09,
    HIDPP_LONG_REPORT_ID, // Usage
    0x95,
    HIDPP_LONG_REPORT_PAYLOAD_LEN, // Report Count (19 bytes payload)
    0x91,
    0x02, // Output (Data, Variable, Absolute)
    0x09,
    HIDPP_LONG_REPORT_ID, // Usage
    0x95,
    HIDPP_LONG_REPORT_PAYLOAD_LEN, // Report Count (19 bytes payload)
    0xb1,
    0x02, // Feature (Data, Variable, Absolute)
    0xc0, // End Collection
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_g102_usb_identity() {
        assert_eq!(LOGITECH_VENDOR_ID, 0x046d);
        assert_eq!(G102_LIGHTSYNC_PRODUCT_ID, 0xc092);
        assert_eq!(MANUFACTURER, "Logitech");
        assert_eq!(PRODUCT, "G102 LIGHTSYNC Gaming Mouse");
    }

    #[test]
    fn hidpp_descriptor_exposes_short_and_long_reports() {
        assert!(HIDPP_REPORT_DESCRIPTOR
            .windows(3)
            .any(|item| item == [0x06, 0x00, 0xff]));
        assert!(HIDPP_REPORT_DESCRIPTOR
            .windows(2)
            .any(|item| item == [0x85, HIDPP_SHORT_REPORT_ID]));
        assert!(HIDPP_REPORT_DESCRIPTOR
            .windows(2)
            .any(|item| item == [0x95, HIDPP_SHORT_REPORT_PAYLOAD_LEN]));
        assert!(HIDPP_REPORT_DESCRIPTOR
            .windows(2)
            .any(|item| item == [0x85, HIDPP_LONG_REPORT_ID]));
        assert!(HIDPP_REPORT_DESCRIPTOR
            .windows(2)
            .any(|item| item == [0x95, HIDPP_LONG_REPORT_PAYLOAD_LEN]));
        assert_eq!(HIDPP_REPORT_DESCRIPTOR.last(), Some(&0xc0));
    }
}
