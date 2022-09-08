use std::collections::HashSet;
use std::fmt;
use std::net::Ipv4Addr;

use encoding_rs::SHIFT_JIS;
use enet::*;
use itertools::Itertools;
use rand::seq::SliceRandom;
use serde::{de, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use unicode_normalization::UnicodeNormalization;

use slippi_re::LATEST_SLIPPI_CLIENT_VERSION;

const ENET_CHANNEL_ID: u8 = 0;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct User {
    uid: String,
    play_key: String,
    display_name: String,
    connect_code: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Search {
    #[serde(default)]
    #[serde(
        deserialize_with = "shift_jis_code_point_array_to_string",
        rename = "connectCode"
    )]
    connect_code: Option<String>,
    mode: OnlinePlayMode,
}

fn shift_jis_code_point_array_to_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let v: Vec<u8> = Deserialize::deserialize(deserializer)?;
    let (connect_code, _enc, _errors) = SHIFT_JIS.decode(&v);
    Ok(Some(connect_code.to_string().nfkc().collect::<String>()))
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Player {
    is_local_player: bool,
    ip_address: String,
    ip_address_lan: String,
    port: ControllerPort,
    uid: String,
    display_name: String,
    connect_code: String,
}

#[derive(Debug, PartialEq, Copy, Clone, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
enum ControllerPort {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
}

#[derive(Debug, PartialEq, Copy, Clone, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
enum OnlinePlayMode {
    Ranked = 0,
    Unranked = 1,
    Direct = 2,
    Teams = 3,
}

impl fmt::Display for OnlinePlayMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let string = match &self {
            OnlinePlayMode::Ranked => "ranked",
            OnlinePlayMode::Unranked => "unranked",
            OnlinePlayMode::Direct => "direct",
            OnlinePlayMode::Teams => "teams",
        };
        write!(f, "{}", string)
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
enum Stage {
    FountainOfDreams = 0x2,
    PokemonStadium = 0x3,
    YoshisStory = 0x8,
    DreamLand = 0x1C,
    Battlefield = 0x1F,
    FinalDestination = 0x20,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename = "create-ticket", rename_all = "camelCase")]
