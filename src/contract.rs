use crate::error::contract_error;
use crate::raise_msg::RaiseExecuteMsg;
use cosmwasm_std::{coin, wasm_execute};
use cosmwasm_std::{
    coins, entry_point, to_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};
use provwasm_std::ProvenanceMsg;
use provwasm_std::ProvenanceQuery;

use crate::error::ContractError;
use crate::msg::{HandleMsg, QueryMsg};
use crate::state::{
    asset_exchange_storage, state_storage, state_storage_read, AssetExchangeAuthorization,
};

pub type ContractResponse = Result<Response<ProvenanceMsg>, ContractError>;

// And declare a custom Error variant for the ones where you will want to make use of it
#[entry_point]
pub fn execute(
    deps: DepsMut<ProvenanceQuery>,
    _env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> ContractResponse {
    match msg {
        HandleMsg::Recover { lp } => {
            let mut state = state_storage_read(deps.storage).load()?;

            if info.sender != state.admin {
                return contract_error("only admin can recover subscription");
            }

            state.lp = lp;
            state_storage(deps.storage).save(&state)?;

            Ok(Response::default())
        }
        HandleMsg::AuthorizeAssetExchange { exchange, to, memo } => {
            let state = state_storage(deps.storage).load()?;

            if info.sender != state.lp {
                return contract_error("only the lp can authorize asset exchange");
            }

            let mut exchanges = asset_exchange_storage(deps.storage)
                .may_load()?
                .unwrap_or_default();
            exchanges.push(AssetExchangeAuthorization { exchange, to, memo });
            asset_exchange_storage(deps.storage).save(&exchanges)?;

            Ok(Response::default())
        }
        HandleMsg::CompleteAssetExchange { exchange, to, memo } => {
            let state = state_storage(deps.storage).load()?;

            if info.sender == state.admin {
                let authorization = AssetExchangeAuthorization {
                    exchange: exchange.clone(),
                    to: to.clone(),
                    memo: memo.clone(),
                };
                let mut exchanges = asset_exchange_storage(deps.storage).load()?;

                let index = exchanges
                    .iter()
                    .position(|e| &authorization == e)
                    .ok_or("no previously authorized asset exchange matched")?;
                exchanges.remove(index);

                asset_exchange_storage(deps.storage).save(&exchanges)?;
            } else if info.sender != state.lp {
                return contract_error("only the lp or admin can complete asset exchange");
            }

            let mut funds = Vec::new();
            if let Some(investment) = exchange.investment {
                if investment < 0 {
                    funds.push(coin(
                        investment.unsigned_abs().into(),
                        state.investment_denom.clone(),
                    ));
                }
            }
            if let Some(commitment) = exchange.commitment {
                if commitment < 0 {
                    funds.push(coin(
                        commitment.unsigned_abs().into(),
                        state.commitment_denom.clone(),
                    ));
                }
            }
            if let Some(capital) = exchange.capital {
                if capital < 0 {
                    funds.push(coin(
                        capital.unsigned_abs().into(),
                        state.capital_denom.clone(),
                    ));
                }
            }

            Ok(Response::new().add_message(wasm_execute(
                state.raise,
                &RaiseExecuteMsg::CompleteAssetExchange { exchange, to, memo },
                funds,
            )?))
        }
        HandleMsg::IssueWithdrawal { to, amount } => {
            let state = state_storage(deps.storage).load()?;

            if info.sender != state.lp {
                return contract_error("only the lp can withdraw");
            }

            Ok(Response::new().add_message(BankMsg::Send {
                to_address: to.to_string(),
                amount: coins(amount.into(), state.capital_denom),
            }))
        }
    }
}

#[entry_point]
pub fn query(deps: Deps<ProvenanceQuery>, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetState {} => to_binary(&state_storage_read(deps.storage).load()?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::execute_args;
    use crate::mock::msg_at_index;
    use crate::mock::send_msg;
    use crate::msg::AssetExchange;
    use crate::state::asset_exchange_storage_read;
    use crate::state::State;
    use cosmwasm_std::testing::MockApi;
    use cosmwasm_std::testing::MockStorage;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::Addr;
    use cosmwasm_std::OwnedDeps;
    use provwasm_mocks::{mock_dependencies, ProvenanceMockQuerier};

    pub fn default_deps(
        update_state: Option<fn(&mut State)>,
    ) -> OwnedDeps<MockStorage, MockApi, ProvenanceMockQuerier, ProvenanceQuery> {
        let mut deps = mock_dependencies(&[]);

        let mut state = State::test_default();
        if let Some(update) = update_state {
            update(&mut state);
        }
        state_storage(&mut deps.storage).save(&state).unwrap();

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
    fn authorize_asset_exchange() {
        let mut deps = default_deps(None);

        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::AuthorizeAssetExchange {
                exchange: AssetExchange {
                    investment: Some(1_000),
                    commitment: Some(1_000),
                    capital: Some(1_000),
                    date: None,
                },
                to: Some(Addr::unchecked("lp_side_account")),
                memo: Some(String::from("memo")),
            },
        )
        .unwrap();

        // verify asset exchange authorization saved
        assert_eq!(
            1,
            asset_exchange_storage_read(&deps.storage)
                .load()
                .unwrap()
                .len()
        );
    }

    #[test]
    fn authorize_asset_exchange_bad_actor() {
        let mut deps = default_deps(None);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::AuthorizeAssetExchange {
                exchange: AssetExchange {
                    investment: Some(1_000),
                    commitment: Some(1_000),
                    capital: Some(1_000),
                    date: None,
                },
                to: Some(Addr::unchecked("lp_side_account")),
                memo: Some(String::from("memo")),
            },
        );

        // verify error
        assert!(res.is_err());
    }

    #[test]
    fn complete_asset_exchange_accept_only() {
        let mut deps = default_deps(None);

        let exchange = AssetExchange {
            investment: Some(1_000),
            commitment: Some(1_000),
            capital: Some(1_000),
            date: None,
        };
        let to = Some(Addr::unchecked("lp_side_account"));
        let memo = Some(String::from("memo"));
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::CompleteAssetExchange {
                exchange: exchange.clone(),
                to: to.clone(),
                memo: memo.clone(),
            },
        )
        .unwrap();

        // verify exec message sent
        assert_eq!(1, res.messages.len());
        let (recipient, msg, funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 0));
        assert_eq!("raise_1", recipient);
        assert_eq!(
            RaiseExecuteMsg::CompleteAssetExchange {
                exchange: exchange.clone(),
                to,
                memo
            },
            msg
        );

        // verify no funds sent
        assert_eq!(0, funds.len());
    }

    #[test]
    fn complete_asset_exchange_send_only() {
        let mut deps = default_deps(None);

        let exchange = AssetExchange {
            investment: Some(-1_000),
            commitment: Some(-1_000),
            capital: Some(-1_000),
            date: None,
        };
        let to = Some(Addr::unchecked("lp_side_account"));
        let memo = Some(String::from("memo"));
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::CompleteAssetExchange {
                exchange: exchange.clone(),
                to: to.clone(),
                memo: memo.clone(),
            },
        )
        .unwrap();

        // verify exec message sent
        assert_eq!(1, res.messages.len());
        let (recipient, msg, funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 0));
        assert_eq!("raise_1", recipient);
        assert_eq!(
            RaiseExecuteMsg::CompleteAssetExchange {
                exchange: exchange.clone(),
                to,
                memo
            },
            msg
        );

        // verify funds sent
        assert_eq!(3, funds.len());
        let investment = funds.get(0).unwrap();
        assert_eq!(1_000, investment.amount.u128());
        let commitment = funds.get(1).unwrap();
        assert_eq!(1_000, commitment.amount.u128());
        let capital = funds.get(2).unwrap();
        assert_eq!(1_000, capital.amount.u128());
    }

    #[test]
    fn complete_asset_exchange_admin() {
        let mut deps = default_deps(None);

        let exchange = AssetExchange {
            investment: Some(1_000),
            commitment: Some(1_000),
            capital: Some(1_000),
            date: None,
        };
        let to = Some(Addr::unchecked("lp_side_account"));
        let memo = Some(String::from("memo"));

        asset_exchange_storage(&mut deps.storage)
            .save(&vec![AssetExchangeAuthorization {
                exchange: exchange.clone(),
                to: to.clone(),
                memo: memo.clone(),
            }])
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("admin", &vec![]),
            HandleMsg::CompleteAssetExchange {
                exchange: exchange.clone(),
                to: to.clone(),
                memo: memo.clone(),
            },
        )
        .unwrap();

        // verify exec message sent
        assert_eq!(1, res.messages.len());
        let (recipient, msg, funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 0));
        assert_eq!("raise_1", recipient);
        assert_eq!(
            RaiseExecuteMsg::CompleteAssetExchange {
                exchange: exchange.clone(),
                to,
                memo
            },
            msg
        );

        // verify no funds sent
        assert_eq!(0, funds.len());

        // verify asset exchange authorization removed
        assert_eq!(
            0,
            asset_exchange_storage_read(&deps.storage)
                .load()
                .unwrap()
                .len()
        );
    }

    #[test]
    fn complete_asset_exchange_bad_actor() {
        let mut deps = default_deps(None);

        let exchange = AssetExchange {
            investment: Some(1_000),
            commitment: Some(1_000),
            capital: Some(1_000),
            date: None,
        };
        let to = Some(Addr::unchecked("lp_side_account"));
        let memo = Some(String::from("memo"));

        asset_exchange_storage(&mut deps.storage)
            .save(&vec![AssetExchangeAuthorization {
                exchange: exchange.clone(),
                to: to.clone(),
                memo: memo.clone(),
            }])
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::CompleteAssetExchange {
                exchange: exchange.clone(),
                to: to.clone(),
                memo: memo.clone(),
            },
        );

        // verify error
        assert!(res.is_err());
    }

    #[test]
    fn withdraw() {
        let mut deps = default_deps(None);

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
        let mut deps = default_deps(None);

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
