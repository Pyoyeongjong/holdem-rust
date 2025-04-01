use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
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
}