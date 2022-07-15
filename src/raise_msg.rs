use cosmwasm_std::Addr;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RaiseExecuteMsg {
    CloseRemainingCommitment {},
    ClaimInvestment {},
    ClaimRedemption { to: Addr, memo: Option<String> },
    ClaimDistribution { to: Addr, memo: Option<String> },
}
