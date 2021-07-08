use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Storage};
use cosmwasm_storage::{singleton, singleton_read, ReadonlySingleton, Singleton};

pub static CONFIG_KEY: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub owner: Addr,
    pub status: Status,
    pub raise_contract_address: Addr,
    pub admin: Addr,
    pub commitment_denom: String,
    pub min_commitment: u64,
    pub max_commitment: u64,
    pub min_days_of_notice: Option<u16>,
    pub commitment: Option<u64>,
    pub paid: Option<u64>,
}

impl State {
    pub fn remaining_commitment(&self) -> Option<u64> {
        self.commitment.map(|commitment| commitment - self.paid.unwrap_or(0))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum Status {
    Draft,
    Pending,
    Accepted,
}

pub fn config(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}
