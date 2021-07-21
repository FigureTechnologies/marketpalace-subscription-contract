use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;

#[derive(Deserialize, Serialize)]
pub struct CallTerms {
    pub subscription: Addr,
    pub raise: Addr,
    pub amount: u64,
    pub days_of_notice: Option<u16>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CallQueryMsg {
    GetTerms {},
}
