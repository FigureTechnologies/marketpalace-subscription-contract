use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub admin: Addr,
    pub lp: Addr,
    pub commitment_denom: String,
    pub investment_denom: String,
    pub like_capital_denoms: Vec<String>,
    pub capital_per_share: u64,
    pub initial_commitment: Option<u64>,
    pub required_capital_attribute: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {
    pub like_capital_denoms: Vec<String>,
    pub required_capital_attribute: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Recover {
        lp: Addr,
    },
    AuthorizeAssetExchange {
        exchanges: Vec<AssetExchange>,
        to: Option<Addr>,
        memo: Option<String>,
    },
    CancelAssetExchangeAuthorization {
        exchanges: Vec<AssetExchange>,
        to: Option<Addr>,
        memo: Option<String>,
    },
    CompleteAssetExchange {
        exchanges: Vec<AssetExchange>,
        to: Option<Addr>,
        memo: Option<String>,
    },
    IssueWithdrawal {
        to: Addr,
        amount: u64,
        capital_denom: Option<String>,
    },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AssetExchange {
    #[serde(rename = "inv")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub investment: Option<i64>,
    #[serde(rename = "com")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub commitment_in_shares: Option<i64>,
    #[serde(rename = "cap_d")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub capital_denom: Option<String>,
    #[serde(rename = "cap")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub capital: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub date: Option<ExchangeDate>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum ExchangeDate {
    #[serde(rename = "due")]
    Due(u64),
    #[serde(rename = "avl")]
    Available(u64),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetState {},
    GetAssetExchangeAuthorizations {},
}
