use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RaiseExecuteMsg {
    ClaimRedemption {},
    ClaimDistribution { amount: u64 },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubQueryMsg {
    GetTerms {},
    GetTransactions {},
}
