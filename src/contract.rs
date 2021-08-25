use cosmwasm_std::{
    coins, entry_point, to_binary, Addr, Attribute, BankMsg, Binary, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult,
};
use provwasm_std::{
    activate_marker, create_marker, finalize_marker, grant_marker_access, withdraw_coins,
    MarkerAccess, MarkerType, ProvenanceMsg,
};
use std::collections::HashSet;

use crate::error::ContractError;
use crate::msg::{
    CapitalCallIssuance, CapitalCalls, HandleMsg, InstantiateMsg, QueryMsg, Terms, Transactions,
};
use crate::state::{config, config_read, CapitalCall, Distribution, Redemption, State, Status};

fn contract_error(err: &str) -> ContractError {
    ContractError::Std(StdError::generic_err(err))
}

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = State {
        raise: info.sender,
        status: Status::Draft,
        lp: msg.lp.clone(),
        admin: msg.admin,
        capital_denom: msg.capital_denom,
        commitment_denom: format!("{}.commitment", env.contract.address),
        min_commitment: msg.min_commitment,
        max_commitment: msg.max_commitment,
        min_days_of_notice: msg.min_days_of_notice,
        sequence: 0,
        active_capital_call: None,
        closed_capital_calls: HashSet::new(),
        cancelled_capital_calls: HashSet::new(),
        redemptions: HashSet::new(),
        distributions: HashSet::new(),
    };
    config(deps.storage).save(&state)?;

    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![],
        data: Option::None,
    })
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
        HandleMsg::Accept {} => try_accept(deps, env, info),
        HandleMsg::IssueCapitalCall { capital_call } => {
            try_issue_capital_call(deps, env, info, capital_call)
        }
        HandleMsg::CloseCapitalCall {} => try_close_capital_call(deps, env, info),
        HandleMsg::IssueRedemption { redemption } => {
            try_issue_redemption(deps, env, info, redemption)
        }
        HandleMsg::IssueDistribution {} => try_issue_distribution(deps, env, info),
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
    env: Env,
    info: MessageInfo,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Draft {
        return Err(contract_error("subscription is not in draft status"));
    }

    if info.sender != state.raise {
        return Err(contract_error("only the raise contract can accept"));
    }

    let investment = match info.funds.first() {
        Some(investment) => investment,
        None => return Err(contract_error("investment required")),
    };

    if investment.amount.u128() < state.min_commitment.into() {
        return Err(contract_error("investment less than minimum commitment"));
    }

    if investment.amount.u128() > state.max_commitment.into() {
        return Err(contract_error("investment more than maximum commitment"));
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.status = Status::Accepted;
        Ok(state)
    })?;

    let create = create_marker(
        investment.amount.u128(),
        state.commitment_denom.clone(),
        MarkerType::Coin,
    )?;
    let grant = grant_marker_access(
        state.commitment_denom.clone(),
        env.contract.address,
        vec![
            MarkerAccess::Admin,
            MarkerAccess::Mint,
            MarkerAccess::Burn,
            MarkerAccess::Withdraw,
        ],
    )?;
    let finalize = finalize_marker(state.commitment_denom.clone())?;
    let activate = activate_marker(state.commitment_denom.clone())?;

    let withraw = withdraw_coins(
        state.commitment_denom.clone(),
        investment.amount.u128(),
        state.commitment_denom,
        state.raise,
    )?;

    Ok(Response {
        submessages: vec![],
        messages: vec![create, grant, finalize, activate, withraw],
        attributes: vec![],
        data: Option::None,
    })
}

pub fn try_issue_capital_call(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    capital_call: CapitalCallIssuance,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return Err(contract_error("subscription is not accepted"));
    }

    if info.sender != state.raise {
        return Err(contract_error(
            "only the raise contract can issue capital call",
        ));
    }

    let commitment = deps
        .querier
        .query_balance(state.raise, state.commitment_denom)
        .unwrap();

    if capital_call.amount > commitment.amount.u128() as u64 {
        return Err(contract_error(
            "capital call larger than remaining commitment",
        ));
    }

    if capital_call.days_of_notice.unwrap_or(u16::MAX) < state.min_days_of_notice.unwrap_or(0) {
        return Err(contract_error("not enough notice"));
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.sequence += 1;
        let replaced = state.active_capital_call.replace(CapitalCall {
            sequence: state.sequence,
            amount: capital_call.amount,
            days_of_notice: capital_call.days_of_notice,
        });
        if let Some(replaced) = replaced {
            state.cancelled_capital_calls.insert(replaced);
        }
        Ok(state)
    })?;

    let state = config_read(deps.storage).load()?;

    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![Attribute {
            key: format!("{}.capital_call.sequence", env.contract.address),
            value: format!("{}", state.sequence),
        }],
        data: Option::None,
    })
}

