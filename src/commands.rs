use std::net::IpAddr;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::protocol;

pub(crate) const COMMAND_TYPE_FETCH: u8 = 1;
pub(crate) const COMMAND_TYPE_SET: u8 = 2;

pub enum CommandType {
    Fetch,
    Set,
}

pub trait Command<RequestDataType, ResponseDataType: std::fmt::Debug> {
    const GET_COMMAND_ID: u16;
    const SET_COMMAND_ID: u16;
    const NOTIFY_ID: u16;
    const NAME: &'static str;

    fn marshal_data(_: RequestDataType) -> Vec<u8> {
        vec![]
    }
    fn unmarshal_data(_: &[u8]) -> Result<ResponseDataType>;

    fn packet(command_type: CommandType, data: Option<RequestDataType>) -> protocol::Packet {
        protocol::Packet {
            command_type: match command_type {
                CommandType::Fetch => 1,
                CommandType::Set => 2,
            },
            command: match command_type {
                CommandType::Fetch => Self::GET_COMMAND_ID,
                CommandType::Set => Self::SET_COMMAND_ID,
            },
            command_data: data.map(Self::marshal_data),
        }
    }

    fn fetch() -> protocol::Packet {
        Self::packet(CommandType::Fetch, None)
    }

    fn set(data: RequestDataType) -> protocol::Packet {
        Self::packet(CommandType::Set, Some(data))
    }

    fn format_notification(p: &protocol::Packet) -> String {
        assert_eq!(Self::NOTIFY_ID, p.command);
        Self::format(p)
    }

    fn format_reply(p: &protocol::Packet) -> String {
        assert_eq!(Self::GET_COMMAND_ID, p.command);
        Self::format(p)
    }

    fn format(p: &protocol::Packet) -> String {
        let command_type = match p.command_type {
            COMMAND_TYPE_FETCH => "fetch",
            COMMAND_TYPE_SET => "set",
            _ => "??",
        };

        let unmarshal_result = match &p.command_data {
            Some(data) => Self::unmarshal_data(&data)
                .map(|x| format!("{:?}", x))
                .unwrap_or_else(|err| format!("{:?}", err)),
            None => "".to_string(),
        };

        format!("{} {} {:?}", command_type, Self::NAME, unmarshal_result)
    }
}

pub fn format_reply(p: &protocol::Packet) -> String {
    match p.command {
        Capabilities::GET_COMMAND_ID => Capabilities::format_reply(p),
        DeviceName::GET_COMMAND_ID => DeviceName::format_reply(p),
        FirmwareUpdate::GET_COMMAND_ID => FirmwareUpdate::format_reply(p),
        PlayControl::GET_COMMAND_ID => PlayControl::format_reply(p),
        Volume::GET_COMMAND_ID => Volume::format_notification(p),
        _ => format!("{:?}", p),
    }
}

pub fn format_notification(p: &protocol::Packet) -> String {
    match p.command {
        FirmwareUpdate::NOTIFY_ID => FirmwareUpdate::format_notification(p),
        PlayControl::NOTIFY_ID => PlayControl::format_notification(p),
        PlayInfo::NOTIFY_ID => PlayInfo::format_notification(p),
        Power::NOTIFY_ID => Power::format_notification(p),
        PowerMode::NOTIFY_ID => PowerMode::format_notification(p),
        Volume::NOTIFY_ID => Volume::format_notification(p),
        _ => format!("{:?}", p),
    }
}

pub enum PowerState {
    Sleep,
    WakeUp,
}

pub struct Power;

impl Command<PowerState, ()> for Power {
    const GET_COMMAND_ID: u16 = 15;
    const SET_COMMAND_ID: u16 = 15;
    const NOTIFY_ID: u16 = 15;
    const NAME: &'static str = "Power";

    fn marshal_data(s: PowerState) -> Vec<u8> {
        match s {
            PowerState::Sleep => "02",
            PowerState::WakeUp => "00",
        }
        .as_bytes()
        .to_vec()
    }

    fn unmarshal_data(_: &[u8]) -> Result<()> {
        Ok(())
    }
}

pub struct DeviceName;

impl Command<String, String> for DeviceName {
    const GET_COMMAND_ID: u16 = 90;
    const SET_COMMAND_ID: u16 = 90;
    const NOTIFY_ID: u16 = 0;
    const NAME: &'static str = "Name";

    fn marshal_data(name: String) -> Vec<u8> {
        name.as_bytes().to_vec()
    }

    fn unmarshal_data(data: &[u8]) -> Result<String> {
        Ok(String::from_utf8_lossy(data).to_string())
    }
}

pub struct Volume;

impl Command<u8, u8> for Volume {
    const GET_COMMAND_ID: u16 = 64;
    const SET_COMMAND_ID: u16 = 64;
    const NOTIFY_ID: u16 = 64;
    const NAME: &'static str = "Volume";

    fn marshal_data(volume: u8) -> Vec<u8> {
        if volume > 100 {
            panic!("volume cannot be greater than 100")
        }

        volume.to_string().as_bytes().to_vec()
    }

    fn unmarshal_data(data: &[u8]) -> Result<u8> {
        String::from_utf8_lossy(data)
            .parse()
            .map_err(|_| anyhow!("invalid volume data"))
    }
}

