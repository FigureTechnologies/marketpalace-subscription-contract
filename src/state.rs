use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Storage};
use cosmwasm_storage::{singleton, singleton_read, ReadonlySingleton, Singleton};

use crate::msg::{AssetExchange, CapitalDenomRequirement};

pub static CONFIG_KEY: &[u8] = b"config";
pub static ASSET_EXCHANGE_AUTHORIZATION_KEY: &[u8] = b"asset_exchange_authorizations";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub admin: Addr,
    pub lp: Addr,
    pub raise: Addr,
    pub commitment_denom: String,
    pub investment_denom: String,
    pub like_capital_denoms: Vec<String>,
    pub capital_per_share: u64,
    pub required_capital_attributes: Vec<CapitalDenomRequirement>,
}

impl State {
    pub fn not_evenly_divisble(&self, amount: u64) -> bool {
        amount % self.capital_per_share > 0
    }

    pub fn capital_to_shares(&self, amount: u64) -> u64 {
        amount / self.capital_per_share
    }
}

pub fn state_storage(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn state_storage_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AssetExchangeAuthorization {
    pub exchanges: Vec<AssetExchange>,
    pub to: Option<Addr>,
    pub memo: Option<String>,
}

pub fn asset_exchange_authorization_storage(
    storage: &mut dyn Storage,
) -> Singleton<Vec<AssetExchangeAuthorization>> {
    singleton(storage, ASSET_EXCHANGE_AUTHORIZATION_KEY)
}

pub fn asset_exchange_authorization_storage_read(
    storage: &dyn Storage,
) -> ReadonlySingleton<Vec<AssetExchangeAuthorization>> {
    singleton_read(storage, ASSET_EXCHANGE_AUTHORIZATION_KEY)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    impl State {
        pub fn test_default() -> State {
            State {
                admin: Addr::unchecked("admin"),
                lp: Addr::unchecked("lp"),
                raise: Addr::unchecked("raise_1"),
                commitment_denom: String::from("raise_1.commitment"),
                investment_denom: String::from("raise_1.investment"),
                like_capital_denoms: vec![String::from("stable_coin")],
                capital_per_share: 100,
                required_capital_attributes: vec![],
            }
        }

        pub fn test_capital_coin() -> State {
            State {
                admin: Addr::unchecked("admin"),
                lp: Addr::unchecked("lp"),
                raise: Addr::unchecked("raise_1"),
                commitment_denom: String::from("raise_1.commitment"),
                investment_denom: String::from("raise_1.investment"),
                like_capital_denoms: vec![String::from("capital_coin")],
                capital_per_share: 100,
                required_capital_attributes: vec![],
            }
        }

        pub fn test_restricted_capital_coin() -> State {
            State {
                admin: Addr::unchecked("admin"),
                lp: Addr::unchecked("lp"),
                raise: Addr::unchecked("raise_1"),
                commitment_denom: String::from("raise_1.commitment"),
                investment_denom: String::from("raise_1.investment"),
                like_capital_denoms: vec![String::from("restricted_capital_coin")],
                capital_per_share: 100,
                required_capital_attributes: vec![CapitalDenomRequirement {
                    capital_denom: String::from("restricted_capital_coin"),
                    required_attribute: String::from("capital.test"),
                }],
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
