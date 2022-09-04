use std::net::Ipv4Addr;
use std::convert::TryFrom;

use enet::*;

#[derive(Debug, PartialEq)]
enum MatchmakingMessageType {
    CreateTicket,
    CreateTicketResponse,
    GetTicketResponse,
}

impl TryFrom<&str> for MatchmakingMessageType {
    type Error = ();

    fn try_from(input: &str) -> Result<MatchmakingMessageType, Self::Error> {
        match input {
            "create-ticket" => Ok(MatchmakingMessageType::CreateTicket),
            "create-ticket-resp" => Ok(MatchmakingMessageType::CreateTicketResponse),
            "get-ticket-resp" => Ok(MatchmakingMessageType::GetTicketResponse),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone)]
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

                let parsed_json = json::parse(packet_data).unwrap();
                let message_type = MatchmakingMessageType::try_from(
                    parsed_json["type"].as_str().unwrap()
                );
                let play_mode = OnlinePlayMode::try_from(
                    parsed_json["search"]["mode"].as_i64().unwrap()
                ).unwrap();

                match message_type {
                    Ok(MatchmakingMessageType::CreateTicket) => {
                        println!("create-ticket for {:?}", play_mode);
                        sender.send_packet(
                            Packet::new(
                                &json::stringify(json::object!{"type": "create-ticket-resp"}).into_bytes(),
                                PacketMode::ReliableSequenced
                            ).unwrap(),
                            channel_id,
                        ).unwrap()
                    },
                    Ok(MatchmakingMessageType::CreateTicketResponse) => println!("create-ticket-resp"),
                    Ok(MatchmakingMessageType::GetTicketResponse) => println!("get-ticket-resp"),
                    _ => (),
                }
            },
            _ => (),
        }
    }
}
