use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::Result;

use crate::commands;
use crate::commands::Command;
use crate::device::{DeviceDiscoveryImpl, DeviceManagerConfig, NetworkImpl};
use crate::discovery_reply::DiscoveryReply;
use crate::protocol::{CMD_RESP_PORT, NOTIF_RECV_PORT, Packet, PacketReceiver, PacketSender};

type AddressAndPacket = (SocketAddr, Packet);
type FakeSocket = (mpsc::Sender<AddressAndPacket>, Arc<Mutex<mpsc::Receiver<AddressAndPacket>>>);

trait FakeSocketMap {
    fn sender(&mut self, port: u16) -> mpsc::Sender<AddressAndPacket>;
    fn receiver(&mut self, port: u16) -> Arc<Mutex<mpsc::Receiver<AddressAndPacket>>>;
}

impl FakeSocketMap for HashMap<u16, FakeSocket> {
    fn sender(&mut self, port: u16) -> mpsc::Sender<AddressAndPacket> {
        self
            .entry(port)
            .or_insert_with(|| {
                let (tx, rx) = mpsc::channel();
                (tx, Arc::new(Mutex::new(rx)))
            })
            .0
            .clone()
    }

    fn receiver(&mut self, port: u16) -> Arc<Mutex<mpsc::Receiver<AddressAndPacket>>> {
        Arc::clone(
            &self
                .entry(port)
                .or_insert_with(|| {
                    let (tx, rx) = mpsc::channel();
                    (tx, Arc::new(Mutex::new(rx)))
                })
                .1
        )
    }
}

struct FakeNetwork {
    reply_senders: Arc<Mutex<HashMap<u16, FakeSocket>>>,
}