struct CreateTicket {
    app_version: String,
    ip_address_lan: String,
    search: Search,
    user: User,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum MatchmakingMessage {
    #[serde(rename = "create-ticket-resp", rename_all = "camelCase")]
    CreateTicketResponse {},
    #[serde(rename = "get-ticket-resp", rename_all = "camelCase")]
    GetTicketResponse {
        latest_version: String,
        match_id: String,
        is_host: bool,
        is_assigned: bool,
        players: Vec<Player>,
        stages: Vec<Stage>,
    },
}

pub fn start_server(host: Ipv4Addr, port: u16) {
    let enet = Enet::new().expect("Could not initialize ENet");
    let listen_address = Address::new(host, port);
    let mut host = enet
        .create_host::<CreateTicket>(
            Some(&listen_address),
            10,
            ChannelLimit::Maximum,
            BandwidthLimit::Unlimited,
            BandwidthLimit::Unlimited,
        )
        .expect("Could not create host");

    loop {
        match host.service(1000).expect("ENet service failed") {
            Some(Event::Connect(_)) => println!("New connection!"),
            Some(Event::Disconnect(..)) => println!("Disconnect!"),
            Some(Event::Receive {
                ref packet,
                ref mut sender,
                ..
            }) => {
                let packet_data = std::str::from_utf8(packet.data()).unwrap();
                let message: CreateTicket = serde_json::from_str(packet_data).unwrap();

                println!("{:?}", packet_data);

                match message.search.mode {
                    OnlinePlayMode::Direct => {
                        sender.set_data(Some(message.clone()));
                        sender
                            .send_packet(
                                Packet::new(
                                    &serde_json::to_string(
                                        &MatchmakingMessage::CreateTicketResponse {},
                                    )
                                    .unwrap()
                                    .into_bytes(),
                                    PacketMode::ReliableSequenced,
                                )
                                .unwrap(),
                                ENET_CHANNEL_ID,
                            )
                            .unwrap();
                    }
                    _ => {
                        println!("Play mode {:?} not implemented", message.search.mode);
                        sender.disconnect_later(0);
                    }
                }
            }
            _ => (),
        }

        host.peers()
            .collect_vec()
            .clone()
            .into_iter()
            .filter(|peer| peer.state() == PeerState::Connected)
            .filter(|peer| match peer.data() {
                Some(CreateTicket { search, .. }) => search.mode == OnlinePlayMode::Direct,
                _ => false,
            })
            .group_by(|peer| {
                let CreateTicket { user, search, .. } = peer.data().unwrap();
                return vec![
                    user.connect_code.clone(),
                    search.connect_code.clone().unwrap(),
                ]
                .into_iter()
                .collect::<HashSet<_>>();
            })
            .into_iter()
            .filter_map(|(_, all_peers)| {
                let vec = all_peers.collect_vec().clone();
                match vec.len() {
                    2 => Some(vec),
                    _ => None,
                }
            })
            .for_each(|all_peers| {
                for (mut first, mut second) in all_peers.clone().into_iter().tuples() {
                    let (first_message, second_message) =
                        create_game(first.clone(), second.clone(), vec![Stage::FinalDestination]);

                    let first_message_str = &serde_json::to_string(&first_message).unwrap();
                    let second_message_str = &serde_json::to_string(&second_message).unwrap();
                    println!(
                        "Sending messages: \n{:?}\n{:?}",
                        first_message_str, second_message_str
                    );

                    first
                        .send_packet(
                            Packet::new(
                                &first_message_str.clone().into_bytes(),
                                PacketMode::ReliableSequenced,
                            )
                            .unwrap(),
                            ENET_CHANNEL_ID,
                        )
                        .unwrap();
                    first.set_data(None);

                    second
                        .send_packet(
                            Packet::new(
                                &second_message_str.clone().into_bytes(),
                                PacketMode::ReliableSequenced,
                            )
                            .unwrap(),
                            ENET_CHANNEL_ID,
                        )
                        .unwrap();
                    second.set_data(None);
                }
            });
    }
}

fn create_game(
    first: Peer<CreateTicket>,
    second: Peer<CreateTicket>,
    stages: Vec<Stage>,
) -> (MatchmakingMessage, MatchmakingMessage) {
    let CreateTicket {
        user: first_user,
        ip_address_lan: first_ip_address_lan,
        ..
    } = first.data().unwrap();

    let CreateTicket {
        user: second_user,
        ip_address_lan: second_ip_address_lan,
        ..
    } = second.data().unwrap();

    let mut rng = &mut rand::thread_rng();
    let ports = vec![ControllerPort::One, ControllerPort::Two];
    let (first_port, second_port) = ports
        .choose_multiple(&mut rng, 2)
        .cloned()
        .tuples()
        .next()
        .unwrap();

    let first_message = MatchmakingMessage::GetTicketResponse {
        latest_version: LATEST_SLIPPI_CLIENT_VERSION.to_string(),
        match_id: "123456789".to_string(),
        is_host: true,
        is_assigned: true,
        players: vec![
            Player {
                is_local_player: false,
                uid: second_user.uid.to_string(),
                display_name: second_user.display_name.to_string(),
                connect_code: second_user.connect_code.to_string(),
                ip_address: format!(
                    "{}:{}",
                    second.address().ip().to_string(),
                    second.address().port().to_string()
                ),
                ip_address_lan: second_ip_address_lan.to_string(),
                port: second_port,
            },
            Player {
                is_local_player: true,
                uid: first_user.uid.to_string(),
                display_name: first_user.display_name.to_string(),
                connect_code: first_user.connect_code.to_string(),
                ip_address: format!(
                    "{}:{}",
                    first.address().ip().to_string(),
                    first.address().port().to_string()
                ),
                ip_address_lan: first_ip_address_lan.to_string(),
                port: first_port,
            },
        ],
        stages: stages.clone(),
    };

    let second_message = MatchmakingMessage::GetTicketResponse {
        latest_version: LATEST_SLIPPI_CLIENT_VERSION.to_string(),
        match_id: "123456789".to_string(),
        is_host: false,
        is_assigned: true,
        players: vec![
            Player {
                is_local_player: true,
                uid: second_user.uid.to_string(),
                display_name: second_user.display_name.to_string(),
                connect_code: second_user.connect_code.to_string(),
                ip_address: format!(
                    "{}:{}",
                    second.address().ip().to_string(),
                    second.address().port().to_string()
                ),
                ip_address_lan: second_ip_address_lan.to_string(),
                port: second_port,
            },
            Player {
                is_local_player: false,
                uid: first_user.uid.to_string(),
                display_name: first_user.display_name.to_string(),
                connect_code: first_user.connect_code.to_string(),
                ip_address: format!(
                    "{}:{}",
                    first.address().ip().to_string(),
                    first.address().port().to_string()
                ),
                ip_address_lan: first_ip_address_lan.to_string(),
                port: first_port,
            },
        ],
        stages: stages.clone(),
    };

    (first_message, second_message)
}

#[test]
fn can_parse_create_ticket_direct_message() {
    let CreateTicket {
        app_version,
        search,
        ..
    } = serde_json::from_str(
        r#"
        {
            "type": "create-ticket",
            "appVersion": "2.5.1",
            "ipAddressLan": "127.0.0.2:50285",
            "search": {
                "connectCode": [130, 115, 130, 100, 130, 114, 130, 115, 129, 148, 130, 79, 130, 79, 130, 80],
                "mode": 2
            },
            "user": {
                "connectCode": "TEST#001",
                "displayName": "test",
                "playKey": "1",
                "uid": "1"
            }
        }
    "#,
    )
    .unwrap();

    assert_eq!(app_version, "2.5.1");
    assert_eq!(search.connect_code.as_ref().unwrap(), "TEST#001");
}

#[test]
fn can_parse_create_ticket_unranked_message() {
    let CreateTicket { app_version, .. } = serde_json::from_str(
        r#"
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
    "#,
    )
    .unwrap();

    assert_eq!(app_version, "2.5.1");
}

#[test]
fn can_create_get_ticket_response_message() {
    let _message = MatchmakingMessage::GetTicketResponse {
        latest_version: String::from(LATEST_SLIPPI_CLIENT_VERSION),
        match_id: String::from("1"),
        is_host: false,
        is_assigned: true,
        players: vec![Player {
            is_local_player: false,
            uid: String::from("1"),
            display_name: String::from("test"),
            connect_code: String::from("TEST#001"),
            ip_address: String::from("127.0.0.1:48593"),
            ip_address_lan: String::from("127.0.0.1:48593"),
            port: ControllerPort::One,
        }],
        stages: vec![Stage::FountainOfDreams],
    };
}
