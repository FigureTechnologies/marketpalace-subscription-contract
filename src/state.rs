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
    pub min_commitment: u64,
    pub max_commitment: u64,
    pub min_days_of_notice: Option<u16>,
    pub sequence: u16,
    pub active_capital_call: Option<CapitalCall>,
    pub closed_capital_calls: HashSet<CapitalCall>,
    pub cancelled_capital_calls: HashSet<CapitalCall>,
    pub redemptions: HashSet<Redemption>,
    pub distributions: HashSet<Distribution>,
    pub withdrawals: HashSet<Withdrawal>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum Status {
    Draft,
    Accepted,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, JsonSchema)]
pub struct CapitalCall {
    pub sequence: u16,
    pub amount: u64,
    pub days_of_notice: Option<u16>,
}

impl PartialEq for CapitalCall {
    fn eq(&self, other: &Self) -> bool {
        self.sequence == other.sequence
    }
}

impl Hash for CapitalCall {
    fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher,
    {
        self.sequence.hash(state);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, JsonSchema)]
pub struct Redemption {
    pub sequence: u16,
    pub asset: u64,
    pub capital: u64,
}

impl PartialEq for Redemption {
    fn eq(&self, other: &Self) -> bool {
        self.sequence == other.sequence
    }
}

impl Hash for Redemption {
    fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher,
    {
        self.sequence.hash(state);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, JsonSchema)]
pub struct Distribution {
    pub sequence: u16,
    pub amount: u64,
}

impl PartialEq for Distribution {
    fn eq(&self, other: &Self) -> bool {
        self.sequence == other.sequence
    }
}

impl Hash for Distribution {
    fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher,
    {
        self.sequence.hash(state);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, JsonSchema)]
pub struct Withdrawal {
    pub sequence: u16,
    pub amount: u64,
}

impl PartialEq for Withdrawal {
    fn eq(&self, other: &Self) -> bool {
        self.sequence == other.sequence
    }
}

impl Hash for Withdrawal {
    fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher,
    {
        self.sequence.hash(state);
    }
}

pub fn config(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}
