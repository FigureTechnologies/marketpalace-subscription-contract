use crate::state::{CapitalCall, Distribution, Redemption};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
    IssueCapitalCall { capital_call: CapitalCallIssuance },
    CloseCapitalCall {},
    IssueRedemption { redemption: u64 },
    IssueDistribution {},
    Redeem {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CapitalCallIssuance {
    pub amount: u64,
    pub days_of_notice: Option<u16>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetTerms {},
    GetStatus {},
    GetTransactions {},
}

#[derive(Deserialize, Serialize)]
pub struct Terms {
    pub lp: Addr,
    pub raise: Addr,
    pub capital_denom: String,
    pub min_commitment: u64,
    pub max_commitment: u64,
}

#[derive(Deserialize, Serialize)]
pub struct CapitalCalls {
    pub active: Option<CapitalCall>,
    pub closed: HashSet<CapitalCall>,
    pub cancelled: HashSet<CapitalCall>,
}

#[derive(Deserialize, Serialize)]
pub struct Transactions {
    pub capital_calls: CapitalCalls,
    pub redemptions: HashSet<Redemption>,
    pub distributions: HashSet<Distribution>,
}
