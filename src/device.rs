use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::{anyhow, Context, Result};

use net2;
use net2::unix::UnixUdpBuilderExt;

use crate::commands;
use crate::commands::Command;
use crate::discovery_reply;
use crate::protocol;
use crate::protocol::PacketSender;

pub type DeviceID = String;

#[derive(Clone, Debug)]
pub struct Device {
    id: DeviceID,
    addr: IpAddr,

    name: Option<String>,
    volume: Option<u8>,
}

impl Device {
    fn new(id: DeviceID, addr: IpAddr) -> Device {
        Device {
            id,
            addr,
            name: None,
            volume: None,
        }
    }

    pub fn id(&self) -> DeviceID {
        self.id.clone()
    }

    pub fn addr(&self) -> IpAddr {
        self.addr
    }

    pub fn name(&self) -> Option<String> {
        self.name.clone()
    }
}

#[derive(Clone, Debug)]
pub enum DeviceManagerEvent {
    DeviceDiscovered(Device),
    DeviceUpdated(Device),
}

// crate-public so that the fake device manager can use it
pub(crate) struct DeviceManagerData {
    pub(crate) event_listeners: Vec<std::sync::mpsc::Sender<DeviceManagerEvent>>,
    pub(crate) sock_send: Box<dyn PacketSender + Send>,
    pub(crate) devices: std::collections::HashMap<String, Device>,
}

pub struct DeviceManager {
    data: std::sync::Arc<std::sync::Mutex<DeviceManagerData>>,
}

const ADDR_ANY: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);

impl DeviceManager {
    pub fn new() -> Result<DeviceManager> {
        let sock_send = net2::UdpBuilder::new_v4()?
            .reuse_address(true)?
            .reuse_port(true)?
            .bind((ADDR_ANY, 0))?;
        let data = std::sync::Arc::new(std::sync::Mutex::new(DeviceManagerData {
            event_listeners: vec![],
            sock_send: Box::new(sock_send),
            devices: std::collections::HashMap::new(),
        }));

        {
            let data = std::sync::Arc::clone(&data);

            std::thread::spawn(|| {
                Self::thread_manager("discovery", Self::discovery_thread, data);
            });
        }

        {
            let data = std::sync::Arc::clone(&data);

            std::thread::spawn(|| {
                Self::thread_manager("notification", Self::notification_thread, data);
            });
        }

        {
            let data = std::sync::Arc::clone(&data);

            std::thread::spawn(|| {
                Self::thread_manager("command reply", Self::command_reply_thread, data);
            });
        }

        Ok(DeviceManager { data })
    }

