use std::net::{SocketAddr, UdpSocket};

use anyhow::{anyhow, Result};

pub const CMD_SEND_PORT: u16 = 7777;
pub const CMD_RESP_PORT: u16 = 7778;
pub const NOTIF_RECV_PORT: u16 = 3333;
pub const NOTIF_ACK_PORT: u16 = 3334;

#[derive(Debug, PartialEq)]
pub struct Packet {
    pub command_type: u8,
    pub command: u16,
    pub command_data: Option<Vec<u8>>,
}

impl Packet {
    pub fn parse(data: &[u8]) -> Result<Packet> {
        const HEADER_LEN: usize = 10;

        if data.len() < HEADER_LEN {
            return Err(anyhow!("packet is too short"));
        }

        let data_len: u16 = ((data[8] as u16) << 8) | (data[9] as u16);

        if (HEADER_LEN + (data_len as usize)) != data.len() {
            return Err(anyhow!("incorrect packet length"));
        }

        let command_type: u8 = data[2];
        let command: u16 = ((data[3] as u16) << 8) | (data[4] as u16);
        let command_data = if data_len > 0 {
            Some(data[HEADER_LEN..].to_vec())
        } else {
            None
        };

        Ok(Packet {
            command_type,
            command,
            command_data,
        })
    }

    pub fn data(&self) -> Vec<u8> {
        let data_len = self.command_data.as_ref().map_or(0, |x| x.len());
        let mut packet_data = vec![
            0xaa,
            0xaa,
            self.command_type,
            ((self.command & 0xff00) >> 8) as u8,
            (self.command & 0xff) as u8,
            0x00, // command status?
            0x12, // random byte
            0x34, // random byte
            ((data_len & 0xff00) >> 8) as u8,
            (data_len & 0xff) as u8,
        ];

        if let Some(command_data) = self.command_data.as_ref() {
            packet_data.extend(command_data.iter());
        }

        packet_data
    }
}

pub trait PacketSender {
    fn send_packet(&self, packet: &Packet, to: SocketAddr) -> Result<usize>;
}

impl PacketSender for UdpSocket {
    fn send_packet(&self, packet: &Packet, to: SocketAddr) -> Result<usize> {
        self.send_to(&packet.data(), to).map_err(|e| e.into())
    }
}

impl PacketSender for std::sync::Arc<UdpSocket> {
    fn send_packet(&self, packet: &Packet, to: SocketAddr) -> Result<usize> {
        self.send_to(&packet.data(), to).map_err(|e| e.into())
    }
}

pub trait PacketReceiver {
    fn receive_packet(&self) -> Result<(SocketAddr, Packet)>;
}

impl PacketReceiver for UdpSocket {
    fn receive_packet(&self) -> Result<(SocketAddr, Packet)> {
        let mut recv_buffer = vec![0; 65536];
        let (count, from_addr, ) = self.recv_from(&mut recv_buffer)?;

        Ok((from_addr, Packet::parse(&recv_buffer[..count])?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACKET: &'static [u8] = &[
        0xaa, 0xaa, 0x02, 0x00, 0x0e, 0x01, 0x00, 0x00, 0x00, 0x01, 0x30,
    ];

    #[test]
    fn parse_test() {
        assert_eq!(
            Packet::parse(PACKET).unwrap(),
            Packet {
                command_type: 2,
                command: 14,
                command_data: Some(vec![0x30]),
            },
        )
    }
}
