use crate::error::contract_error;
use cosmwasm_std::coin;
use cosmwasm_std::{
    coins, entry_point, to_binary, Addr, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult,
};
use provwasm_std::ProvenanceMsg;

use crate::error::ContractError;
use crate::msg::{CapitalCallIssuance, CapitalCalls, HandleMsg, QueryMsg, Terms, Transactions};
use crate::state::{
    config, config_read, CapitalCall, Distribution, Redemption, Status, Withdrawal,
};

pub type ContractResponse = Result<Response<ProvenanceMsg>, ContractError>;

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
            try_issue_capital_call(deps, env, info, capital_call)
        }
        HandleMsg::CloseCapitalCall { is_retroactive } => {
            try_close_capital_call(deps, env, info, is_retroactive)
        }
        HandleMsg::IssueRedemption {
            redemption,
            payment,
            is_retroactive,
        } => try_issue_redemption(deps, env, info, redemption, payment, is_retroactive),
        HandleMsg::IssueDistribution {
            payment,
            is_retroactive,
        } => try_issue_distribution(deps, env, info, payment, is_retroactive),
        HandleMsg::IssueWithdrawal { to, amount } => {
            try_issue_withdrawal(deps, env, info, to, amount)
        }
    }
}

pub fn try_recover(
    deps: DepsMut,
    info: MessageInfo,
    lp: Addr,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if info.sender != state.recovery_admin {
        return contract_error("only admin can recover subscription");
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.lp = lp;
        Ok(state)
    })?;

    Ok(Response::default())
}

pub fn try_accept(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Draft {
        return contract_error("subscription is not in draft status");
    }

    if info.sender != state.raise {
        return contract_error("only the raise contract can accept");
    }

    let commitment = match info.funds.first() {
        Some(commitment) => commitment,
        None => return contract_error("commitment required"),
    };

    if commitment.amount.u128() < state.capital_to_shares(state.min_commitment).into() {
        return contract_error("commitment less than minimum commitment");
    }

    if commitment.amount.u128() > state.capital_to_shares(state.max_commitment).into() {
        return contract_error("commitment more than maximum commitment");
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.status = Status::Accepted;
        Ok(state)
    })?;

    Ok(Response::default())
}

pub fn try_issue_capital_call(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    capital_call: CapitalCallIssuance,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return contract_error("subscription is not accepted");
    }

    if info.sender != state.raise {
        return contract_error("only the raise contract can issue capital call");
    }

    let commitment = deps
        .querier
        .query_balance(
            env.contract.address.clone(),
            format!("{}.commitment", state.raise),
        )
        .unwrap();

    if state.not_evenly_divisble(capital_call.amount) {
        return contract_error("capital call amount must be evenly divisible by capital per share");
    }

    if state.capital_to_shares(capital_call.amount) > commitment.amount.u128() as u64 {
        return contract_error("capital call larger than remaining commitment");
    }

    if capital_call.days_of_notice.unwrap_or(u16::MAX) < state.min_days_of_notice.unwrap_or(0) {
        return contract_error("not enough notice");
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

    Ok(Response::new().add_attribute(
        format!("{}.capital_call.sequence", env.contract.address),
        format!("{}", state.sequence),
    ))
}

pub fn try_close_capital_call(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    is_retroactive: bool,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return contract_error("subscription is not accepted");
    }

    if info.sender != state.raise {
        return contract_error("only the raise contract can close capital call");
    }

    let capital_call = match state.active_capital_call {
        Some(ref capital_call) => capital_call,
        None => return contract_error("no existing capital call issued"),
    };

    let investment = match info.funds.first() {
        Some(investment) => investment,
        None => return contract_error("no investment provided"),
    };

    if investment.amount.u128() != u128::from(state.capital_to_shares(capital_call.amount)) {
        return contract_error("incorrect investment provided");
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state
            .closed_capital_calls
            .insert(state.active_capital_call.take().unwrap());
        Ok(state)
    })?;

    let commitment_coin = coin(
        state.capital_to_shares(capital_call.amount) as u128,
        format!("{}.commitment", state.raise),
    );
    let capital_coin = coin(capital_call.amount as u128, state.capital_denom.clone());

    let send = BankMsg::Send {
        to_address: state.raise.to_string(),
        amount: if is_retroactive {
            vec![commitment_coin]
        } else {
            vec![capital_coin, commitment_coin]
        },
    };

    Ok(Response::new().add_message(send).add_attribute(
        format!("{}.capital_call.sequence", env.contract.address),
        format!("{}", state.sequence),
    ))
}

