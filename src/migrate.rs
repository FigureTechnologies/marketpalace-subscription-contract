use std::hash::Hash;

use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::state::state_storage;
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
    migrate_msg: MigrateMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let old_state: StateV2_0_0 = singleton_read(deps.storage, CONFIG_KEY).load()?;

    let new_state = State {
        admin: old_state.admin,
        lp: old_state.lp,
        raise: old_state.raise.clone(),
        commitment_denom: old_state.commitment_denom,
        investment_denom: old_state.investment_denom,
        like_capital_denoms: migrate_msg.like_capital_denoms,
        capital_per_share: old_state.capital_per_share,
        required_capital_attributes: migrate_msg.required_capital_attributes,
    };

    state_storage(deps.storage).save(&new_state)?;

    Ok(Response::default())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StateV2_0_0 {
    pub admin: Addr,
    pub lp: Addr,
    pub raise: Addr,
    pub commitment_denom: String,
    pub investment_denom: String,
    pub capital_denom: String,
    pub capital_per_share: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StateV2_2_0 {
    pub admin: Addr,
    pub lp: Addr,
    pub raise: Addr,
    pub commitment_denom: String,
    pub investment_denom: String,
    pub capital_denom: String,
    pub capital_per_share: u64,
    pub required_capital_attribute: Option<String>,
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
    use crate::msg::CapitalDenomRequirement;

    use super::*;
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_storage::singleton;
    use provwasm_mocks::mock_dependencies;

    use super::StateV2_0_0;

    #[test]
    fn migration() {
        let mut deps = mock_dependencies(&[]);
        singleton(&mut deps.storage, CONFIG_KEY)
            .save(&StateV2_0_0 {
                admin: Addr::unchecked("marketpalace"),
                lp: Addr::unchecked("lp"),
                raise: Addr::unchecked("raise_1"),
                commitment_denom: "commitment".to_string(),
                investment_denom: "investment".to_string(),
                capital_denom: String::from("stable_coin"),
                capital_per_share: 100,
            })
            .unwrap();

        migrate(
            deps.as_mut(),
            mock_env(),
            MigrateMsg {
                like_capital_denoms: vec![String::from("stable_coin")],
                required_capital_attributes: vec![],
            },
        )
        .unwrap();

        assert_eq!(
            State {
                admin: Addr::unchecked("marketpalace"),
                lp: Addr::unchecked("lp"),
                raise: Addr::unchecked("raise_1"),
                commitment_denom: String::from("commitment"),
                investment_denom: String::from("investment"),
                like_capital_denoms: vec![String::from("stable_coin")],
                capital_per_share: 100,
                required_capital_attributes: vec![],
            },
            singleton_read(&deps.storage, CONFIG_KEY).load().unwrap()
        );
    }

    #[test]
    fn migration_with_capital_denom_and_attribute() {
        let mut deps = mock_dependencies(&[]);
        singleton(&mut deps.storage, CONFIG_KEY)
            .save(&StateV2_0_0 {
                admin: Addr::unchecked("marketpalace"),
                lp: Addr::unchecked("lp"),
                raise: Addr::unchecked("raise_1"),
                commitment_denom: "commitment".to_string(),
                investment_denom: "investment".to_string(),
                capital_denom: String::from("stable_coin"),
                capital_per_share: 100,
            })
            .unwrap();

        let migration_msg = MigrateMsg {
            like_capital_denoms: vec![String::from("new_denom")],
            required_capital_attributes: vec![CapitalDenomRequirement {
                capital_denom: String::from("new_denom"),
                required_attribute: String::from("attr"),
            }],
        };
        migrate(deps.as_mut(), mock_env(), migration_msg).unwrap();

        assert_eq!(
            State {
                admin: Addr::unchecked("marketpalace"),
                lp: Addr::unchecked("lp"),
                raise: Addr::unchecked("raise_1"),
                commitment_denom: String::from("commitment"),
                investment_denom: String::from("investment"),
                like_capital_denoms: vec![String::from("new_denom")],
                capital_per_share: 100,
                required_capital_attributes: vec![CapitalDenomRequirement {
                    capital_denom: String::from("new_denom"),
                    required_attribute: String::from("attr"),
                }],
            },
            singleton_read(&deps.storage, CONFIG_KEY).load().unwrap()
        );
    }
}
