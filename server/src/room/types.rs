use std::net::SocketAddr;

use futures_channel::mpsc::UnboundedSender;
use serde::{Serialize, Deserialize};
use tokio_tungstenite::tungstenite::Message;

use crate::utils::config::{Tx, MAX_PLAYER};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    sub: String,
    exp: usize,
}

pub struct TxInfo {
    pub tx: Tx,
    pub room_id: Option<usize>
}

#[derive(Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: usize,
    pub name: String,
    pub max_player: usize,
    pub cur_player: usize,
    pub bb: usize,
}

impl RoomInfo {
    pub fn new(id: usize, name: String, bb: usize) -> RoomInfo {
        RoomInfo { id, name, max_player: MAX_PLAYER, cur_player: 0, bb }
    }
}

impl Clone for RoomInfo {
    fn clone(&self) -> Self {
        RoomInfo {
            id: self.id,
            name: self.name.clone(),
            max_player: self.max_player,
            cur_player: self.cur_player,
            bb: self.bb
        }
    }
}

// 정보 보내기용
pub struct PlayerInfo{
    pub _id: String,
    pub name: String,
    pub addr: SocketAddr,
    pub chips: usize,
}

pub enum GameRequest {
    Command { cmd: GameCommand, id: String},
    AddPlayer { info: PlayerInfo, socket: UnboundedSender<Message>},
    RemovePlayer { addr: SocketAddr }
}

pub enum GameCommand {
    StartGame, 
    Check,
    Call,
    Raise(usize),
    AllIn,
    Fold,
}

pub enum GameResponse {}