pub fn try_close_capital_call(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return Err(contract_error("subscription is not accepted"));
    }

    if info.sender != state.raise {
        return Err(contract_error(
            "only the raise contract can close capital call",
        ));
    }

    let capital_call = match state.active_capital_call {
        Some(capital_call) => capital_call,
        None => return Err(contract_error("no existing capital call issued")),
    };

    let commitment = match info.funds.first() {
        Some(commitment) => commitment,
        None => return Err(contract_error("no commitment provided")),
    };

    if commitment.amount.u128() != u128::from(capital_call.amount) {
        return Err(contract_error("incorrect commitment provided"));
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state
            .closed_capital_calls
            .insert(state.active_capital_call.take().unwrap());
        Ok(state)
    })?;

    let send = BankMsg::Send {
        to_address: state.raise.to_string(),
        amount: coins(capital_call.amount as u128, state.capital_denom),
    }
    .into();

    Ok(Response {
        submessages: vec![],
        messages: vec![send],
        attributes: vec![Attribute {
            key: format!("{}.capital_call.sequence", env.contract.address),
            value: format!("{}", state.sequence),
        }],
        data: Option::None,
    })
}

pub fn try_issue_redemption(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    redemption: u64,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return Err(contract_error("subscription is not accepted"));
    }

    if info.sender != state.raise {
        return Err(contract_error(
            "only the raise contract can issue redemption",
        ));
    }

    let payment = match info.funds.first() {
        Some(payment) => payment,
        None => return Err(contract_error("payment required for redemption")),
    };

    if payment.denom != state.capital_denom {
        return Err(contract_error("payment should be made in capital denom"));
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.sequence += 1;
        state.redemptions.insert(Redemption {
            sequence: state.sequence,
            asset: redemption,
            capital: payment.amount.u128() as u64,
        });
        Ok(state)
    })?;

    let send = BankMsg::Send {
        to_address: state.raise.to_string(),
        amount: coins(redemption as u128, format!("{}.investment", state.raise)),
    }
    .into();

    let state = config_read(deps.storage).load()?;

    Ok(Response {
        submessages: vec![],
        messages: vec![send],
        attributes: vec![Attribute {
            key: format!("{}.redemption.sequence", env.contract.address),
            value: format!("{}", state.sequence),
        }],
        data: Option::None,
    })
}

