use std::net::Ipv4Addr;
use std::convert::TryFrom;

use serde_derive::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use encoding_rs::SHIFT_JIS;
use enet::*;

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct User {
    pub uid: String,
    pub play_key: String,
    pub display_name: String,
    pub connect_code: String,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Search {
    connect_code: Option<Vec<u8>>,
    mode: OnlinePlayMode
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Player {
    is_local_player: bool,
    port: i64,
    user: User,
}

#[derive(Debug, PartialEq, Copy, Clone, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
enum OnlinePlayMode {
    Ranked = 0,
    Unranked = 1,
    Direct = 2,
    Teams = 3,
}

impl TryFrom<i64> for OnlinePlayMode {
    type Error = ();

    fn try_from(value: i64) -> Result<OnlinePlayMode, Self::Error> {
        match value {
            0 => Ok(OnlinePlayMode::Ranked),
            1 => Ok(OnlinePlayMode::Unranked),
            2 => Ok(OnlinePlayMode::Direct),
            3 => Ok(OnlinePlayMode::Teams),
            _ => Err(()),
        }
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Deserialize, Serialize)]
enum Stage {
    FountainOfDreams = 0x2,
    PokemonStadium = 0x3,
    YoshisStory = 0x8,
    DreamLand = 0x1C,
    Battlefield = 0x1F,
    FinalDestination = 0x20,
}

impl TryFrom<i64> for Stage {
    type Error = ();

    fn try_from(value: i64) -> Result<Stage, Self::Error> {
        match value {
            0x2 => Ok(Stage::FountainOfDreams),
            0x3 => Ok(Stage::PokemonStadium),
            0x8 => Ok(Stage::YoshisStory),
            0x1C => Ok(Stage::DreamLand),
            0x1F => Ok(Stage::Battlefield),
            0x20 => Ok(Stage::FinalDestination),
            _ => Err(())
        }
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type")]
enum MatchmakingMessage {
    #[serde(rename = "create-ticket", rename_all = "camelCase")]
    CreateTicket {
        app_version: String,
        ip_address_lan: String,
        search: Search,
        user: User,
    },
    #[serde(rename = "create-ticket-resp")]
    CreateTicketResponse {},
    #[serde(rename = "get-ticket-resp")]
    GetTicketResponse {
        latest_version: String,
        match_id: String,
        is_host: bool,
        players: Vec<Player>,
        stages: Vec<Stage>,
    },
}

pub fn start_server(host: Ipv4Addr, port: u16) {
    let enet = Enet::new().expect("Could not initialize ENet");
    let listen_address = Address::new(host, port);
    let mut host = enet
        .create_host::<()>(
            Some(&listen_address),
            10,
            ChannelLimit::Maximum,
            BandwidthLimit::Unlimited,
            BandwidthLimit::Unlimited,
        )
        .expect("Could not create host");

    loop {
        match host.service(1000).expect("ENet service failed") {
            Some(Event::Connect(_)) => println!("new connection!"),
            Some(Event::Disconnect(..)) => println!("disconnect!"),
            Some(Event::Receive {
                channel_id,
                ref packet,
                ref mut sender,
            }) => {
                let packet_data = std::str::from_utf8(packet.data()).unwrap();

                println!(
                    "got packet on channel {}, content: '{}'",
                    channel_id,
                    packet_data,
                );

                let message: MatchmakingMessage = serde_json::from_str(packet_data).unwrap();

                match message {
                    MatchmakingMessage::CreateTicket{
                        search,
                        ..
                    } => {
                        match search.mode {
                            OnlinePlayMode::Direct => {
                                let connect_code_enc = search.connect_code.unwrap();
                                let (connect_code, _enc, _errors) = SHIFT_JIS.decode(&connect_code_enc);
                                println!("create-ticket for {:?} {}", search.mode, connect_code);
                            }
                            _ => {
                                println!("create-ticket for {:?}", search.mode);
                            }
                        }

                        sender.send_packet(
                            Packet::new(
                                &serde_json::to_string(&MatchmakingMessage::CreateTicketResponse {}).unwrap().into_bytes(),
                                PacketMode::ReliableSequenced
                            ).unwrap(),
                            channel_id,
                        ).unwrap()
                    },
                    MatchmakingMessage::CreateTicketResponse {..} => println!("create-ticket-resp"),
                    MatchmakingMessage::GetTicketResponse {..} => println!("get-ticket-resp"),
                }
            },
            _ => (),
        }
    }
}

#[test]
fn can_parse_create_ticket_direct_message() {
    let message: MatchmakingMessage = serde_json::from_str(r#"
        {
            "type": "create-ticket",
            "appVersion": "2.5.1",
            "ipAddressLan": "127.0.0.2:50285",
            "search": {
                "connectCode": [130,120,130,116,130,108,130,104,129,148,130,84,130,84,130,87],
                "mode": 2
            },
            "user": {
                "connectCode": "TEST#001",
                "displayName": "test",
                "playKey": "1",
                "uid": "1"
            }
        }
    "#).unwrap();

    match message {
        MatchmakingMessage::CreateTicket{app_version, ..} =>
            assert_eq!(app_version, "2.5.1"),
        _ => assert_eq!(1, 2),
    }
}

#[test]
fn can_parse_create_ticket_unranked_message() {
    let message: MatchmakingMessage = serde_json::from_str(r#"
        {
            "type": "create-ticket",
            "appVersion": "2.5.1",
            "ipAddressLan": "127.0.0.2:51000",
            "search": {
                "mode": 1
            },
            "user": {
                "connectCode": "TEST#001",
                "displayName": "test",
                "playKey": "1",
                "uid": "1"
            }
        }
    "#).unwrap();

    match message {
        MatchmakingMessage::CreateTicket{app_version, ..} =>
            assert_eq!(app_version, "2.5.1"),
        _ => assert_eq!(1, 2),
    }
}
