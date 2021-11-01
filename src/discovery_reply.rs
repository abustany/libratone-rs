use std::borrow::Cow;
use std::net::IpAddr;

use anyhow::{anyhow, Context, Result};

#[derive(Debug, PartialEq)]
pub struct DiscoveryReply {
    pub device_name: String,
    pub device_id: String,
    pub device_state: String,
    pub port: u16,
    pub zone_id: String,
    pub creator: String,
    pub ip_address: IpAddr,
    pub color_code: String,
    pub firmware_version: String,
    pub stereo_pair_id: String,
}

impl DiscoveryReply {
    pub fn parse(input: &[u8]) -> Result<DiscoveryReply> {
        // For some reason Libratone devices add a space in the first line of
        // the response, between "HTTP/1.1" and the first "\r\n". This
        // confuses httparse, so strip it out.
        const BROKEN_NOTIFY_PREFIX: &[u8] = "NOTIFY * HTTP/1.1 \r\n".as_bytes();
        const FIXED_NOTIFY_PREFIX: &[u8] = "NOTIFY * HTTP/1.1\r\n".as_bytes();

        let input = if input.starts_with(BROKEN_NOTIFY_PREFIX) {
            Cow::Owned(
                FIXED_NOTIFY_PREFIX
                    .iter()
                    .chain(input.iter().skip(BROKEN_NOTIFY_PREFIX.len()))
                    .cloned()
                    .collect(),
            )
        } else {
            Cow::Borrowed(input)
        };

        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut req = httparse::Request::new(&mut headers);
        req.parse(&input).context("error parsing HTTP data")?;

        if req.method != Some("NOTIFY") {
            return Err(anyhow!("unexpected method: {:?}", req.method));
        }

        if req.path != Some("*") {
            return Err(anyhow!("unexpected path: {:?}", req.path));
        }

        let mut device_name: Option<String> = None;
        let mut device_id: Option<String> = None;
        let mut device_state: Option<String> = None;
        let mut port: Option<u16> = None;
        let mut zone_id: Option<String> = None;
        let mut creator: Option<String> = None;
        let mut ip_address: Option<IpAddr> = None;
        let mut color_code: Option<String> = None;
        let mut firmware_version: Option<String> = None;
        let mut stereo_pair_id: Option<String> = None;

        for header in req.headers {
            match header.name {
                "DeviceName" => {
                    device_name = Some(String::from_utf8_lossy(header.value).to_string());
                }
                "DeviceID" => {
                    device_id = Some(String::from_utf8_lossy(header.value).to_string());
                }
                "DeviceState" => {
                    device_state = Some(String::from_utf8_lossy(header.value).to_string());
                }
                "PORT" => {
                    port = Some(
                        String::from_utf8_lossy(header.value).as_ref().parse::<u16>()
                            .context("invalid port number")?,
                    );
                }
                "ZoneID" => {
                    zone_id = Some(String::from_utf8_lossy(header.value).to_string());
                }
                "Creator" => {
                    creator = Some(String::from_utf8_lossy(header.value).to_string());
                }
                "IPAddr" => {
                    ip_address = Some(
                        String::from_utf8_lossy(header.value)
                            .parse()
                            .context("invalid IP address")?,
                    );
                }
                "ColorCode" => {
                    color_code = Some(String::from_utf8_lossy(header.value).to_string());
                }
                "FWVersion" => {
                    firmware_version = Some(String::from_utf8_lossy(header.value).to_string());
                }
                "StereoPairID" => {
                    stereo_pair_id = Some(String::from_utf8_lossy(header.value).to_string());
                }
                _ => {
                    continue;
                }
            }
        }

        Ok(DiscoveryReply {
            device_name: device_name.ok_or_else(|| anyhow!("missing DeviceName header"))?,
            device_id: device_id.ok_or_else(|| anyhow!("missing DeviceID header"))?,
            device_state: device_state.ok_or_else(|| anyhow!("missing DeviceState header"))?,
            port: port.ok_or_else(|| anyhow!("missing PORT header"))?,
            zone_id: zone_id.ok_or_else(|| anyhow!("missing ZoneID header"))?,
            creator: creator.ok_or_else(|| anyhow!("missing Creator header"))?,
            ip_address: ip_address.ok_or_else(|| anyhow!("missing IPAddr header"))?,
            color_code: color_code.ok_or_else(|| anyhow!("missing ColorCode header"))?,
            firmware_version: firmware_version
                .ok_or_else(|| anyhow!("missing FWVersion header"))?,
            stereo_pair_id: stereo_pair_id.ok_or_else(|| anyhow!("missing StereoPairID header"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &'static str = "NOTIFY * HTTP/1.1 \r\n\
HOST: 239.255.255.250:1800\r\n\
PROTOCOL: Version 1.0\r\n\
NTS: ssdp-alive\r\n\
DeviceName: Device Name_9999-H0020000-07-12345\r\n\
DeviceID: 0123456789ab\r\n\
DeviceState: F,S,P\r\n\
PORT: 7777\r\n\
ZoneID: \r\n\
Creator: \r\n\
IPAddr: 192.168.178.75\r\n\
ColorCode: 2003\r\n\
FWVersion: 809;1,1;1,1\r\n\
StereoPairID: ";

    #[test]
    fn parse_test() {
        use std::net::{IpAddr, Ipv4Addr};

        assert_eq!(
            DiscoveryReply::parse(DATA.as_bytes()).unwrap(),
            DiscoveryReply {
                device_name: "Device Name_9999-H0020000-07-12345".to_string(),
                device_id: "0123456789ab".to_string(),
                device_state: "F,S,P".to_string(),
                port: 7777,
                zone_id: "".to_string(),
                creator: "".to_string(),
                ip_address: IpAddr::V4(Ipv4Addr::new(192, 168, 178, 75)),
                color_code: "2003".to_string(),
                firmware_version: "809;1,1;1,1".to_string(),
                stereo_pair_id: "".to_string(),
            },
        );
    }
}
