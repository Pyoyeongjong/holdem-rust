use std::net::SocketAddr;
use crate::db;
use futures_channel::mpsc::UnboundedSender;

// 

use tokio_tungstenite::tungstenite::Message;

// Player는 room에 종속된 구조체 -> room 바깥에서 생성되거나 따로 관장될 수 없음.
pub struct Player {
    pub id: String,
    pub chips: usize,
    pub state: PlayerState,
    pub hands: Option<(String, String)>, // 있을 수도 있고 없을 수도 있으니까
    pub player_pot: usize, // 변수 명 바꾸고 싶은데
    // 웹소켓 소통용 (PeerMap 역할)
    pub addr: SocketAddr,
    pub tx: UnboundedSender<Message>, // 이 플레이어에게 보낼 res을 기다리는 쓰레드에게 전송하는 역할
}

#[derive(PartialEq, Debug)]
pub enum PlayerState {
    Idle, Check, Call, Raise,
    Fold, AllIn, Waiting, Winner,
}

#[allow(dead_code)]
impl Player {
    pub fn print_current_state(&self) {
        println!("Player State is {:?}", self.state);
    }
}

impl Player {
    pub fn new(name: String, chips: usize, addr: SocketAddr, tx: UnboundedSender<Message>) -> Player {
        Player {
            id: name,
            chips,
            state: PlayerState::Waiting,
            hands: None,
            player_pot: 0,
            addr,
            tx
        }
    }

    pub fn is_acted(&self) -> bool {
        match self.state {
            PlayerState::Check => true,
            PlayerState::Call => true,
            PlayerState::Raise => true,
            PlayerState::AllIn => true,
            _ => false,
        }
    }

    pub fn should_return_to_idle(&self) -> bool {
        match self.state {
            PlayerState::Check => true,
            PlayerState::Call => true,
            PlayerState::Raise => true,
            _ => false,
        }
    }

    pub fn is_alive(&self) -> bool {
        match self.state {
            PlayerState::Idle => true,
            PlayerState::Check => true,
            PlayerState::Call => true,
            PlayerState::Raise => true,
            PlayerState::AllIn => true,
            PlayerState::Winner => true,
            _ => false,
        }
    }

    pub fn check(&mut self) {
        self.state = PlayerState::Check;
    }

    pub fn call(&mut self, size: usize) {
        assert!(self.chips >= size);
        self.chips -= size;
        self.player_pot += size;
        self.state = PlayerState::Call;
    }

    pub fn allin(&mut self) {
        assert!(self.chips > 0);
        self.player_pot += self.chips;
        self.chips = 0;
        self.state = PlayerState::AllIn; 
    }

    pub fn raise(&mut self, size: usize) -> bool {

        assert!(self.chips >= size);

        self.chips -= size;
        self.player_pot += size;

        if self.chips == 0 {
            self.state = PlayerState::AllIn;
            true
        } else {
            self.state = PlayerState::Raise;
            false
        }
    }

    pub fn blind_raise(&mut self, size: usize) {

        assert!(self.chips >= size);

        self.chips -= size;
        self.player_pot += size;
    }

    pub fn fold(&mut self) {
        self.state = PlayerState::Fold;
    }

    pub fn change_state(&mut self, state: PlayerState) {
        self.state = state;
    }

    pub fn get_chips(&mut self, chips: usize) {
        self.chips += chips;
    }
}

pub fn get_player_chips(id: &String) -> Result<usize, rusqlite::Error> {
    if let Some(user) = db::find_user_by_id(id)? {
        let chips = user.chips as usize;
        Ok(chips)
    } else {
        Err(rusqlite::Error::QueryReturnedNoRows)
    }
}