use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub recovery_admin: Addr,
    pub lp: Addr,
    pub commitment_denom: String,
    pub investment_denom: String,
    pub capital_denom: String,
    pub capital_per_share: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Recover {
        lp: Addr,
    },
    CompleteAssetExchange {
        exchange: AssetExchange,
        to: Option<Addr>,
        memo: Option<String>,
    },
    IssueWithdrawal {
        to: Addr,
        amount: u64,
    },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AssetExchange {
    pub investment: Option<i64>,
    pub commitment: Option<i64>,
    pub capital: Option<i64>,
    pub date: Option<ExchangeDate>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum ExchangeDate {
    Due(u64),
    Available(u64),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetState {},
}
