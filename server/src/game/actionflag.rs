pub struct ActionFlag {
    pub check: bool,
    pub call: bool,
    pub raise: bool,
    pub allin: bool,
    pub fold: bool,
}

impl ActionFlag {
    pub fn new() -> Self {
        Self {
            check: true,
            call: true,
            raise: true,
            allin: true,
            fold: true,
        }
    }

    pub fn can_act(&self, action: u32) -> bool {
        match action {
            1 => self.check,
            2 => self.call,
            3 => self.raise,
            4 => self.allin,
            5 => self.fold,
            _ => false
        }
    }
}