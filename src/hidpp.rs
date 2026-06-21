use rp2350_hid_bridge::usb_desc;

use embassy_usb::class::hid::{HidProtocolMode, ReportId, RequestHandler};
use embassy_usb::control::OutResponse;

pub struct HidppRequestHandler {
    short_report: [u8; usb_desc::HIDPP_SHORT_REPORT_PAYLOAD_LEN as usize],
    long_report: [u8; usb_desc::HIDPP_LONG_REPORT_PAYLOAD_LEN as usize],
    idle_ms: u32,
}

impl HidppRequestHandler {
    pub const fn new() -> Self {
        Self {
            short_report: [0; usb_desc::HIDPP_SHORT_REPORT_PAYLOAD_LEN as usize],
            long_report: [0; usb_desc::HIDPP_LONG_REPORT_PAYLOAD_LEN as usize],
            idle_ms: u32::MAX,
        }
    }

    fn store_short_report(&mut self, data: &[u8]) {
        self.short_report = [0; usb_desc::HIDPP_SHORT_REPORT_PAYLOAD_LEN as usize];
        let len = core::cmp::min(data.len(), self.short_report.len());
        self.short_report[..len].copy_from_slice(&data[..len]);
    }

    fn store_long_report(&mut self, data: &[u8]) {
        self.long_report = [0; usb_desc::HIDPP_LONG_REPORT_PAYLOAD_LEN as usize];
        let len = core::cmp::min(data.len(), self.long_report.len());
        self.long_report[..len].copy_from_slice(&data[..len]);
    }
}

impl Default for HidppRequestHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestHandler for HidppRequestHandler {
    fn get_report(&mut self, id: ReportId, buf: &mut [u8]) -> Option<usize> {
        let report = match id {
            ReportId::In(usb_desc::HIDPP_SHORT_REPORT_ID)
            | ReportId::Feature(usb_desc::HIDPP_SHORT_REPORT_ID) => &self.short_report[..],
            ReportId::In(usb_desc::HIDPP_LONG_REPORT_ID)
            | ReportId::Feature(usb_desc::HIDPP_LONG_REPORT_ID) => &self.long_report[..],
            _ => return None,
        };

        if buf.len() < report.len() {
            return None;
        }

        buf[..report.len()].copy_from_slice(report);
        Some(report.len())
    }

    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        match id {
            ReportId::Out(usb_desc::HIDPP_SHORT_REPORT_ID)
            | ReportId::Feature(usb_desc::HIDPP_SHORT_REPORT_ID) => {
                self.store_short_report(data);
                OutResponse::Accepted
            }
            ReportId::Out(usb_desc::HIDPP_LONG_REPORT_ID)
            | ReportId::Feature(usb_desc::HIDPP_LONG_REPORT_ID) => {
                self.store_long_report(data);
                OutResponse::Accepted
            }
            _ => OutResponse::Rejected,
        }
    }

    fn get_protocol(&self) -> HidProtocolMode {
        HidProtocolMode::Report
    }

    fn set_protocol(&mut self, protocol: HidProtocolMode) -> OutResponse {
        match protocol {
            HidProtocolMode::Report => OutResponse::Accepted,
            HidProtocolMode::Boot => OutResponse::Rejected,
        }
    }

    fn get_idle_ms(&mut self, _id: Option<ReportId>) -> Option<u32> {
        Some(self.idle_ms)
    }

    fn set_idle_ms(&mut self, _id: Option<ReportId>, duration_ms: u32) {
        self.idle_ms = duration_ms;
    }
}
