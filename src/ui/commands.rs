use druid::{Command, Selector, Target};

use crate::protocol::Packet;
use crate::ui::appstate::Device;

pub struct ShowDeviceList;

impl ShowDeviceList {
    pub const SELECTOR: Selector<()> = Selector::new("libratone.show-home");

    pub fn command() -> Command {
        Command::new(Self::SELECTOR, (), Target::Auto)
    }
}

pub struct ShowDeviceDetails;

impl ShowDeviceDetails {
    pub const SELECTOR: Selector<String> = Selector::new("libratone.show-device");

    pub fn command(device_id: &str) -> Command {
        Command::new(Self::SELECTOR, device_id.to_owned(), Target::Auto)
    }
}

pub struct SendCommand {
    pub device_id: String,
    pub packet: Packet,
    pub optimistic_update: Box<dyn Fn(&mut Device)>,
}

impl SendCommand {
    pub const SELECTOR: Selector<SendCommand> = Selector::new("libratone.set-volume");

    pub fn command(
        device_id: &str,
        packet: Packet,
        optimistic_update: impl Fn(&mut Device) + 'static,
    ) -> Command {
        Command::new(
            Self::SELECTOR,
            SendCommand {
                device_id: device_id.to_owned(),
                packet,
                optimistic_update: Box::new(optimistic_update),
            },
            Target::Auto,
        )
    }
}

pub struct DeviceUpdated;

impl DeviceUpdated {
    pub const SELECTOR: Selector<Device> = Selector::new("libratone.device-updated");

    pub fn command(device: Device) -> Command {
        Command::new(Self::SELECTOR, device, Target::Auto)
    }
}
