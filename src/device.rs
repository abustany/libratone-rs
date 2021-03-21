use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};

use net2::unix::UnixUdpBuilderExt;

use crate::commands;
use crate::commands::{
    ChannelObject, ChargingStateData, Command, PlayControlCommand, PlayInfoData,
};
use crate::discovery_reply;
use crate::protocol;
use crate::protocol::{PacketReceiver, PacketSender};

#[derive(Clone, Debug)]
pub struct Device {
    id: String,
    addr: IpAddr,

    name: Option<String>,
    volume: Option<u8>,
    play_status: Option<PlayControlCommand>,
    play_info: Option<PlayInfoData>,
    pre_channels: Option<Vec<ChannelObject>>,
    charging_state: Option<ChargingStateData>,
    battery_level: Option<u8>,
}

impl Device {
    fn new(id: String, addr: IpAddr) -> Device {
        Device {
            id,
            addr,
            name: None,
            volume: None,
            play_status: None,
            play_info: None,
            pre_channels: None,
            charging_state: None,
            battery_level: None,
        }
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn addr(&self) -> IpAddr {
        self.addr
    }

    pub fn name(&self) -> Option<String> {
        self.name.clone()
    }

    pub fn volume(&self) -> Option<u8> {
        self.volume
    }

    pub fn play_status(&self) -> Option<PlayControlCommand> {
        self.play_status
    }

    pub fn play_info(&self) -> Option<PlayInfoData> {
        self.play_info.clone()
    }

    pub fn pre_channels(&self) -> Option<Vec<ChannelObject>> {
        self.pre_channels.clone()
    }
}

#[derive(Clone, Debug)]
pub enum DeviceManagerEvent {
    DeviceDiscovered(Device),
    DeviceUpdated(Device),
}

struct DeviceManagerData {
    event_listeners: Vec<std::sync::mpsc::Sender<DeviceManagerEvent>>,
    sock_send: Box<dyn PacketSender + Send>,
    devices: std::collections::HashMap<String, Device>,
}

pub struct DeviceManager {
    data: Arc<std::sync::Mutex<DeviceManagerData>>,
}

const ADDR_ANY: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);

pub trait NetworkImpl {
    fn packet_sender(&self) -> Result<Box<dyn PacketSender + Send>>;
    fn packet_receiver(&self, port: u16) -> Result<Box<dyn PacketReceiver + Send>>;
}

pub trait DeviceDiscoveryImpl {
    fn discover(&self) -> Result<()>;
    fn poll(&self) -> Result<discovery_reply::DiscoveryReply>;
}

type ThreadsafeNetworkImpl = Box<dyn NetworkImpl + Send + Sync + 'static>;
type ThreadsafeDeviceDiscoveryImpl = Box<dyn DeviceDiscoveryImpl + Send + Sync + 'static>;

pub struct DeviceManagerConfig {
    network_impl: Arc<ThreadsafeNetworkImpl>,
    device_discovery_impl: Arc<ThreadsafeDeviceDiscoveryImpl>,
}

impl DeviceManagerConfig {
    pub fn default() -> Result<Self> {
        Ok(DeviceManagerConfig {
            network_impl: Arc::new(Box::new(RealNetworkImpl {})),
            device_discovery_impl: Arc::new(Box::new(SSDPDiscovery::new()?)),
        })
    }

    pub fn new(
        network_impl: ThreadsafeNetworkImpl,
        device_discovery_impl: ThreadsafeDeviceDiscoveryImpl,
    ) -> Self {
        DeviceManagerConfig {
            network_impl: Arc::new(network_impl),
            device_discovery_impl: Arc::new(device_discovery_impl),
        }
    }
}

impl DeviceManager {
    pub fn new(config: DeviceManagerConfig) -> Result<DeviceManager> {
        let sock_send = config.network_impl.packet_sender()?;
        let data = Arc::new(std::sync::Mutex::new(DeviceManagerData {
            event_listeners: vec![],
            sock_send,
            devices: std::collections::HashMap::new(),
        }));

        {
            let data = Arc::clone(&data);
            let device_discovery_impl = Arc::clone(&config.device_discovery_impl);

            std::thread::spawn(|| {
                Self::thread_manager(
                    "discovery",
                    Self::discovery_thread,
                    data,
                    device_discovery_impl,
                );
            });
        }

        {
            let data = Arc::clone(&data);
            let network_impl = Arc::clone(&config.network_impl);

            std::thread::spawn(|| {
                Self::thread_manager(
                    "notification",
                    Self::notification_thread,
                    data,
                    network_impl,
                );
            });
        }

        {
            let data = Arc::clone(&data);
            let network_impl = Arc::clone(&config.network_impl);

            std::thread::spawn(|| {
                Self::thread_manager(
                    "command reply",
                    Self::command_reply_thread,
                    data,
                    network_impl,
                );
            });
        }

        Ok(DeviceManager { data })
    }