impl FakeNetwork {
    fn new() -> Self {
        FakeNetwork {
            reply_senders: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl NetworkImpl for FakeNetwork {
    fn packet_sender(&self) -> Result<Box<dyn PacketSender + Send>> {
        Ok(Box::new(FakePacketSender{
            reply_senders: Arc::clone(&self.reply_senders),
            playing: Arc::new(Mutex::new(Cell::new(false))),
        }))
    }

    fn packet_receiver(&self, port: u16) -> Result<Box<dyn PacketReceiver + Send>> {
        Ok(Box::new(FakePacketReceiver{reply_senders: Arc::clone(&self.reply_senders), port}))
    }
}

struct FakePacketSender {
    reply_senders: Arc<Mutex<HashMap<u16, FakeSocket>>>,
    playing: Arc<Mutex<Cell<bool>>>,
}

impl FakePacketSender {
    fn reply(&self, to_addr: SocketAddr, packet: Packet) {
        let reply_senders = Arc::clone(&self.reply_senders);
        let mut reply_senders = reply_senders.lock().unwrap();
        reply_senders.sender(to_addr.port()).send((to_addr, packet)).expect("error sending packet");
    }
}

impl PacketSender for FakePacketSender {
    fn send_packet(&self, packet: &Packet, to: SocketAddr) -> Result<usize> {
        println!("Sending {:?} to {}", packet, to);

        match packet.command_type {
            commands::COMMAND_TYPE_FETCH => match packet.command {
                commands::DeviceName::GET_COMMAND_ID => {
                    println!("faking DeviceName reply");
                    self.reply(
                        SocketAddr::new(to.ip(), CMD_RESP_PORT),
                        Packet {
                            command_type: commands::COMMAND_TYPE_SET,
                            command: commands::DeviceName::SET_COMMAND_ID,
                            command_data: Some("Pretty name".as_bytes().to_vec()),
                        },
                    )
                },
                commands::Volume::GET_COMMAND_ID => {
                    println!("faking Volume reply");
                    self.reply(
                        SocketAddr::new(to.ip(), CMD_RESP_PORT),
                        Packet {
                            command_type: commands::COMMAND_TYPE_SET,
                            command: commands::Volume::SET_COMMAND_ID,
                            command_data: Some("35".as_bytes().to_vec()),
                        },
                    )
                },
                _ => {},
            }

            commands::COMMAND_TYPE_SET => match packet.command {
                commands::Volume::SET_COMMAND_ID => {
                    println!("faking Volume notification");
                    self.reply(
                        SocketAddr::new(to.ip(), NOTIF_RECV_PORT),
                        Packet {
                            command_type: commands::COMMAND_TYPE_SET,
                            command: commands::Volume::NOTIFY_ID,
                            command_data: packet.command_data.clone(),
                        },
                    )
                },
                commands::PlayControl::SET_COMMAND_ID => {
                    println!("faking PlayControl notification");
                    let now_playing = {
                        let playing = Arc::clone(&self.playing);
                        let playing = playing.lock().unwrap();
                        let command_data = String::from_utf8_lossy(packet.command_data.as_ref().unwrap());

                        match command_data.as_ref() {
                            "PLAY" => { playing.set(true); }
                            "PAUSE" | "STOP" => { playing.set(false); }
                            "TOGGL" => { playing.set(!playing.get()); }
                            _ => {}
                        };

                        playing.get()
                    };
                    let notification_data: Vec<u8> = vec![if now_playing { 48 } else { 49 }];

                    self.reply(
                        SocketAddr::new(to.ip(), NOTIF_RECV_PORT),
                        Packet {
                            command_type: commands::COMMAND_TYPE_SET,
                            command: commands::PlayControl::NOTIFY_ID,
                            command_data: Some(notification_data),
                        },
                    )
                },
                _ => {},
            }

            _ => {},
        }

        Ok(packet.data().len())
    }
}

struct FakePacketReceiver {
    port: u16,
    reply_senders: Arc<Mutex<HashMap<u16, FakeSocket>>>,
}

impl PacketReceiver for FakePacketReceiver {
    fn receive_packet(&self) -> Result<(SocketAddr, Packet)> {
        let receiver = {
            let reply_senders = Arc::clone(&self.reply_senders);
            let mut reply_senders = reply_senders.lock().unwrap();
            Arc::clone(&reply_senders.receiver(self.port))
        };
        let receiver = receiver.lock().unwrap();
        receiver.recv().map_err(|err| err.into())
    }
}

const FAKE_DEVICE_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 10, 10));
const FAKE_DEVICE_PORT: u16 = 7777;

struct FakeDeviceDiscovery {
    sender: mpsc::Sender<DiscoveryReply>,
    receiver: mpsc::Receiver<DiscoveryReply>,
}

impl FakeDeviceDiscovery {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        FakeDeviceDiscovery{sender: tx, receiver: rx}
    }
}

impl DeviceDiscoveryImpl for FakeDeviceDiscovery {
    fn discover(&self) -> Result<()> {
        let sender = self.sender.clone();

        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(1));

            sender.send(DiscoveryReply{
                device_name: "Test device".to_owned(),
                device_id: "test-device".to_owned(),
                device_state: String::new(),
                port: FAKE_DEVICE_PORT,
                zone_id: String::new(),
                creator: String::new(),
                ip_address: FAKE_DEVICE_ADDR,
                color_code: String::new(),
                firmware_version: String::new(),
                stereo_pair_id: String::new(),
            }).expect("error sending discovery packet");
        });

        Ok(())
    }

    fn poll(&self) -> Result<DiscoveryReply> {
        let packet = self.receiver.recv().expect("error receiving discovery packet");
        Ok(packet)
    }
}

impl DeviceDiscoveryImpl for std::sync::Mutex<FakeDeviceDiscovery> {
    fn discover(&self) -> Result<()> {
        self.lock().unwrap().discover()
    }

    fn poll(&self) -> Result<DiscoveryReply> {
        self.lock().unwrap().poll()
    }
}

pub fn device_manager_config() -> Result<DeviceManagerConfig> {
    Ok(DeviceManagerConfig::new(
        Box::new(FakeNetwork::new()),
        Box::new(std::sync::Mutex::new(FakeDeviceDiscovery::new())),
    ))
}
