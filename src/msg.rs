use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Coin};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub raise: Addr,
    pub admin: Addr,
    pub capital_denom: String,
    pub min_commitment: u64,
    pub max_commitment: u64,
    pub min_days_of_notice: Option<u16>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Accept { commitment: u64 },
    IssueCapitalCall { capital_call: Addr },
    IssueRedemption { redemption: Coin },
    IssueDistribution {},
    RedeemDistribution {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetTerms returns the terms of this subscription.
    GetTerms {},
}

#[derive(Deserialize, Serialize)]
pub struct SubTerms {
    pub owner: Addr,
    pub raise: Addr,
    pub capital_denom: String,
    pub min_commitment: u64,
    pub max_commitment: u64,
}
