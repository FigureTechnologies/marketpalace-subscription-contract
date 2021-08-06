use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub lp: Addr,
    pub admin: Addr,
    pub capital_denom: String,
    pub min_commitment: u64,
    pub max_commitment: u64,
    pub min_days_of_notice: Option<u16>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Recover { lp: Addr },
    Accept {},
    IssueCapitalCall { capital_call: CapitalCall },
    CloseCapitalCall {},
    IssueRedemption { redemption: u64 },
    IssueDistribution {},
    Redeem {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CapitalCall {
    pub amount: u64,
    pub days_of_notice: Option<u16>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetTerms {},
    GetStatus {},
}

#[derive(Deserialize, Serialize)]
pub struct Terms {
    pub lp: Addr,
    pub raise: Addr,
    pub capital_denom: String,
    pub min_commitment: u64,
    pub max_commitment: u64,
}
