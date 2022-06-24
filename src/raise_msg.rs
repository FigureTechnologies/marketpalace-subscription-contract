use cosmwasm_std::Addr;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RaiseExecuteMsg {
    ClaimRedemption {
        asset: u64,
        capital: u64,
        to: Addr,
        memo: Option<String>,
    },
    ClaimDistribution {
        amount: u64,
        to: Addr,
        memo: Option<String>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubQueryMsg {
    GetTerms {},
    GetTransactions {},
}