pub fn try_issue_distribution(
    deps: DepsMut,
    env: Env,
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

    let payment = match info.funds.first() {
        Some(payment) => payment,
        None => return Err(contract_error("payment required for redemption")),
    };

    if payment.denom != state.capital_denom {
        return Err(contract_error("payment should be made in capital denom"));
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.sequence += 1;
        state.distributions.insert(Distribution {
            sequence: state.sequence,
            amount: payment.amount.u128() as u64,
        });
        Ok(state)
    })?;

    let state = config_read(deps.storage).load()?;

    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![Attribute {
            key: format!("{}.distribution.sequence", env.contract.address),
            value: format!("{}", state.sequence),
        }],
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
        QueryMsg::GetTransactions {} => to_binary(&Transactions {
            capital_calls: CapitalCalls {
                active: state.active_capital_call,
                closed: state.closed_capital_calls,
                cancelled: state.cancelled_capital_calls,
            },
            redemptions: state.redemptions,
            distributions: state.distributions,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Addr};
    use provwasm_mocks::{mock_dependencies, must_read_binary_file, ProvenanceMockQuerier};
    use provwasm_std::Marker;

    fn load_markers(querier: &mut ProvenanceMockQuerier) {
        let bin = must_read_binary_file("testdata/marker.json");
        let expected_marker: Marker = from_binary(&bin).unwrap();
        querier.with_markers(vec![expected_marker.clone()]);
    }

    #[test]
    fn initialization() {
        let mut deps = mock_dependencies(&[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &[]),
            InstantiateMsg {
                lp: Addr::unchecked("lp"),
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
                raise: Addr::unchecked("raise_1"),
                capital_denom: String::from("stable_coin"),
                commitment_denom: String::from("commitment_sub_raise"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                sequence: 0,
                active_capital_call: None,
                closed_capital_calls: HashSet::new(),
                cancelled_capital_calls: HashSet::new(),
                redemptions: HashSet::new(),
                distributions: HashSet::new(),
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
                raise: Addr::unchecked("raise_1"),
                capital_denom: String::from("stable_coin"),
                commitment_denom: String::from("commitment_sub_raise"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                sequence: 0,
                active_capital_call: None,
                closed_capital_calls: HashSet::new(),
                cancelled_capital_calls: HashSet::new(),
                redemptions: HashSet::new(),
                distributions: HashSet::new(),
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
                raise: Addr::unchecked("raise_1"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                commitment_denom: String::from("commitment_sub_raise"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                sequence: 0,
                active_capital_call: None,
                closed_capital_calls: HashSet::new(),
                cancelled_capital_calls: HashSet::new(),
                redemptions: HashSet::new(),
                distributions: HashSet::new(),
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &coins(20_000, "investment_raise")),
            HandleMsg::Accept {},
        )
        .unwrap();
        assert_eq!(5, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetStatus {}).unwrap();
        let status: Status = from_binary(&res).unwrap();
        assert_eq!(Status::Accepted, status);
    }

    #[test]
    fn issue_capital_call() {
        let mut deps = mock_dependencies(&vec![]);

        deps.querier
            .base
            .update_balance(Addr::unchecked("raise_1"), coins(10_000, "commitment"));

        config(&mut deps.storage)
            .save(&State {
                lp: Addr::unchecked("lp"),
                status: Status::Accepted,
                raise: Addr::unchecked("raise_1"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                commitment_denom: String::from("commitment"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                sequence: 0,
                active_capital_call: None,
                closed_capital_calls: HashSet::new(),
                cancelled_capital_calls: HashSet::new(),
                redemptions: HashSet::new(),
                distributions: HashSet::new(),
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &vec![]),
            HandleMsg::IssueCapitalCall {
                capital_call: CapitalCallIssuance {
                    amount: 10_000,
                    days_of_notice: None,
                },
            },
        )
        .unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn close_capital_call() {
        let mut deps = mock_dependencies(&coins(10_000, "stable_coin"));

        config(&mut deps.storage)
            .save(&State {
                lp: Addr::unchecked("lp"),
                status: Status::Accepted,
                raise: Addr::unchecked("raise_1"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                commitment_denom: String::from("commitment"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                sequence: 0,
                active_capital_call: Some(CapitalCall {
                    sequence: 1,
                    amount: 10_000,
                    days_of_notice: None,
                }),
                closed_capital_calls: HashSet::new(),
                cancelled_capital_calls: HashSet::new(),
                redemptions: HashSet::new(),
                distributions: HashSet::new(),
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &coins(10_000, "commitment")),
            HandleMsg::CloseCapitalCall {},
        )
        .unwrap();
        assert_eq!(1, res.messages.len());
    }

    #[test]
    fn issue_redemption() {
        let mut deps = mock_dependencies(&[]);

        load_markers(&mut deps.querier);

        config(&mut deps.storage)
            .save(&State {
                lp: Addr::unchecked("lp"),
                status: Status::Accepted,
                raise: Addr::unchecked("raise_1"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                commitment_denom: String::from("commitment_sub_raise"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                sequence: 0,
                active_capital_call: None,
                closed_capital_calls: HashSet::new(),
                cancelled_capital_calls: HashSet::new(),
                redemptions: HashSet::new(),
                distributions: HashSet::new(),
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &coins(5_000, "stable_coin")),
            HandleMsg::IssueRedemption { redemption: 5_000 },
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
                raise: Addr::unchecked("raise_1"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                commitment_denom: String::from("commitment_sub_raise"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                sequence: 0,
                active_capital_call: None,
                closed_capital_calls: HashSet::new(),
                cancelled_capital_calls: HashSet::new(),
                redemptions: HashSet::new(),
                distributions: HashSet::new(),
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &coins(5_000, "stable_coin")),
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
                raise: Addr::unchecked("raise_1"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                commitment_denom: String::from("commitment_sub_raise"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                sequence: 0,
                active_capital_call: None,
                closed_capital_calls: HashSet::new(),
                cancelled_capital_calls: HashSet::new(),
                redemptions: HashSet::new(),
                distributions: HashSet::new(),
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
