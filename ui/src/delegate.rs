use std::sync::Arc;

use druid::{AppDelegate, Command, DelegateCtx, Env, Handled, Target};

use super::appstate::{AppState, DeviceMap, Route};
use super::commands::{DeviceUpdated, SendCommand, ShowDeviceDetails, ShowDeviceList};
use libratone_rs::device::DeviceManager;

pub struct Delegate {
    pub device_manager: Arc<DeviceManager>,
}

impl AppDelegate<AppState> for Delegate {
    fn command(
        &mut self,
        _ctx: &mut DelegateCtx,
        _target: Target,
        cmd: &Command,
        data: &mut AppState,
        _env: &Env,
    ) -> Handled {
        if cmd.is(ShowDeviceDetails::SELECTOR) {
            let device_id = cmd.get(ShowDeviceDetails::SELECTOR).unwrap();
            data.route = Route::DeviceDetails(device_id.to_owned());
            Handled::Yes
        } else if cmd.is(ShowDeviceList::SELECTOR) {
            data.route = Route::DeviceList;
            Handled::Yes
        } else if cmd.is(SendCommand::SELECTOR) {
            let cmd = cmd.get(SendCommand::SELECTOR).unwrap();

            data.devices
                .modify_device(&cmd.device_id, |d| (&cmd.optimistic_update)(d));

            if let Err(err) = self.device_manager.send_packet(&cmd.device_id, &cmd.packet) {
                println!(
                    "error sending command to device {}: {}",
                    &cmd.device_id, err
                );
            }

            Handled::Yes
        } else if cmd.is(DeviceUpdated::SELECTOR) {
            let device = cmd.get(DeviceUpdated::SELECTOR).unwrap();
            data.devices.upsert_device(device);
            Handled::Yes
        } else {
            Handled::No
        }
    }
}
