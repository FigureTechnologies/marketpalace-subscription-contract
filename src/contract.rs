use cosmwasm_std::{
    coin, entry_point, from_slice, to_binary, Addr, BankMsg, Binary, Coin, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult,
};
use provwasm_std::{mint_marker_supply, withdraw_coins, ProvenanceMsg, ProvenanceQuerier};

use serde::{Deserialize, Serialize};

use crate::error::ContractError;
use crate::msg::{CapitalCall, HandleMsg, InstantiateMsg, QueryMsg};
use crate::state::{config, config_read, State, Status, CONFIG_KEY};

fn contract_error(err: &str) -> ContractError {
    ContractError::Std(StdError::generic_err(err))
}

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        owner: info.sender,
        status: Status::Draft,
        raise: msg.raise,
        admin: msg.admin,
        capital_denom: msg.capital_denom,
        min_commitment: msg.min_commitment,
        max_commitment: msg.max_commitment,
        min_days_of_notice: msg.min_days_of_notice,
        commitment: None,
        capital_calls: vec![],
    };
    config(deps.storage).save(&state)?;

    Ok(Response::default())
}

// And declare a custom Error variant for the ones where you will want to make use of it
#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    match msg {
        HandleMsg::Accept { commitment } => try_accept(deps, _env, info, commitment),
        HandleMsg::IssueCapitalCall { capital_call } => {
            try_issue_capital_call(deps, _env, info, capital_call)
        }
        HandleMsg::IssueDistribution {} => try_issue_distribution(deps, _env, info),
        HandleMsg::RedeemDistribution {} => try_redeem_distribution(deps, _env, info),
    }
}

pub fn try_accept(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    commitment: u64,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Draft {
        return Err(contract_error("subscription is not in draft status"));
    }

    if info.sender != state.raise {
        return Err(contract_error("only the raise contract can accept"));
    }

    if commitment < state.min_commitment {
        return Err(contract_error("commitment less than minimum"));
    }

    if commitment > state.max_commitment {
        return Err(contract_error("commitment more than maximum"));
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.status = Status::Accepted;
        state.commitment = Option::Some(commitment);
        Ok(state)
    })?;

    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![],
        data: Option::None,
    })
}

#[derive(Deserialize)]
struct CallState {
    amount: u64,
    days_of_notice: Option<u16>,
}

pub fn try_issue_capital_call(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    capital_call: Addr,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return Err(contract_error("capital promise is not accepted"));
    }

    if info.sender != state.raise {
        return Err(contract_error(
            "only the raise contract can issue capital call",
        ));
    }

    let contract: CallState = from_slice(
        &deps
            .querier
            .query_wasm_raw(capital_call.clone(), CONFIG_KEY)?
            .unwrap(),
    )?;

    let balance = deps
        .querier
        .query_balance(_env.contract.address, state.capital_denom)?;
    if contract.amount
        > state
            .commitment
            .map(|commitment| commitment - balance.amount.u128() as u64)
            .unwrap_or(0)
    {
        return Err(contract_error(
            "capital call larger than remaining commitment",
        ));
    }

    if contract.days_of_notice.unwrap_or(u16::MAX) < state.min_days_of_notice.unwrap_or(0) {
        return Err(contract_error("not enough notice"));
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.capital_calls.push(capital_call);
        Ok(state)
    })?;

    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![],
        data: Option::None,
    })
}

pub fn try_issue_distribution(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return Err(contract_error("capital promise is not accepted"));
    }

    if info.sender != state.raise {
        return Err(contract_error(
            "only the raise contract can issue distribution",
        ));
    }

    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![],
        data: Option::None,
    })
}

pub fn try_redeem_distribution(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return Err(contract_error("capital promise is not accepted"));
    }

    if info.sender != state.owner {
        return Err(contract_error("only the owner can redeem distribution"));
    }

    let balance = deps
        .querier
        .query_balance(_env.contract.address, state.capital_denom)?;
    let send = BankMsg::Send {
        to_address: state.owner.to_string(),
        amount: vec![balance],
    }
    .into();

    Ok(Response {
        submessages: vec![],
        messages: vec![send],
        attributes: vec![],
        data: Option::None,
    })
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetStatus {} => to_binary(&query_status(deps)?),
    }
}

fn query_status(deps: Deps) -> StdResult<Status> {
    let state = config_read(deps.storage).load()?;
    Ok(state.status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{from_binary, Addr};
    use provwasm_mocks::mock_dependencies;

    fn inst_msg() -> InstantiateMsg {
        InstantiateMsg {
            raise: Addr::unchecked("tp18lysxk7sueunnspju4dar34vlv98a7kyyfkqs7"),
            admin: Addr::unchecked("tp1apnhcu9x5cz2l8hhgnj0hg7ez53jah7hcan000"),
            capital_denom: String::from("stable_coin"),
            min_commitment: 10_000,
            max_commitment: 50_000,
            min_days_of_notice: None,
        }
    }

    #[test]
    fn initialization() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, inst_msg()).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetStatus {}).unwrap();
        let status: Status = from_binary(&res).unwrap();
        assert_eq!(Status::Draft, status);
    }
}
