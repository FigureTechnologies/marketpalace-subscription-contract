use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult,
};
use provwasm_std::{transfer_marker_coins, ProvenanceMsg, ProvenanceQuerier};

use crate::call::{CallQueryMsg, CallTerms};
use crate::error::ContractError;
use crate::msg::{HandleMsg, InstantiateMsg, QueryMsg, Terms};
use crate::state::{config, config_read, State, Status};

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
        lp: info.sender,
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
    env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    match msg {
        HandleMsg::Recover { lp } => try_recover(deps, info, lp),
        HandleMsg::Accept { commitment } => try_accept(deps, info, commitment),
        HandleMsg::IssueCapitalCall { capital_call } => {
            try_issue_capital_call(deps, env, info, capital_call)
        }
        HandleMsg::IssueRedemption { redemption } => {
            try_issue_redemption(deps, env, info, redemption)
        }
        HandleMsg::IssueDistribution {} => try_issue_distribution(deps, info),
        HandleMsg::Redeem {} => try_redeem(deps, env, info),
    }
}

pub fn try_recover(
    deps: DepsMut,
    info: MessageInfo,
    lp: Addr,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if info.sender != state.admin {
        return Err(contract_error("only admin can recover subscription"));
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.lp = lp;
        Ok(state)
    })?;

    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![],
        data: Option::None,
    })
}

pub fn try_accept(
    deps: DepsMut,
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

pub fn try_issue_capital_call(
    deps: DepsMut,
    env: Env,
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

    let terms: CallTerms = deps
        .querier
        .query_wasm_smart(capital_call.clone(), &CallQueryMsg::GetTerms {})
        .expect("terms");

    let balance = deps
        .querier
        .query_balance(env.contract.address, state.capital_denom)?;
    if terms.amount
        > state
            .commitment
            .map(|commitment| commitment - balance.amount.u128() as u64)
            .unwrap_or(0)
    {
        return Err(contract_error(
            "capital call larger than remaining commitment",
        ));
    }

    if terms.days_of_notice.unwrap_or(u16::MAX) < state.min_days_of_notice.unwrap_or(0) {
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

pub fn try_issue_redemption(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    redemption: Coin,
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

    let marker =
        ProvenanceQuerier::new(&deps.querier).get_marker_by_denom(redemption.denom.clone())?;
    let transfer = transfer_marker_coins(
        redemption.amount.u128(),
        redemption.denom,
        marker.address,
        env.contract.address,
    )?;

    Ok(Response {
        submessages: vec![],
        messages: vec![transfer],
        attributes: vec![],
        data: Option::None,
    })
}

pub fn try_issue_distribution(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return Err(contract_error("subscription has not been accepted"));
    }

    if info.sender != state.raise {
        return Err(contract_error(
            "only the raise contract can issue redemption",
        ));
    }

    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![],
        data: Option::None,
    })
}

