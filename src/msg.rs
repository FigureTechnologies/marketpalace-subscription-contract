use crate::state::{CapitalCall, Distribution, Redemption, Withdrawal};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use cosmwasm_std::Addr;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub recovery_admin: Addr,
    pub lp: Addr,
    pub capital_denom: String,
    pub capital_per_share: u64,
    pub min_commitment: u64,
    pub max_commitment: u64,
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
    CloseRemainingCommitment {},
    ClaimInvestment {
        amount: u64,
    },
    ClaimRedemption {
        asset: u64,
        capital: u64,
        to: Option<Addr>,
        memo: Option<String>,
    },
    ClaimDistribution {
        amount: u64,
        to: Option<Addr>,
        memo: Option<String>,
    },
    IssueWithdrawal {
        to: Addr,
        amount: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetTerms {},
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
    pub withdrawals: HashSet<Withdrawal>,
}
