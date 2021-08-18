use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::Hash;

use cosmwasm_std::{Addr, Storage};
use cosmwasm_storage::{singleton, singleton_read, ReadonlySingleton, Singleton};

pub static CONFIG_KEY: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub admin: Addr,
    pub lp: Addr,
    pub status: Status,
    pub raise: Addr,
    pub capital_denom: String,
    pub commitment_denom: String,
    pub min_commitment: u64,
    pub max_commitment: u64,
    pub min_days_of_notice: Option<u16>,
    pub capital_call_id_sequence: u16,
    pub active_capital_call: Option<CapitalCall>,
    pub closed_capital_calls: HashSet<CapitalCall>,
    pub cancelled_capital_calls: HashSet<CapitalCall>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum Status {
    Draft,
    Accepted,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, JsonSchema)]
pub struct CapitalCall {
    pub id: u16,
    pub amount: u64,
    pub days_of_notice: Option<u16>,
}

impl PartialEq for CapitalCall {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Hash for CapitalCall {
    fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher,
    {
        self.id.hash(state);
    }
}

pub fn config(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}
