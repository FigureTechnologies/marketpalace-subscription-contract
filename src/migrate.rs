use std::collections::HashSet;
use std::convert::TryInto;
use std::hash::Hash;

use crate::error::ContractError;
use crate::msg::AssetExchange;
use crate::msg::MigrateMsg;
use crate::state::asset_exchange_authorization_storage;
use crate::state::state_storage;
use crate::state::AssetExchangeAuthorization;
use crate::state::State;
use crate::state::CONFIG_KEY;
use crate::version::CONTRACT_NAME;
use crate::version::CONTRACT_VERSION;
use cosmwasm_std::entry_point;
use cosmwasm_std::Addr;
use cosmwasm_std::DepsMut;
use cosmwasm_std::Env;
use cosmwasm_std::Response;
use cosmwasm_storage::singleton_read;
use cw2::set_contract_version;
use provwasm_std::ProvenanceMsg;
use provwasm_std::ProvenanceQuery;
use serde::Deserialize;
use serde::Serialize;

#[entry_point]
pub fn migrate(
    deps: DepsMut<ProvenanceQuery>,
    _: Env,
    _: MigrateMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let old_state: StateV1_0_0 = singleton_read(deps.storage, CONFIG_KEY).load()?;

    let new_state = State {
        admin: old_state.recovery_admin,
        lp: old_state.lp,
        raise: old_state.raise.clone(),
        commitment_denom: format!("{}.commitment", old_state.raise),
        investment_denom: format!("{}.investment", old_state.raise),
        capital_denom: old_state.capital_denom,
        capital_per_share: old_state.capital_per_share,
        required_capital_attribute: None,
    };

    state_storage(deps.storage).save(&new_state)?;

    if old_state.status == Status::Draft {
        asset_exchange_authorization_storage(deps.storage).save(&vec![
            AssetExchangeAuthorization {
                exchanges: vec![AssetExchange {
                    investment: None,
                    commitment_in_shares: Some(
                        new_state
                            .capital_to_shares(old_state.max_commitment)
                            .try_into()?,
                    ),
                    capital: None,
                    date: None,
                }],
                to: None,
                memo: None,
            },
        ])?;
    }

    Ok(Response::default())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StateV1_0_0 {
    pub recovery_admin: Addr,
    pub lp: Addr,
    pub status: Status,
    pub raise: Addr,
    pub capital_denom: String,
    pub capital_per_share: u64,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Status {
    Draft,
    Accepted,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq)]
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

#[derive(Serialize, Deserialize, Clone, Debug, Eq)]
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

#[derive(Serialize, Deserialize, Clone, Debug, Eq)]
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

#[derive(Serialize, Deserialize, Clone, Debug, Eq)]
pub struct Withdrawal {
    pub sequence: u16,
    pub to: Addr,
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

#[cfg(test)]
mod tests {
    use crate::state::asset_exchange_authorization_storage_read;

    use super::*;
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_storage::singleton;
    use provwasm_mocks::mock_dependencies;

    use super::StateV1_0_0;

    #[test]
    fn migration() {
        let mut deps = mock_dependencies(&[]);
        singleton(&mut deps.storage, CONFIG_KEY)
            .save(&StateV1_0_0 {
                recovery_admin: Addr::unchecked("marketpalace"),
                status: Status::Draft,
                lp: Addr::unchecked("lp"),
                raise: Addr::unchecked("raise_1"),
                capital_denom: String::from("stable_coin"),
                capital_per_share: 100,
                min_commitment: 0,
                max_commitment: 10_000,
                min_days_of_notice: None,
                sequence: 0,
                active_capital_call: None,
                closed_capital_calls: HashSet::new(),
                cancelled_capital_calls: HashSet::new(),
                redemptions: HashSet::new(),
                distributions: HashSet::new(),
                withdrawals: HashSet::new(),
            })
            .unwrap();

        migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();

        assert_eq!(
            State {
                admin: Addr::unchecked("marketpalace"),
                lp: Addr::unchecked("lp"),
                raise: Addr::unchecked("raise_1"),
                commitment_denom: String::from("raise_1.commitment"),
                investment_denom: String::from("raise_1.investment"),
                capital_denom: String::from("stable_coin"),
                capital_per_share: 100,
                required_capital_attribute: None,
            },
            singleton_read(&deps.storage, CONFIG_KEY).load().unwrap()
        );

        assert_eq!(
            &AssetExchangeAuthorization {
                exchanges: vec![AssetExchange {
                    investment: None,
                    commitment_in_shares: Some(100),
                    capital: None,
                    date: None,
                }],
                to: None,
                memo: None,
            },
            asset_exchange_authorization_storage_read(&deps.storage)
                .load()
                .unwrap()
                .get(0)
                .unwrap()
        );
    }
}