    fn thread_manager<F, T>(
        thread_name: &'static str,
        thread_func: F,
        data: Arc<std::sync::Mutex<DeviceManagerData>>,
        extra: Arc<T>,
    ) where
        F: Fn(Arc<std::sync::Mutex<DeviceManagerData>>, Arc<T>) -> Result<()>,
        T: Send + 'static,
    {
        loop {
            let data = Arc::clone(&data);
            let extra = Arc::clone(&extra);

            match thread_func(data, extra) {
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

    fn discovery_thread(
        data: Arc<std::sync::Mutex<DeviceManagerData>>,
        device_discovery_impl: Arc<ThreadsafeDeviceDiscoveryImpl>,
    ) -> Result<()> {
        device_discovery_impl
            .discover()
            .context("error sending discovery packet")?;

        loop {
            let device_discovery_impl = Arc::clone(&device_discovery_impl);

            match device_discovery_impl.poll() {
                Ok(x) => {
                    let data = Arc::clone(&data);
                    let mut data = data.lock().unwrap();
                    data.register_device(&x);
                }
                Err(err) => {
                    println!("Error processing discovery reply: {}", err);
                    continue;
                }
            };
        }
    }

    fn packet_receiver_thread<F>(
        network_impl: Arc<ThreadsafeNetworkImpl>,
        port: u16,
        packet_func: F,
    ) -> Result<()>
    where
        F: Fn(SocketAddr, &protocol::Packet) -> Result<()>,
    {
        let packet_receiver = network_impl
            .packet_receiver(port)
            .context("error creating packet receiver")?;

        loop {
            match packet_receiver.receive_packet() {
                Ok((from_addr, packet)) => {
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
        data: Arc<std::sync::Mutex<DeviceManagerData>>,
        network_impl: Arc<ThreadsafeNetworkImpl>,
    ) -> Result<()> {
        Self::packet_receiver_thread(
            network_impl,
            protocol::NOTIF_RECV_PORT,
            |from_addr, packet| {
                let data = Arc::clone(&data);
                let mut data = data.lock().unwrap();
                data.handle_notification(from_addr, &packet)
            },
        )
    }

    fn command_reply_thread(
        data: Arc<std::sync::Mutex<DeviceManagerData>>,
        network_impl: Arc<ThreadsafeNetworkImpl>,
    ) -> Result<()> {
        Self::packet_receiver_thread(
            network_impl,
            protocol::CMD_RESP_PORT,
            |from_addr, packet| {
                let data = Arc::clone(&data);
                let mut data = data.lock().unwrap();
                data.handle_command_response(from_addr, &packet)
            },
        )
    }

    pub fn listen(&self) -> std::sync::mpsc::Receiver<DeviceManagerEvent> {
        let (tx, rx) = std::sync::mpsc::channel();

        let data = Arc::clone(&self.data);
        let mut data = data.lock().unwrap();
        data.event_listeners.push(tx);

        rx
    }

    pub fn fetch_info(&self, device_id: &str) -> Result<()> {
        let data = Arc::clone(&self.data);
        let data = data.lock().unwrap();
        data.send_packet(device_id, &commands::DeviceName::fetch())?;
        data.send_packet(device_id, &commands::Volume::fetch())?;
        data.send_packet(device_id, &commands::PlayControl::fetch())?;
        data.send_packet(device_id, &commands::PlayInfo::fetch())?;
        data.send_packet(device_id, &commands::ChargingState::fetch())?;
        data.send_packet(device_id, &commands::BatteryLevel::fetch())?;
        data.send_packet(device_id, &commands::PreChannel::fetch())?;

        Ok(())
    }

    pub fn set_volume(&self, device_id: &str, volume: u8) -> Result<()> {
        let data = Arc::clone(&self.data);
        let data = data.lock().unwrap();
        data.send_packet(device_id, &commands::Volume::set(volume.clamp(0, 100)))
    }

    pub fn send_packet(&self, device_id: &str, packet: &protocol::Packet) -> Result<()> {
        let data = Arc::clone(&self.data);
        let data = data.lock().unwrap();
        data.send_packet(device_id, packet)
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

    fn register_device(&mut self, info: &discovery_reply::DiscoveryReply) {
        if self.devices.contains_key(&info.device_id) {
            return;
        }

        let device = Device::new(info.device_id.clone(), info.ip_address);
        self.devices.insert(device.id.clone(), device.clone());

        self.send_event(DeviceManagerEvent::DeviceDiscovered(device));
    }

    fn send_packet(&self, device_id: &str, packet: &protocol::Packet) -> Result<()> {
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
            commands::DeviceName::GET_REPLY_COMMAND_ID => {
                device.name = Some(commands::DeviceName::unmarshal_data(
                    packet.command_data.as_ref().unwrap_or(&vec![]),
                )?);
                Ok(Some(DeviceManagerEvent::DeviceUpdated(device.clone())))
            }
            commands::Volume::GET_REPLY_COMMAND_ID => {
                device.volume = Some(commands::Volume::unmarshal_data(
                    packet.command_data.as_ref().unwrap_or(&vec![]),
                )?);
                Ok(Some(DeviceManagerEvent::DeviceUpdated(device.clone())))
            }
            commands::PlayControl::GET_REPLY_COMMAND_ID => {
                device.play_status = Some(commands::PlayControl::unmarshal_data(
                    packet.command_data.as_ref().unwrap_or(&vec![]),
                )?);
                Ok(Some(DeviceManagerEvent::DeviceUpdated(device.clone())))
            }
            commands::PlayInfo::GET_REPLY_COMMAND_ID => {
                device.play_info = Some(commands::PlayInfo::unmarshal_data(
                    packet.command_data.as_ref().unwrap_or(&vec![]),
                )?);
                Ok(Some(DeviceManagerEvent::DeviceUpdated(device.clone())))
            }
            commands::ChargingState::GET_REPLY_COMMAND_ID => {
                device.charging_state = Some(commands::ChargingState::unmarshal_data(
                    packet.command_data.as_ref().unwrap_or(&vec![]),
                )?);
                Ok(Some(DeviceManagerEvent::DeviceUpdated(device.clone())))
            }
            commands::BatteryLevel::GET_REPLY_COMMAND_ID | commands::BatteryLevel::NOTIFY_ID => {
                device.battery_level = Some(commands::BatteryLevel::unmarshal_data(
                    packet.command_data.as_ref().unwrap_or(&vec![]),
                )?);
                Ok(Some(DeviceManagerEvent::DeviceUpdated(device.clone())))
            }
            commands::PreChannel::GET_COMMAND_ID => {
                device.pre_channels = Some(commands::PreChannel::unmarshal_data(
                    packet.command_data.as_ref().unwrap_or(&vec![]),
                )?);
                Ok(Some(DeviceManagerEvent::DeviceUpdated(device.clone())))
            }
            _ => Ok(None),
        }
    }
}

struct RealNetworkImpl;

impl NetworkImpl for RealNetworkImpl {
    fn packet_sender(&self) -> Result<Box<dyn PacketSender + Send>> {
        let sock = net2::UdpBuilder::new_v4()?
            .reuse_address(true)?
            .reuse_port(true)?
            .bind((ADDR_ANY, 0))?;
        Ok(Box::new(sock))
    }

    fn packet_receiver(&self, port: u16) -> Result<Box<dyn PacketReceiver + Send>> {
        let sock = net2::UdpBuilder::new_v4()?
            .reuse_address(true)?
            .reuse_port(true)?
            .bind((ADDR_ANY, port))
            .context("error creating socket")?;

        Ok(Box::new(sock))
    }
}

struct SSDPDiscovery {
    sock: UdpSocket,
}

const SSDP_MULTICAST_PORT: u16 = 1800; // for some reason not the standard one

impl SSDPDiscovery {
    fn new() -> Result<SSDPDiscovery> {
        let sock = net2::UdpBuilder::new_v4()?
            .reuse_address(true)?
            .reuse_port(true)?
            .bind((ADDR_ANY, SSDP_MULTICAST_PORT))
            .context("error creating socket")?;

        Ok(SSDPDiscovery { sock })
    }
}

impl DeviceDiscoveryImpl for SSDPDiscovery {
    fn discover(&self) -> Result<()> {
        const SSDP_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
        const SEARCH_REQUEST_BODY: &str = "M-SEARCH * HTTP/1.1";

        self.sock.send_to(
            SEARCH_REQUEST_BODY.as_bytes(),
            SocketAddr::new(IpAddr::V4(SSDP_MULTICAST_ADDR), SSDP_MULTICAST_PORT),
        )?;
        Ok(())
    }

    fn poll(&self) -> Result<discovery_reply::DiscoveryReply> {
        let mut recv_buffer = vec![0; 4096];
        let (count, _) = self
            .sock
            .recv_from(&mut recv_buffer)
            .context("error receiving discovery packet")?;

        discovery_reply::DiscoveryReply::parse(&recv_buffer[..count])
    }
}
