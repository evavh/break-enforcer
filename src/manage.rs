use embassy_time::{Duration, Timer};
use crate::control::UsbPower;

#[embassy_executor::task]
pub async fn manage(mut usb: UsbPower) {}
