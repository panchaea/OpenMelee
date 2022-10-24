use std::fmt;

use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum ControllerPort {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
}

impl ControllerPort {
    pub fn get_ports(mode: OnlinePlayMode) -> Vec<ControllerPort> {
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
pub enum OnlinePlayMode {
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
pub enum Stage {
    FountainOfDreams = 0x2,
    PokemonStadium = 0x3,
    YoshisStory = 0x8,
    DreamLand = 0x1C,
    Battlefield = 0x1F,
    FinalDestination = 0x20,
}

impl Stage {
    pub fn get_allowed_stages(mode: OnlinePlayMode) -> Vec<Stage> {
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
