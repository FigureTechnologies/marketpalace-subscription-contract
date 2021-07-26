use cosmwasm_std::{
    coins, entry_point, to_binary, Addr, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult,
};
use provwasm_std::{
    activate_marker, create_marker, grant_marker_access, withdraw_coins, MarkerAccess, MarkerType,
    ProvenanceMsg, ProvenanceQuerier,
};

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
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = State {
        lp: info.sender,
        status: Status::Draft,
        raise: msg.raise.clone(),
        admin: msg.admin,
        capital_denom: msg.capital_denom,
        commitment_denom: format!("commitment_{}_{}", env.contract.address, msg.raise),
        min_commitment: msg.min_commitment,
        max_commitment: msg.max_commitment,
        min_days_of_notice: msg.min_days_of_notice,
        capital_calls: vec![],
    };
    config(deps.storage).save(&state)?;

    let create = create_marker(
        msg.min_commitment as u128,
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
    let activate = activate_marker(state.commitment_denom)?;

    Ok(Response {
        submessages: vec![],
        messages: vec![create, grant, activate],
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
        HandleMsg::Accept {} => try_accept(deps, info),
        HandleMsg::IssueCapitalCall { capital_call } => {
            try_issue_capital_call(deps, info, capital_call)
        }
        HandleMsg::IssueRedemption { redemption } => try_issue_redemption(deps, info, redemption),
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

    let withraw = withdraw_coins(
        state.commitment_denom.clone(),
        investment.amount.u128(),
        state.commitment_denom,
        state.raise,
    )?;

    Ok(Response {
        submessages: vec![],
        messages: vec![withraw],
        attributes: vec![],
        data: Option::None,
    })
}

pub fn try_issue_capital_call(
    deps: DepsMut,
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

    let raise_marker = ProvenanceQuerier::new(&deps.querier)
        .get_marker_by_denom(format!("investment_{}", state.raise))?;
    let commitment = match raise_marker
        .coins
        .iter()
        .find(|coin| coin.denom == state.commitment_denom)
    {
        Some(commitment) => commitment,
        None => return Err(contract_error("no commitement held in raise")),
    };

    if terms.amount > commitment.amount.u128() as u64 {
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

    let send = BankMsg::Send {
        to_address: state.raise.to_string(),
        amount: coins(redemption as u128, format!("investment_{}", state.raise)),
    }
    .into();

    Ok(Response {
        submessages: vec![],
        messages: vec![send],
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
    use cosmwasm_std::{coins, from_binary, Addr, ContractResult, SystemError, SystemResult};
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
                raise: Addr::unchecked("raise_1"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                min_commitment: 10_000,
                max_commitment: 50_000,
                min_days_of_notice: None,
            },
        )
        .unwrap();
        assert_eq!(3, res.messages.len());

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
                raise: Addr::unchecked("raise_1"),
                capital_denom: String::from("stable_coin"),
                commitment_denom: String::from("commitment_sub_raise"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
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
                raise: Addr::unchecked("raise_1"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                commitment_denom: String::from("commitment_sub_raise"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                capital_calls: vec![],
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &coins(20_000, "investment_raise")),
            HandleMsg::Accept {},
        )
        .unwrap();
        assert_eq!(1, res.messages.len());

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

        load_markers(&mut deps.querier.base);

        config(&mut deps.storage)
            .save(&State {
                lp: Addr::unchecked("lp"),
                status: Status::Accepted,
                raise: Addr::unchecked("raise_1"),
                admin: Addr::unchecked("admin"),
                capital_denom: String::from("stable_coin"),
                commitment_denom: String::from("commitment_sub_1_raise_1"),
                min_commitment: 10_000,
                max_commitment: 100_000,
                min_days_of_notice: Some(10),
                capital_calls: vec![],
            })
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &vec![]),
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
                capital_calls: vec![],
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
                capital_calls: vec![],
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