pub fn try_issue_redemption(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    redemption: u64,
    payment: u64,
    is_retroactive: bool,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return contract_error("subscription is not accepted");
    }

    if info.sender != state.raise {
        return contract_error("only the raise contract can issue redemption");
    }

    if !is_retroactive {
        let sent = match info.funds.first() {
            Some(sent) => sent,
            None => return contract_error("payment required for redemption"),
        };

        if sent.denom != state.capital_denom {
            return contract_error("payment should be made in capital denom");
        }

        if sent.amount.u128() != payment.into() {
            return contract_error("sent funds should match specified payment");
        }
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.sequence += 1;
        state.redemptions.insert(Redemption {
            sequence: state.sequence,
            asset: redemption,
            capital: payment,
        });
        Ok(state)
    })?;

    let send = BankMsg::Send {
        to_address: state.raise.to_string(),
        amount: coins(redemption as u128, format!("{}.investment", state.raise)),
    };

    let state = config_read(deps.storage).load()?;

    Ok(Response::new().add_message(send).add_attribute(
        format!("{}.redemption.sequence", env.contract.address),
        format!("{}", state.sequence),
    ))
}

pub fn try_issue_distribution(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    payment: u64,
    is_retroactive: bool,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return contract_error("subscription has not been accepted");
    }

    if info.sender != state.raise {
        return contract_error("only the raise contract can issue redemption");
    }

    if !is_retroactive {
        let sent = match info.funds.first() {
            Some(sent) => sent,
            None => return contract_error("payment required for distribution"),
        };

        if sent.denom != state.capital_denom {
            return contract_error("payment should be made in capital denom");
        }

        if sent.amount.u128() != payment.into() {
            return contract_error("sent funds should match specified payment");
        }
    }

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.sequence += 1;
        state.distributions.insert(Distribution {
            sequence: state.sequence,
            amount: payment,
        });
        Ok(state)
    })?;

    let state = config_read(deps.storage).load()?;

    Ok(Response::new().add_attribute(
        format!("{}.distribution.sequence", env.contract.address),
        format!("{}", state.sequence),
    ))
}

