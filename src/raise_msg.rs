use cosmwasm_std::Addr;
use serde::{Deserialize, Serialize};

use crate::msg::AssetExchange;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RaiseExecuteMsg {
    CompleteAssetExchange {
        exchange: AssetExchange,
        to: Option<Addr>,
        memo: Option<String>,
    },
}