    fn thread_manager<F>(
        thread_name: &'static str,
        thread_func: F,
        data: std::sync::Arc<std::sync::Mutex<DeviceManagerData>>,
    ) where
        F: Fn(std::sync::Arc<std::sync::Mutex<DeviceManagerData>>) -> Result<()>,
    {
        loop {
            let data = std::sync::Arc::clone(&data);

            match thread_func(data) {
                Err(err) => {
                    println!("error in thread {}: {}", thread_name, err);
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
                _ => {
                    unreachable!("thread should never exit");
                }
            }
        }
    }

    fn discovery_thread(data: std::sync::Arc<std::sync::Mutex<DeviceManagerData>>) -> Result<()> {
        const SSDP_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
        const SSDP_MULTICAST_PORT: u16 = 1800; // for some reason not the standard one
        const SEARCH_REQUEST_BODY: &'static str = "M-SEARCH * HTTP/1.1";

        let sock = net2::UdpBuilder::new_v4()?
            .reuse_address(true)?
            .reuse_port(true)?
            .bind((ADDR_ANY, SSDP_MULTICAST_PORT))
            .context("error creating socket")?;

        sock.send_to(
            SEARCH_REQUEST_BODY.as_bytes(),
            SocketAddr::new(IpAddr::V4(SSDP_MULTICAST_ADDR), SSDP_MULTICAST_PORT),
        )
        .context("error sending discovery packet")?;

        let mut recv_buffer = vec![0; 4096];

        loop {
            let (count, from_addr) = sock
                .recv_from(&mut recv_buffer)
                .context("error receiving discovery packet")?;

            if count == 0 {
                continue;
            }

            match discovery_reply::DiscoveryReply::parse(&recv_buffer[..count]) {
                Ok(x) => {
                    let data = std::sync::Arc::clone(&data);
                    let mut data = data.lock().unwrap();
                    data.register_device(&x);
                }
                Err(err) => {
                    println!(
                        "Skipping invalid discovery reply from {} ({})",
                        from_addr, err
                    );
                    continue;
                }
            };
        }
    }

    fn packet_receiver<F>(port: u16, packet_func: F) -> Result<()>
    where
        F: Fn(SocketAddr, &protocol::Packet) -> Result<()>,
    {
        let sock = net2::UdpBuilder::new_v4()?
            .reuse_address(true)?
            .reuse_port(true)?
            .bind((ADDR_ANY, port))
            .context("error creating socket")?;

        let mut recv_buffer = vec![0; 65536];

        loop {
            let (count, from_addr) = sock
                .recv_from(&mut recv_buffer)
                .context("error receiving discovery packet")?;

            if count == 0 {
                continue;
            }

            match protocol::Packet::parse(&recv_buffer[..count]) {
                Ok(packet) => {
                    if let Err(err) = packet_func(from_addr, &packet) {
                        println!("error handling packet from device {}: {}", from_addr, err);
                    }
                }
                Err(err) => {
                    println!("invalid packet: {}", err)
                }
            };
        }
    }

    fn notification_thread(
        data: std::sync::Arc<std::sync::Mutex<DeviceManagerData>>,
    ) -> Result<()> {
        Self::packet_receiver(protocol::NOTIF_RECV_PORT, |from_addr, packet| {
            let data = std::sync::Arc::clone(&data);
            let mut data = data.lock().unwrap();
            data.handle_notification(from_addr, &packet)
        })
    }

    fn command_reply_thread(
        data: std::sync::Arc<std::sync::Mutex<DeviceManagerData>>,
    ) -> Result<()> {
        Self::packet_receiver(protocol::CMD_RESP_PORT, |from_addr, packet| {
            let data = std::sync::Arc::clone(&data);
            let mut data = data.lock().unwrap();
            data.handle_command_response(from_addr, &packet)
        })
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

impl DeviceManagerData {
    fn send_event(&mut self, event: DeviceManagerEvent) {
        let mut failed_send_indices = vec![];

        for (i, tx) in self.event_listeners.iter().enumerate() {
            if tx.send(event.clone()).is_err() {
                failed_send_indices.push(i);
            }
        }

        for idx in failed_send_indices.iter().rev() {
            self.event_listeners.remove(*idx);
        }
    }

    pub(crate) fn register_device(&mut self, info: &discovery_reply::DiscoveryReply) {
        if self.devices.contains_key(&info.device_id) {
            return;
        }

        let device = Device::new(info.device_id.clone(), info.ip_address);
        self.devices.insert(
            device.id.clone(),
            device.clone(),
        );

        self.send_event(DeviceManagerEvent::DeviceDiscovered(device));
    }

    pub(crate) fn send_packet(&self, device_id: &DeviceID, packet: &protocol::Packet) -> Result<()> {
        match self.devices.get(device_id) {
            Some(device) => {
                self.sock_send.send_packet(
                    packet,
                    SocketAddr::new(device.addr, protocol::CMD_SEND_PORT),
                )?;
                Ok(())
            }
            None => Err(anyhow!("unknown device ID")),
        }
    }

    fn handle_notification(&mut self, addr: SocketAddr, packet: &protocol::Packet) -> Result<()> {
        let notif_ack_addr = SocketAddr::new(addr.ip(), protocol::NOTIF_ACK_PORT);
        self.sock_send
            .send_packet(
                &protocol::Packet {
                    command: 2,
                    command_type: packet.command_type,
                    command_data: None,
                },
                notif_ack_addr,
            )
            .context("error acknowledging notification")?;

        println!(
            "handling notification packet for {}: {}",
            addr,
            commands::format_notification(packet)
        );

        self.handle_incoming_packet(addr, packet)
    }

    fn handle_command_response(
        &mut self,
        addr: SocketAddr,
        packet: &protocol::Packet,
    ) -> Result<()> {
        println!(
            "handling reply packet for {}: {}",
            addr,
            commands::format_reply(packet)
        );

        self.handle_incoming_packet(addr, packet)
    }

    fn handle_incoming_packet(
        &mut self,
        addr: SocketAddr,
        packet: &protocol::Packet,
    ) -> Result<()> {
        if let Some(event) = self
            .devices
            .values_mut()
            .find(|device| device.addr == addr.ip())
            .map(|device| Self::handle_device_update(device, packet))
            .unwrap_or(Ok(None))?
        {
            self.send_event(event);
        }

        Ok(())
    }

    fn handle_device_update(
        device: &mut Device,
        packet: &protocol::Packet,
    ) -> Result<Option<DeviceManagerEvent>> {
        match packet.command {
            commands::DeviceName::SET_COMMAND_ID => {
                device.name = Some(commands::DeviceName::unmarshal_data(
                    packet.command_data.as_ref().unwrap_or(&vec![]),
                )?);
                Ok(Some(DeviceManagerEvent::DeviceUpdated(device.clone())))
            }
            commands::Volume::SET_COMMAND_ID => {
                device.volume = Some(commands::Volume::unmarshal_data(
                    packet.command_data.as_ref().unwrap_or(&vec![]),
                )?);
                Ok(Some(DeviceManagerEvent::DeviceUpdated(device.clone())))
            }
            _ => Ok(None),
        }
    }
}