pub fn try_redeem(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return Err(contract_error("subscription has not been accepted"));
    }

    if info.sender != state.lp {
        return Err(contract_error("only the lp can redeem distribution"));
    }

    let balance = deps
        .querier
        .query_balance(env.contract.address, state.capital_denom)?;
    let send = BankMsg::Send {
        to_address: state.lp.to_string(),
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
    let state = config_read(deps.storage).load()?;

    match msg {
        QueryMsg::GetTerms {} => to_binary(&Terms {
            lp: state.lp,
            raise: state.raise,
            capital_denom: state.capital_denom,
            min_commitment: state.min_commitment,
            max_commitment: state.max_commitment,
        }),
        QueryMsg::GetStatus {} => to_binary(&state.status),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::wasm_smart_mock_dependencies;
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{coin, coins, from_binary, Addr, ContractResult, SystemError, SystemResult};
    use provwasm_mocks::{mock_dependencies, must_read_binary_file};
    use provwasm_std::Marker;

    #[test]
    fn initialization() {
        let mut deps = mock_dependencies(&[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &[]),
            InstantiateMsg {
                raise: Addr::unchecked("raise"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                min_commitment: 10_000,
                max_commitment: 50_000,
                min_days_of_notice: None,
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
    fn recover() {
        let mut deps = mock_dependencies(&[]);

        config(&mut deps.storage)
            .save(&State {
                admin: Addr::unchecked("admin"),
                lp: Addr::unchecked("lp"),
                status: Status::Draft,
                raise: Addr::unchecked("raise"),
                capital_denom: String::from("stable_coin"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                commitment: None,
                capital_calls: vec![],
            })
            .unwrap();

        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("admin", &vec![]),
            HandleMsg::Recover {
                lp: Addr::unchecked("lp_2"),
            },
        )
        .unwrap();
    }

    #[test]
    fn bad_actor_recover_fail() {
        let mut deps = mock_dependencies(&[]);

        config(&mut deps.storage)
            .save(&State {
                admin: Addr::unchecked("admin"),
                lp: Addr::unchecked("lp"),
                status: Status::Draft,
                raise: Addr::unchecked("raise"),
                capital_denom: String::from("stable_coin"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                commitment: None,
                capital_calls: vec![],
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::Recover {
                lp: Addr::unchecked("bad_actor"),
            },
        );
        assert_eq!(true, res.is_err());
    }

    #[test]
    fn accept() {
        let mut deps = mock_dependencies(&[]);

        config(&mut deps.storage)
            .save(&State {
                lp: Addr::unchecked("lp"),
                status: Status::Draft,
                raise: Addr::unchecked("raise"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                commitment: None,
                capital_calls: vec![],
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise", &vec![]),
            HandleMsg::Accept { commitment: 20_000 },
        )
        .unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetStatus {}).unwrap();
        let status: Status = from_binary(&res).unwrap();
        assert_eq!(Status::Accepted, status);
    }

    #[test]
    fn issue_capital_call() {
        let mut deps = wasm_smart_mock_dependencies(
            &coins(10_000, "capital_receipt"),
            |contract_addr, _msg| match &contract_addr[..] {
                "call_1" => SystemResult::Ok(ContractResult::Ok(
                    to_binary(&CallTerms {
                        subscription: Addr::unchecked("sub_1"),
                        raise: Addr::unchecked(MOCK_CONTRACT_ADDR),
                        amount: 10_000,
                        days_of_notice: None,
                    })
                    .unwrap(),
                )),
                _ => SystemResult::Err(SystemError::UnsupportedRequest {
                    kind: String::from("not mocked"),
                }),
            },
        );

        config(&mut deps.storage)
            .save(&State {
                lp: Addr::unchecked("lp"),
                status: Status::Accepted,
                raise: Addr::unchecked("raise"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                commitment: Some(20_000),
                capital_calls: vec![],
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise", &vec![]),
            HandleMsg::IssueCapitalCall {
                capital_call: Addr::unchecked("call_1"),
            },
        )
        .unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn issue_redemption() {
        let mut deps = mock_dependencies(&[]);

        let bin = must_read_binary_file("testdata/marker.json");
        let expected_marker: Marker = from_binary(&bin).unwrap();
        deps.querier.with_markers(vec![expected_marker.clone()]);

        config(&mut deps.storage)
            .save(&State {
                lp: Addr::unchecked("lp"),
                status: Status::Accepted,
                raise: Addr::unchecked("raise"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                commitment: None,
                capital_calls: vec![],
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise", &coins(5_000, "stable_coin")),
            HandleMsg::IssueRedemption {
                redemption: coin(5_000, "capital_receipt"),
            },
        )
        .unwrap();
        assert_eq!(1, res.messages.len());
    }

    #[test]
    fn issue_distribution() {
        let mut deps = mock_dependencies(&[]);

        config(&mut deps.storage)
            .save(&State {
                lp: Addr::unchecked("lp"),
                status: Status::Accepted,
                raise: Addr::unchecked("raise"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                commitment: None,
                capital_calls: vec![],
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise", &coins(5_000, "stable_coin")),
            HandleMsg::IssueDistribution {},
        )
        .unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn redeem() {
        let mut deps = mock_dependencies(&[]);

        config(&mut deps.storage)
            .save(&State {
                lp: Addr::unchecked("lp"),
                status: Status::Accepted,
                raise: Addr::unchecked("raise"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                commitment: None,
                capital_calls: vec![],
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::Redeem {},
        )
        .unwrap();
        assert_eq!(1, res.messages.len());
    }
}
