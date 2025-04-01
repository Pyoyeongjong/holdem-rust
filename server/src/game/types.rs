use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PlayerAction {
    pub check: bool,
    pub call: bool,
    pub raise: bool,
    pub allin: bool,
    pub fold: bool,
}

impl PlayerAction {
    pub fn new() -> Self {
        Self {
            check: true,
            call: true,
            raise: true,
            allin: true,
            fold: true,
        }
    }
}
#[derive(Serialize, Deserialize, PartialEq)]
pub enum GameState {
    Init,
    BeforeStart,
    FreeFlop,
    Flop,
    River,
    Turn,
    ShowDown,
}