use std::collections::HashSet;
use std::fmt;

use chrono::Utc;
use encoding_rs::SHIFT_JIS;
use enet::*;
use itertools::Itertools;
use rand::{seq::IteratorRandom, seq::SliceRandom, thread_rng};
use serde::{de, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sqlx::SqlitePool;
use unicode_normalization::UnicodeNormalization;

use slippi_re::{Config, LATEST_SLIPPI_CLIENT_VERSION};

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

impl Player {
    fn new(
        ticket: CreateTicket,
        address: Address,
        is_local_player: bool,
        port: ControllerPort,
    ) -> Player {
        let CreateTicket {
            user,
            ip_address_lan,
            ..
        } = ticket;

        Player {
            uid: user.uid,
            display_name: user.display_name,
            connect_code: user.connect_code,
            ip_address: format!("{}:{}", address.ip(), address.port()),
            ip_address_lan,
            is_local_player,
            port,
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
enum ControllerPort {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
}

impl ControllerPort {
    fn get_ports(mode: OnlinePlayMode) -> Vec<ControllerPort> {
        match mode {
            OnlinePlayMode::Teams => vec![
                ControllerPort::One,
                ControllerPort::Two,
                ControllerPort::Three,
                ControllerPort::Four,
            ],
            _ => vec![ControllerPort::One, ControllerPort::Two],
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone, Deserialize_repr, Serialize_repr)]
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

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
enum Stage {
    FountainOfDreams = 0x2,
    PokemonStadium = 0x3,
    YoshisStory = 0x8,
    DreamLand = 0x1C,
    Battlefield = 0x1F,
    FinalDestination = 0x20,
}

impl Stage {
    fn get_allowed_stages(mode: OnlinePlayMode) -> Vec<Stage> {
        let mut allowed_stages = vec![
            Stage::PokemonStadium,
            Stage::YoshisStory,
            Stage::DreamLand,
            Stage::Battlefield,
            Stage::FinalDestination,
        ];
        match mode {
            OnlinePlayMode::Teams => allowed_stages,
            _ => {
                allowed_stages.push(Stage::FountainOfDreams);
                allowed_stages
            }
        }
    }
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

pub fn start_server(config: Config, _pool: SqlitePool) {
    let enet = Enet::new().expect("Could not initialize ENet");
    let listen_address = Address::new(config.matchmaking_server_address, config.matchmaking_port);
    let mut host = enet
        .create_host::<CreateTicket>(
            Some(&listen_address),
            config.matchmaking_max_peers,
            ChannelLimit::Maximum,
            BandwidthLimit::Unlimited,
            BandwidthLimit::Unlimited,
        )
        .expect("Could not create ENet host");

    println!(
        "Matchmaking server listening on udp://{}:{}",
        config.matchmaking_server_address, config.matchmaking_port
    );

    loop {
        host.service(1000)
            .expect("ENet service failed")
            .map(handle_enet_event);

        let connected_peers = host
            .peers()
            .filter(|peer| peer.state() == PeerState::Connected)
            .filter(|peer| peer.data().is_some());

        let peers_by_game_mode = connected_peers.group_by(|peer| peer.data().unwrap().search.mode);

        peers_by_game_mode.into_iter().for_each(|(mode, peers)| {
            handle_matchmaking(mode, peers.collect_vec());
        });
    }
}

fn handle_enet_event(mut event: Event<CreateTicket>) {
    match event {
        Event::Connect(_) => println!("New connection!"),
        Event::Disconnect(..) => println!("Disconnect!"),
        Event::Receive {
            ref packet,
            ref mut sender,
            ..
        } => {
            let packet_data = std::str::from_utf8(packet.data()).unwrap();
            let message: CreateTicket = serde_json::from_str(packet_data).unwrap();

            println!("{:?}", packet_data);

            sender.set_data(Some(message.clone()));

            match message.search.mode {
                OnlinePlayMode::Direct => {
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
    }
}

fn handle_matchmaking(mode: OnlinePlayMode, peers: Vec<Peer<CreateTicket>>) {
    let mut matched_peers: Vec<Vec<Peer<CreateTicket>>> = vec![];

    if mode == OnlinePlayMode::Direct {
        peers
            .iter()
            .group_by(|peer| {
                let CreateTicket { user, search, .. } = peer.data().unwrap();
                vec![
                    user.connect_code.clone(),
                    search.connect_code.clone().unwrap(),
                ]
                .into_iter()
                .collect::<HashSet<_>>()
            })
            .into_iter()
            .for_each(|(_, peer_group)| {
                let peer_vec = peer_group.collect_vec();
                if peer_vec.len() > 1 {
                    matched_peers.push(peer_vec.clone().into_iter().cloned().collect_vec());
                }
            });
    }

    matched_peers.iter().for_each(|_peers| {
        let messages = create_game(
            _peers
                .clone()
                .iter()
                .map(|peer| (peer.data().unwrap().clone(), peer.address()))
                .collect(),
            mode,
        );
        _peers
            .clone()
            .iter()
            .zip(messages)
            .map(|(_peer, message)| {
                let peer = &mut _peer.clone();
                let message_str = &serde_json::to_string(&message).unwrap();
                println!("Sending message: \n{:?}", message_str,);
                peer.send_packet(
                    Packet::new(
                        &message_str.clone().into_bytes(),
                        PacketMode::ReliableSequenced,
                    )
                    .unwrap(),
                    ENET_CHANNEL_ID,
                )
                .unwrap();
            })
            .for_each(drop);
    })
}

fn get_match_id(mode: OnlinePlayMode) -> String {
    let now = Utc::now();
    format!("mode.{}-{}", mode, now.to_rfc3339())
}

fn create_game(
    _players: Vec<(CreateTicket, Address)>,
    mode: OnlinePlayMode,
) -> Vec<MatchmakingMessage> {
    let mut rng = thread_rng();
    let match_id = get_match_id(mode);
    let stages = Stage::get_allowed_stages(mode);
    let ports = ControllerPort::get_ports(mode);
    let randomized_ports: Vec<ControllerPort> = ports
        .choose_multiple(&mut rng, _players.len())
        .cloned()
        .collect();
    let host_port = ports.iter().choose(&mut rng);

    _players
        .iter()
        .enumerate()
        .map(|(i, _)| MatchmakingMessage::GetTicketResponse {
            latest_version: LATEST_SLIPPI_CLIENT_VERSION.to_string(),
            match_id: match_id.clone(),
            is_host: *ports.get(i).unwrap() == *host_port.unwrap(),
            is_assigned: true,
            players: _players
                .clone()
                .iter()
                .enumerate()
                .map(|(j, (_ticket, _address))| {
                    Player::new(
                        _ticket.clone(),
                        _address.clone(),
                        i == j,
                        *randomized_ports.get(j).unwrap(),
                    )
                })
                .collect(),
            stages: stages.clone(),
        })
        .collect()
}

#[cfg(test)]
mod test {
    use std::collections::hash_map::Entry::{Occupied, Vacant};
    use std::collections::HashMap;
    use std::net::Ipv4Addr;

    use rand::Rng;

    use crate::matchmaking::*;

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
                    "connectCode": [130, 115, 130, 100, 130, 114, 130, 115, 129, 148, 130, 79, 130, 79, 130, 81],
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
        assert_eq!(search.connect_code.as_ref().unwrap(), "TEST#002");
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
    fn can_serialize_get_ticket_response_message() {
        let message = MatchmakingMessage::GetTicketResponse {
            latest_version: String::from(LATEST_SLIPPI_CLIENT_VERSION),
            match_id: get_match_id(OnlinePlayMode::Direct),
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
            stages: Stage::get_allowed_stages(OnlinePlayMode::Direct),
        };

        assert!(serde_json::to_string(&message).is_ok());
    }

    #[test]
    fn create_game_direct_mode() {
        let rng = &mut rand::thread_rng();
        let first_port = rng.gen_range(40000..50000);
        let second_port = rng.gen_range(40000..50000);
        let first_ticket = CreateTicket {
            app_version: String::from(LATEST_SLIPPI_CLIENT_VERSION),
            ip_address_lan: format!("127.0.0.1:{}", first_port),
            search: Search {
                mode: OnlinePlayMode::Direct,
                connect_code: Some(String::from("TEST#002")),
            },
            user: User {
                uid: String::from("1234"),
                play_key: String::from("5678"),
                display_name: String::from("test"),
                connect_code: String::from("TEST#001"),
            },
        };
        let first_address = Address::new(Ipv4Addr::LOCALHOST, first_port);
        let second_ticket = CreateTicket {
            app_version: String::from(LATEST_SLIPPI_CLIENT_VERSION),
            ip_address_lan: format!("127.0.0.1:{}", second_port),
            search: Search {
                mode: OnlinePlayMode::Direct,
                connect_code: Some(String::from("TEST#001")),
            },
            user: User {
                uid: String::from("4321"),
                play_key: String::from("8765"),
                display_name: String::from("test-2"),
                connect_code: String::from("TEST#002"),
            },
        };
        let second_address = Address::new(Ipv4Addr::LOCALHOST, second_port);

        let messages = create_game(
            vec![
                (first_ticket, first_address),
                (second_ticket, second_address),
            ],
            OnlinePlayMode::Direct,
        );

        assert_eq!(messages.len(), 2);

        let mut is_host_count = 0;
        let mut port_by_uid: HashMap<String, ControllerPort> = HashMap::new();
        let mut _stages: Option<Vec<Stage>> = None;
        messages.iter().for_each(|message| {
            if let MatchmakingMessage::GetTicketResponse {
                is_host,
                players,
                stages,
                ..
            } = message
            {
                // Stages should match in all messages
                match _stages.clone() {
                    Some(s) => assert_eq!(s, stages.clone()),
                    _ => _stages = Some(stages.clone()),
                }
                let mut is_local_player_count = 0;
                players.iter().for_each(|player| {
                    // Each player has the same port assignment in all messages
                    match port_by_uid.entry(player.uid.clone()) {
                        Occupied(p) => assert_eq!(player.port, p.get().clone()),
                        Vacant(v) => {
                            v.insert(player.port);
                        }
                    }
                    if player.is_local_player {
                        is_local_player_count += 1
                    }
                });

                // Each message has one local player
                assert_eq!(is_local_player_count, 1);

                if *is_host {
                    is_host_count += 1
                };
            } else {
                assert!(false);
            }
        });

        // Each set of messages has one message with is_host: true
        assert_eq!(is_host_count, 1);
    }

    #[test]
    fn test_get_allowed_stages_includes_battlefield_for_all_modes() {
        let unranked_stages = Stage::get_allowed_stages(OnlinePlayMode::Unranked);
        let ranked_stages = Stage::get_allowed_stages(OnlinePlayMode::Unranked);
        let direct_stages = Stage::get_allowed_stages(OnlinePlayMode::Direct);
        let teams_stages = Stage::get_allowed_stages(OnlinePlayMode::Teams);
        assert!(unranked_stages.contains(&Stage::Battlefield));
        assert!(ranked_stages.contains(&Stage::Battlefield));
        assert!(direct_stages.contains(&Stage::Battlefield));
        assert!(teams_stages.contains(&Stage::Battlefield));
    }

    #[test]
    fn test_get_allowed_stages_does_not_include_fountain_of_dreams_for_teams() {
        let stages = Stage::get_allowed_stages(OnlinePlayMode::Teams);
        assert!(!stages.contains(&Stage::FountainOfDreams));
        assert_eq!(stages.into_iter().unique().collect_vec().len(), 5);
    }

    #[test]
    fn test_get_allowed_stages_does_include_fountain_of_dreams_for_other_modes() {
        let unranked_stages = Stage::get_allowed_stages(OnlinePlayMode::Unranked);
        let ranked_stages = Stage::get_allowed_stages(OnlinePlayMode::Unranked);
        let direct_stages = Stage::get_allowed_stages(OnlinePlayMode::Direct);
        assert!(unranked_stages.contains(&Stage::FountainOfDreams));
        assert_eq!(unranked_stages.into_iter().unique().collect_vec().len(), 6);
        assert!(ranked_stages.contains(&Stage::FountainOfDreams));
        assert_eq!(ranked_stages.into_iter().unique().collect_vec().len(), 6);
        assert!(direct_stages.contains(&Stage::FountainOfDreams));
        assert_eq!(direct_stages.into_iter().unique().collect_vec().len(), 6);
    }

    #[test]
    fn test_get_ports_returns_correct_number_of_unique_ports() {
        let unranked_ports = ControllerPort::get_ports(OnlinePlayMode::Unranked);
        let ranked_ports = ControllerPort::get_ports(OnlinePlayMode::Ranked);
        let direct_ports = ControllerPort::get_ports(OnlinePlayMode::Direct);
        let teams_ports = ControllerPort::get_ports(OnlinePlayMode::Teams);
        assert_eq!(unranked_ports.into_iter().unique().collect_vec().len(), 2);
        assert_eq!(ranked_ports.into_iter().unique().collect_vec().len(), 2);
        assert_eq!(direct_ports.into_iter().unique().collect_vec().len(), 2);
        assert_eq!(teams_ports.into_iter().unique().collect_vec().len(), 4);
    }
}
