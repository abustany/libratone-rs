use std::sync::Arc;

use druid::{Data, Lens};
use druid::im::HashMap;

use crate::device;
use crate::commands::{PlayControlCommand, PlayInfoData};

#[derive(Clone, Data, Lens, Debug)]
pub struct Device {
    pub id: String,
    pub ip_addr: String,
    pub name: Option<String>,
    pub volume: Option<u8>,
    pub play_status: Option<Arc<PlayControlCommand>>,
    pub play_info: Option<Arc<PlayInfoData>>,
}

impl Device {
    pub fn label(&self) -> &str {
        self.name.as_ref().unwrap_or_else(|| &self.id)
    }
}

impl From<device::Device> for Device {
    fn from(d: device::Device) -> Self {
        Device{
            id: d.id(),
            ip_addr: d.addr().to_string(),
            name: d.name(),
            volume: d.volume(),
            play_status: d.play_status().map(|x| Arc::new(x)),
            play_info: d.play_info().map(|x| Arc::new(x)),
        }
    }
}

#[derive(Clone, Data)]
pub enum Route {
    DeviceList,
    DeviceDetails(String),
}

pub trait DeviceMap {
    fn upsert_device(&mut self, d: &Device);
    fn modify_device<F>(&mut self, device_id: &str, f: F) where F: FnOnce(&mut Device);
}

impl DeviceMap for HashMap<String, Device> {
    fn upsert_device(&mut self, d: &Device) {
        self.entry(d.id.clone()).and_modify(|x| *x = d.clone()).or_insert(d.clone());
    }

    fn modify_device<F>(&mut self, device_id: &str, f: F)
        where F: FnOnce(&mut Device) {
        self.entry(device_id.to_owned()).and_modify(f);
    }
}

#[derive(Clone, Data, Lens)]
pub struct AppState {
    pub route: Route,
    pub devices: HashMap<String, Device>,
}

impl AppState {
    pub fn show_device(&mut self, device_id: String) {
        self.route = Route::DeviceDetails(device_id);
    }

    pub fn current_device_id(&self) -> Option<String> {
        match &self.route {
            Route::DeviceDetails(device_id) => Some(device_id.clone()),
            _ => { None },
        }
    }
}
