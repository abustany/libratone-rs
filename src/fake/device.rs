use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::Result;

use crate::commands;
use crate::commands::Command;
use crate::device::{DeviceID, DeviceManagerEvent, DeviceManagerData};
use crate::discovery_reply;
use crate::protocol::{Packet, PacketSender};

struct FakePacketSender;

impl PacketSender for FakePacketSender {
    fn send_packet(&self, packet: &Packet, to: SocketAddr) -> Result<usize> {
        println!("Sending {:?} to {}", packet, to);
        Ok(1)
    }
}

pub struct DeviceManager {
    data: std::sync::Arc<std::sync::Mutex<DeviceManagerData>>,
}

impl DeviceManager {
    pub fn new() -> Result<DeviceManager> {
        let data = std::sync::Arc::new(std::sync::Mutex::new(DeviceManagerData {
            event_listeners: vec![],
            sock_send: Box::new(FakePacketSender{}),
            devices: std::collections::HashMap::new(),
        }));

        {
            // Discover device after 1s
            let data = std::sync::Arc::clone(&data);

            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(1));

                let data = std::sync::Arc::clone(&data);
                let mut data = data.lock().unwrap();

                data.register_device(&discovery_reply::DiscoveryReply{
                    device_name: "Test device".to_owned(),
                    device_id: "test-device".to_owned(),
                    device_state: String::new(),
                    port: 1234,
                    zone_id: String::new(),
                    creator: String::new(),
                    ip_address: IpAddr::V4(Ipv4Addr::new(192, 168, 10, 10)),
                    color_code: String::new(),
                    firmware_version: String::new(),
                    stereo_pair_id: String::new(),
                });
            });
        }

        Ok(DeviceManager { data })
    }

    pub fn listen(&self) -> std::sync::mpsc::Receiver<DeviceManagerEvent> {
        let (tx, rx) = std::sync::mpsc::channel();

        let data = std::sync::Arc::clone(&self.data);
        let mut data = data.lock().unwrap();
        data.event_listeners.push(tx);

        rx
    }

    pub fn fetch_info(&self, device_id: &DeviceID) -> Result<()> {
        let data = std::sync::Arc::clone(&self.data);
        let data = data.lock().unwrap();
        data.send_packet(device_id, &commands::DeviceName::fetch())
    }
}
