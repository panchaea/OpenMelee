use std::net::Ipv4Addr;

use encoding_rs::SHIFT_JIS;
use enet::*;
use serde::{de, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct User {
    uid: String,
    play_key: String,
    display_name: String,
    connect_code: String,
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
    port: u16,
    uid: String,
    display_name: String,
    connect_code: String,
}

#[derive(Debug, PartialEq, Copy, Clone, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
enum OnlinePlayMode {
    Ranked = 0,
    Unranked = 1,
    Direct = 2,
    Teams = 3,
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

#[test]
fn can_create_get_ticket_response_message() {
    let message = MatchmakingMessage::GetTicketResponse {
        latest_version: String::from("2.5.1"),
        match_id: String::from("1"),
        is_host: false,
        players: vec![
            Player {
                is_local_player: false,
                uid: String::from("1"),
                display_name: String::from("test"),
                connect_code: String::from("TEST#001"),
                port: 45000,
            }
        ],
        stages: vec![
            Stage::FountainOfDreams
        ]
    };
}
