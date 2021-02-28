use std::sync::Arc;

use druid::{AppDelegate, Command, DelegateCtx, Env, Handled, Target};

use super::appstate::{AppState, Device, DeviceMap, Route};
use super::commands::{DeviceUpdated, SetVolume, ShowDeviceDetails, ShowDeviceList};
use super::super::device::DeviceManager;

pub struct Delegate {
    pub device_manager: Arc<DeviceManager>,
}

impl AppDelegate<AppState> for Delegate {
    fn command(&mut self, _ctx: &mut DelegateCtx, _target: Target, cmd: &Command, data: &mut AppState, _env: &Env) -> Handled {
        if cmd.is(ShowDeviceDetails::SELECTOR) {
            let device_id = cmd.get(ShowDeviceDetails::SELECTOR).unwrap();
            data.route = Route::DeviceDetails(device_id.to_owned());
            Handled::Yes
        } else if cmd.is(ShowDeviceList::SELECTOR) {
            data.route = Route::DeviceList;
            Handled::Yes
        } else if cmd.is(SetVolume::SELECTOR) {
            let cmd = cmd.get(SetVolume::SELECTOR).unwrap();
            data.devices.modify_device(&cmd.device_id, |d: &mut Device| d.volume = Some(cmd.volume));
            if let Err(err) = self.device_manager.set_volume(&cmd.device_id, cmd.volume) {
                println!("error setting volume for device {}: {}", &cmd.device_id, err);
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
