pub struct Player {
    pub name: String,
    pub chips: u32,
    pub state: PlayerState,
    pub hands: Option<(String, String)>, // 있을 수도 있고 없을 수도 있으니까
    pub player_pot: u32, // 변수 명 바꾸고 싶은데
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
    pub fn new(name: String, chips: u32) -> Player {
        Player {
            name,
            chips,
            state: PlayerState::Waiting,
            hands: None,
            player_pot: 0,
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

    pub fn call(&mut self, size: u32) {
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

    pub fn raise(&mut self, size: u32) -> bool {

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

    pub fn blind_raise(&mut self, size: u32) {

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

    pub fn get_chips(&mut self, chips: u32) {
        self.chips += chips;
    }
}