pub fn hello(our_addr: &IpAddr) -> protocol::Packet {
    protocol::Packet {
        command_type: COMMAND_TYPE_SET,
        command: 3,
        command_data: Some(
            format!("{},{}", our_addr, protocol::NOTIF_RECV_PORT)
                .as_bytes()
                .to_vec(),
        ),
    }
}

#[derive(Debug)]
pub enum PlayControlCommand {
    Play,
    Stop,
    Pause,
    Next,
    Previous,
    Toggle,
    Mute,
    Unmute,
}

pub struct PlayControl;

impl Command<PlayControlCommand, PlayControlCommand> for PlayControl {
    const GET_COMMAND_ID: u16 = 51;
    const SET_COMMAND_ID: u16 = 40;
    const NOTIFY_ID: u16 = 51;
    const NAME: &'static str = "Play control";

    fn marshal_data(cmd: PlayControlCommand) -> Vec<u8> {
        match cmd {
            PlayControlCommand::Play => "PLAY",
            PlayControlCommand::Stop => "STOP",
            PlayControlCommand::Pause => "PAUSE",
            PlayControlCommand::Next => "NEXT",
            PlayControlCommand::Previous => "PREV",
            PlayControlCommand::Toggle => "TOGGL",
            PlayControlCommand::Mute => "MUTE",
            PlayControlCommand::Unmute => "UNMUTE",
        }
        .as_bytes()
        .to_vec()
    }

    fn unmarshal_data(data: &[u8]) -> Result<PlayControlCommand> {
        // data is the ASCII code for a digit that is the 0 based command
        // index in the list above
        match data {
            &[48] => Ok(PlayControlCommand::Play),
            &[49] => Ok(PlayControlCommand::Stop),
            &[50] => Ok(PlayControlCommand::Pause),
            &[51] => Ok(PlayControlCommand::Next),
            &[52] => Ok(PlayControlCommand::Previous),
            &[53] => Ok(PlayControlCommand::Toggle),
            &[54] => Ok(PlayControlCommand::Mute),
            &[55] => Ok(PlayControlCommand::Unmute),
            &[other] => Err(anyhow!("invalid data: {}", other)),
            _ => Err(anyhow!("invalid data length")),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PlayInfoData {
    #[serde(rename(deserialize = "isFromChannel"))]
    is_from_channel: bool,
    play_album: Option<String>,
    play_album_uri: Option<String>,
    play_artist: Option<String>,
    play_attribution: Option<String>,
    play_identity: Option<String>,
    play_object: Option<String>,
    play_pic: Option<String>,
    play_preset_available: Option<i32>,
    play_subtitle: Option<String>,
    play_title: Option<String>,
    play_type: Option<String>,
    play_username: Option<String>,
    play_token: Option<String>,
}

pub struct PlayInfo;

impl Command<(), PlayInfoData> for PlayInfo {
    const SET_COMMAND_ID: u16 = 277;
    const GET_COMMAND_ID: u16 = 278;
    const NOTIFY_ID: u16 = 278;
    const NAME: &'static str = "Play info";

    fn marshal_data(_: ()) -> Vec<u8> {
        vec![] // for now
    }

    fn unmarshal_data(data: &[u8]) -> Result<PlayInfoData> {
        serde_json::from_slice(data).context("error parsing JSON")
    }
}

#[derive(Debug, Deserialize)]
pub struct Capability {
    name: String,
}

#[derive(Debug, Deserialize)]
pub struct CapabilitiesData {
    capabilities: Vec<Capability>,
}

pub struct Capabilities;

impl Command<(), CapabilitiesData> for Capabilities {
    const SET_COMMAND_ID: u16 = 0;
    const GET_COMMAND_ID: u16 = 281;
    const NOTIFY_ID: u16 = 0;
    const NAME: &'static str = "Capabilities";

    fn marshal_data(_: ()) -> Vec<u8> {
        vec![] // for now
    }

    fn unmarshal_data(data: &[u8]) -> Result<CapabilitiesData> {
        serde_json::from_slice(data).context("error parsing JSON")
    }
}

pub struct PowerMode;

impl Command<(), String> for PowerMode {
    const SET_COMMAND_ID: u16 = 0;
    const GET_COMMAND_ID: u16 = 14;
    const NOTIFY_ID: u16 = 14;
    const NAME: &'static str = "Power mode";

    fn marshal_data(_: ()) -> Vec<u8> {
        vec![] // for now
    }

    fn unmarshal_data(data: &[u8]) -> Result<String> {
        Ok(String::from_utf8_lossy(data).to_string())
    }
}

pub struct FirmwareUpdate;

impl Command<(), String> for FirmwareUpdate {
    const SET_COMMAND_ID: u16 = 65;
    const GET_COMMAND_ID: u16 = 65;
    const NOTIFY_ID: u16 = 65;
    const NAME: &'static str = "FM Update";

    fn marshal_data(_: ()) -> Vec<u8> {
        vec![] // for now
    }

    fn unmarshal_data(data: &[u8]) -> Result<String> {
        Ok(String::from_utf8_lossy(data).to_string())
    }
}
