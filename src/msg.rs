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
    CloseRemainingCommitment {},
    AcceptCommitmentUpdate {
        forfeit_commitment: Option<u64>,
    },
    ClaimInvestment {},
    ClaimRedemption {
        asset: u64,
        to: Option<Addr>,
        memo: Option<String>,
    },
    ClaimDistribution {
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
    GetState {},
}