pub fn try_issue_withdrawal(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    to: Addr,
    amount: u64,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let state = config_read(deps.storage).load()?;

    if state.status != Status::Accepted {
        return contract_error("subscription has not been accepted");
    }

    if info.sender != state.lp {
        return contract_error("only the lp can withdraw");
    }

    let send = BankMsg::Send {
        to_address: to.to_string(),
        amount: coins(amount as u128, state.capital_denom),
    };

    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        state.sequence += 1;
        state.withdrawals.insert(Withdrawal {
            sequence: state.sequence,
            to,
            amount,
        });
        Ok(state)
    })?;

    let state = config_read(deps.storage).load()?;

    Ok(Response::new().add_message(send).add_attribute(
        format!("{}.withdrawal.sequence", env.contract.address),
        format!("{}", state.sequence),
    ))
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
            withdrawals: state.withdrawals,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::msg_at_index;
    use crate::mock::send_msg;
    use crate::state::State;
    use cosmwasm_std::testing::MockApi;
    use cosmwasm_std::testing::MockStorage;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::OwnedDeps;
    use cosmwasm_std::{coins, from_binary, Addr};
    use provwasm_mocks::{mock_dependencies, must_read_binary_file, ProvenanceMockQuerier};
    use provwasm_std::Marker;

    fn load_markers(querier: &mut ProvenanceMockQuerier) {
        let bin = must_read_binary_file("testdata/marker.json");
        let expected_marker: Marker = from_binary(&bin).unwrap();
        querier.with_markers(vec![expected_marker.clone()]);
    }

    pub fn default_deps(
        update_state: Option<fn(&mut State)>,
    ) -> OwnedDeps<MockStorage, MockApi, ProvenanceMockQuerier> {
        let mut deps = mock_dependencies(&[]);

        let mut state = State::test_default();
        if let Some(update) = update_state {
            update(&mut state);
        }
        config(&mut deps.storage).save(&state).unwrap();

        deps
    }

    #[test]
    fn recover() {
        execute(
            default_deps(None).as_mut(),
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
        let res = execute(
            default_deps(None).as_mut(),
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
        let mut deps = default_deps(None);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &coins(200, "investment_raise")),
            HandleMsg::Accept {},
        )
        .unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetStatus {}).unwrap();
        let status: Status = from_binary(&res).unwrap();
        assert_eq!(Status::Accepted, status);
    }

    #[test]
    fn accept_bad_actor() {
        let res = execute(
            default_deps(None).as_mut(),
            mock_env(),
            mock_info("bad_actor", &coins(200, "investment_raise")),
            HandleMsg::Accept {},
        );
        assert!(res.is_err());
    }

    #[test]
    fn issue_capital_call() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));
        deps.querier.base.update_balance(
            Addr::unchecked("cosmos2contract"),
            coins(100, "raise_1.commitment"),
        );

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
    fn issue_capital_call_bad_actor() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));
        deps.querier.base.update_balance(
            Addr::unchecked("cosmos2contract"),
            coins(100, "raise_1.commitment"),
        );

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::IssueCapitalCall {
                capital_call: CapitalCallIssuance {
                    amount: 10_000,
                    days_of_notice: None,
                },
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn issue_capital_call_with_bad_amount() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));
        deps.querier.base.update_balance(
            Addr::unchecked("cosmos2contract"),
            coins(100, "raise_1.commitment"),
        );

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &vec![]),
            HandleMsg::IssueCapitalCall {
                capital_call: CapitalCallIssuance {
                    amount: 10_001,
                    days_of_notice: None,
                },
            },
        );
        assert_eq!(true, res.is_err());
    }

    #[test]
    fn close_capital_call() {
        let mut deps = mock_dependencies(&coins(100_000, "stable_coin"));

        let mut state = State::test_default();
        state.status = Status::Accepted;
        state.active_capital_call = Some(CapitalCall {
            sequence: 1,
            amount: 100_000,
            days_of_notice: None,
        });
        config(&mut deps.storage).save(&state).unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &coins(1_000, "investment")),
            HandleMsg::CloseCapitalCall {
                is_retroactive: false,
            },
        )
        .unwrap();

        // verify commitment and capital sent to raise
        assert_eq!(1, res.messages.len());
        let (to_address, coins) = send_msg(msg_at_index(&res, 0));
        assert_eq!("raise_1", to_address);
        let capital_coin = coins.get(0).unwrap();
        assert_eq!(100_000, capital_coin.amount.u128());
        assert_eq!("stable_coin", capital_coin.denom);
        let commitment_coin = coins.get(1).unwrap();
        assert_eq!(1_000, commitment_coin.amount.u128());
        assert_eq!("raise_1.commitment", commitment_coin.denom);
    }

    #[test]
    fn close_capital_call_bad_actor() {
        let mut deps = mock_dependencies(&coins(100_000, "stable_coin"));

        let mut state = State::test_default();
        state.status = Status::Accepted;
        state.active_capital_call = Some(CapitalCall {
            sequence: 1,
            amount: 100_000,
            days_of_notice: None,
        });
        config(&mut deps.storage).save(&state).unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &coins(1_000, "investment")),
            HandleMsg::CloseCapitalCall {
                is_retroactive: false,
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn issue_redemption() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));
        load_markers(&mut deps.querier);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &coins(2_500, "stable_coin")),
            HandleMsg::IssueRedemption {
                redemption: 5_000,
                payment: 2_500,
                is_retroactive: false,
            },
        )
        .unwrap();

        // verify send message sent
        assert_eq!(1, res.messages.len());
        let (to_address, coins) = send_msg(msg_at_index(&res, 0));
        assert_eq!("raise_1", to_address);
        assert_eq!(5_000, coins.first().unwrap().amount.u128());
    }

    #[test]
    fn issue_redemption_bad_actor() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));
        load_markers(&mut deps.querier);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &coins(2_500, "stable_coin")),
            HandleMsg::IssueRedemption {
                redemption: 5_000,
                payment: 2_500,
                is_retroactive: false,
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn issue_distribution() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("raise_1", &coins(5_000, "stable_coin")),
            HandleMsg::IssueDistribution {
                payment: 5_000,
                is_retroactive: false,
            },
        )
        .unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn issue_distribution_bad_actor() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &coins(5_000, "stable_coin")),
            HandleMsg::IssueDistribution {
                payment: 5_000,
                is_retroactive: false,
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn withdraw() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::IssueWithdrawal {
                to: Addr::unchecked("lp_side_account"),
                amount: 10_000,
            },
        )
        .unwrap();

        // verify send message sent
        assert_eq!(1, res.messages.len());
        let (to_address, coins) = send_msg(msg_at_index(&res, 0));
        assert_eq!("lp_side_account", to_address);
        assert_eq!(10_000, coins.first().unwrap().amount.u128());
    }

    #[test]
    fn withdraw_bad_actor() {
        let mut deps = default_deps(Some(|state| {
            state.status = Status::Accepted;
        }));
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::IssueWithdrawal {
                to: Addr::unchecked("lp_side_account"),
                amount: 10_000,
            },
        );
        assert!(res.is_err());
    }
}
