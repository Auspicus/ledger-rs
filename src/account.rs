use serde::{Deserialize, Serialize};

// 27 bytes
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct Account {
    /// Client ID.
    #[serde(rename = "client")]
    pub client_id: u16, // 2 bytes

    #[serde(rename = "available")]
    pub available_funds: f64, // 8 bytes

    #[serde(rename = "held")]
    pub held_funds: f64, // 8 bytes

    #[serde(rename = "total")]
    pub total_funds: f64, // 8 bytes

    #[serde(rename = "locked")]
    pub is_locked: bool, // 1 bytes
}

impl Account {
    pub fn new(id: u16) -> Self {
        Account {
            client_id: id,
            held_funds: 0.0,
            available_funds: 0.0,
            total_funds: 0.0,
            is_locked: false,
        }
    }
}
