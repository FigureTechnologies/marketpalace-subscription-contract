use crate::contract::ContractResponse;
use crate::error::contract_error;
use crate::msg::InstantiateMsg;
use crate::state::config;
use crate::state::State;
use crate::state::Status;
use crate::version::CONTRACT_NAME;
use crate::version::CONTRACT_VERSION;
use cosmwasm_std::entry_point;
use cosmwasm_std::DepsMut;
use cosmwasm_std::Env;
use cosmwasm_std::MessageInfo;
use cosmwasm_std::Response;
use cw2::set_contract_version;
use provwasm_std::ProvenanceQuery;
use std::collections::HashSet;

#[entry_point]
pub fn instantiate(
    deps: DepsMut<ProvenanceQuery>,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResponse {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let state = State {
        raise: info.sender,
        status: Status::Draft,
        recovery_admin: msg.recovery_admin,
        lp: msg.lp.clone(),
        capital_denom: msg.capital_denom,
        capital_per_share: msg.capital_per_share,
        min_commitment: msg.min_commitment,
        max_commitment: msg.max_commitment,
        sequence: 0,
        active_capital_call: None,
        closed_capital_calls: HashSet::new(),
        cancelled_capital_calls: HashSet::new(),
        redemptions: HashSet::new(),
        distributions: HashSet::new(),
        withdrawals: HashSet::new(),
    };

    if state.not_evenly_divisble(msg.min_commitment) {
        return contract_error("min commitment must be evenly divisible by capital per share");
    }

    if state.not_evenly_divisble(msg.max_commitment) {
        return contract_error("max commitment must be evenly divisible by capital per share");
    }

    config(deps.storage).save(&state)?;

    Ok(Response::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::query;
    use crate::msg::QueryMsg;
    use crate::msg::Terms;
    use cosmwasm_std::from_binary;
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::testing::mock_info;
    use cosmwasm_std::Addr;
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn initialization() {
        let mut deps = mock_dependencies(&[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &[]),
            InstantiateMsg {
                recovery_admin: Addr::unchecked("admin"),
                lp: Addr::unchecked("lp"),
                capital_denom: String::from("stable_coin"),
                capital_per_share: 100,
                min_commitment: 10_000,
                max_commitment: 50_000,
            },
        )
        .unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetTerms {}).unwrap();
        let terms: Terms = from_binary(&res).unwrap();
        assert_eq!("lp", terms.lp);
    }

    #[test]
    fn init_with_bad_min() {
        let mut deps = mock_dependencies(&[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &[]),
            InstantiateMsg {
                recovery_admin: Addr::unchecked("admin"),
                lp: Addr::unchecked("lp"),
                capital_denom: String::from("stable_coin"),
                capital_per_share: 100,
                min_commitment: 10_001,
                max_commitment: 50_000,
            },
        );
        assert_eq!(true, res.is_err());
    }

    #[test]
    fn init_with_bad_max() {
        let mut deps = mock_dependencies(&[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &[]),
            InstantiateMsg {
                recovery_admin: Addr::unchecked("admin"),
                lp: Addr::unchecked("lp"),
                capital_denom: String::from("stable_coin"),
                capital_per_share: 100,
                min_commitment: 10_000,
                max_commitment: 50_001,
            },
        );
        assert_eq!(true, res.is_err());
    }
}
