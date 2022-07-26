use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Storage};
use cosmwasm_storage::{singleton, singleton_read, ReadonlySingleton, Singleton};

pub static CONFIG_KEY: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub recovery_admin: Addr,
    pub lp: Addr,
    pub raise: Addr,
    pub commitment_denom: String,
    pub investment_denom: String,
    pub capital_denom: String,
    pub capital_per_share: u64,
}

impl State {
    pub fn not_evenly_divisble(&self, amount: u64) -> bool {
        amount % self.capital_per_share > 0
    }

    pub fn capital_to_shares(&self, amount: u64) -> u64 {
        amount / self.capital_per_share
    }
}

pub fn config(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    impl State {
        pub fn test_default() -> State {
            State {
                recovery_admin: Addr::unchecked("admin"),
                lp: Addr::unchecked("lp"),
                raise: Addr::unchecked("raise_1"),
                commitment_denom: String::from("raise_1.commitment"),
                investment_denom: String::from("raise_1.investment"),
                capital_denom: String::from("stable_coin"),
                capital_per_share: 100,
            }
        }
    }

    #[test]
    fn not_evenly_divisble() {
        let state = State::test_default();

        assert_eq!(false, state.not_evenly_divisble(100));
        assert_eq!(true, state.not_evenly_divisble(101));
        assert_eq!(false, state.not_evenly_divisble(1_000));
        assert_eq!(true, state.not_evenly_divisble(1_001));
    }
}
