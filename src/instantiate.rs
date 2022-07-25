use crate::contract::ContractResponse;
use crate::msg::InstantiateMsg;
use crate::state::config;
use crate::state::State;
use crate::version::CONTRACT_NAME;
use crate::version::CONTRACT_VERSION;
use cosmwasm_std::entry_point;
use cosmwasm_std::DepsMut;
use cosmwasm_std::Env;
use cosmwasm_std::MessageInfo;
use cosmwasm_std::Response;
use cw2::set_contract_version;
use provwasm_std::ProvenanceQuery;

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
        recovery_admin: msg.recovery_admin,
        lp: msg.lp.clone(),
        capital_denom: msg.capital_denom,
        capital_per_share: msg.capital_per_share,
    };

    config(deps.storage).save(&state)?;

    Ok(Response::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::query;
    use crate::msg::QueryMsg;
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
            },
        )
        .unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetState {}).unwrap();
        let state: State = from_binary(&res).unwrap();
        assert_eq!("lp", state.lp);
    }
}
