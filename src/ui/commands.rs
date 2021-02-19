use druid::{Command, Selector, Target};

use super::appstate::Device;

pub struct ShowDeviceList;

impl ShowDeviceList {
    pub const SELECTOR: Selector<()> = Selector::new("libratone.show-home");

    pub fn new() -> Command {
        Command::new(Self::SELECTOR, (), Target::Auto)
    }
}

pub struct ShowDeviceDetails;

impl ShowDeviceDetails {
    pub const SELECTOR: Selector<String> = Selector::new("libratone.show-device");

    pub fn new(device_id: &str) -> Command {
        Command::new(Self::SELECTOR, device_id.to_owned(), Target::Auto)
    }
}

pub struct SetVolume {
    pub device_id: String,
    pub volume: u8,
}

impl SetVolume {
    pub const SELECTOR: Selector<SetVolume> = Selector::new("libratone.set-volume");

    pub fn new(device_id: &str, volume: u8) -> Command {
        Command::new(Self::SELECTOR, SetVolume{device_id: device_id.to_owned(), volume}, Target::Auto)
    }
}

pub struct DeviceUpdated;

impl DeviceUpdated {
    pub const SELECTOR: Selector<Device> = Selector::new("libratone.device-updated");

    pub fn new(device: Device) -> Command {
        Command::new(Self::SELECTOR, device, Target::Auto)
    }
}
