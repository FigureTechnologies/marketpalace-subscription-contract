use crate::error::contract_error;
use crate::raise_msg::RaiseExecuteMsg;
use cosmwasm_std::{coin, wasm_execute, Addr, StdError, Storage};
use cosmwasm_std::{
    coins, entry_point, to_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};
use provwasm_std::{transfer_marker_coins, ProvenanceMsg};
use provwasm_std::{ProvenanceQuerier, ProvenanceQuery};
use std::collections::HashMap;
use std::vec::IntoIter;

use crate::error::ContractError;
use crate::msg::{AssetExchange, HandleMsg, QueryMsg};
use crate::state::{
    asset_exchange_authorization_storage, asset_exchange_authorization_storage_read, state_storage,
    state_storage_read, AssetExchangeAuthorization,
};

pub type ContractResponse = Result<Response<ProvenanceMsg>, ContractError>;

// And declare a custom Error variant for the ones where you will want to make use of it
#[entry_point]
pub fn execute(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
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
        HandleMsg::AuthorizeAssetExchange {
            exchanges,
            to,
            memo,
        } => {
            let state = state_storage(deps.storage).load()?;

            if info.sender != state.lp {
                return contract_error("only the lp can authorize asset exchanges");
            }

            for exchange in &exchanges {
                if exchange.capital.is_some() {
                    if let Some(denom_value) = &exchange.capital_denom {
                        if !state.like_capital_denoms.contains(denom_value) {
                            return contract_error("unsupported capital denom");
                        }
                    } else if state.like_capital_denoms.len() > 1 {
                        return contract_error("specified capital denom required");
                    }
                }
            }

            let mut authorizations = asset_exchange_authorization_storage(deps.storage)
                .may_load()?
                .unwrap_or_default();
            authorizations.push(AssetExchangeAuthorization {
                exchanges,
                to,
                memo,
            });
            asset_exchange_authorization_storage(deps.storage).save(&authorizations)?;

            Ok(Response::default())
        }
        HandleMsg::CancelAssetExchangeAuthorization {
            exchanges,
            to,
            memo,
        } => {
            let state = state_storage(deps.storage).load()?;

            if info.sender != state.lp {
                return contract_error("only the lp can cancel asset exchange authorization");
            }

            remove_asset_exchange_authorization(deps.storage, exchanges, to, memo, true)?;

            Ok(Response::default())
        }
        HandleMsg::CompleteAssetExchange {
            exchanges,
            to,
            memo,
        } => {
            let state = state_storage(deps.storage).load()?;

            if info.sender != state.lp && info.sender != state.admin {
                return contract_error("only the lp or admin can complete asset exchange");
            }

            remove_asset_exchange_authorization(
                deps.storage,
                exchanges.clone(),
                to.clone(),
                memo.clone(),
                info.sender == state.admin,
            )?;

            let mut funds = Vec::new();

            let total_investment: i64 = exchanges.iter().filter_map(|e| e.investment).sum();
            if total_investment < 0 {
                funds.push(coin(
                    total_investment.unsigned_abs().into(),
                    state.investment_denom.clone(),
                ));
            }

            let total_commitment: i64 = exchanges
                .iter()
                .filter_map(|e| e.commitment_in_shares)
                .sum();
            if total_commitment < 0 {
                funds.push(coin(
                    total_commitment.unsigned_abs().into(),
                    state.commitment_denom.clone(),
                ));
            }

            let response = Response::new();

            let total_capital_per_denom: HashMap<String, i64> = exchanges.iter().try_fold(
                HashMap::new(),
                |mut acc,
                 AssetExchange {
                     capital_denom,
                     capital,
                     ..
                 }| {
                    if let Some(capital_value) = capital {
                        let denom = if let Some(denom_value) = capital_denom {
                            denom_value.clone()
                        } else if state.like_capital_denoms.len() == 1 {
                            state.like_capital_denoms.first().unwrap().clone()
                        } else {
                            return Err(StdError::generic_err("no capital denom"));
                        };

                        *acc.entry(denom).or_insert(0) += capital_value;
                    }

                    Ok(acc)
                },
            )?;

            let response = total_capital_per_denom.into_iter().try_fold(
                response,
                |response, (capital_denom, capital_sum)| -> Result<_, StdError> {
                    Ok(if capital_sum < 0 {
                        match &state.required_capital_attribute {
                            None => {
                                funds.push(coin(capital_sum.unsigned_abs().into(), capital_denom));
                                response
                            }
                            Some(_required_capital_attribute) => {
                                let marker_transfer = transfer_marker_coins(
                                    capital_sum.unsigned_abs().into(),
                                    &capital_denom,
                                    state.raise.clone(),
                                    env.contract.address.clone(),
                                )?;
                                response.add_message(marker_transfer)
                            }
                        }
                    } else {
                        response
                    })
                },
            )?;

            funds.sort_by_key(|coin| coin.denom.clone());

            Ok(response.add_message(wasm_execute(
                &state.raise,
                &RaiseExecuteMsg::CompleteAssetExchange {
                    exchanges,
                    to,
                    memo,
                },
                funds,
            )?))
        }
        HandleMsg::IssueWithdrawal {
            capital_denom,
            to,
            amount,
        } => {
            let state = state_storage(deps.storage).load()?;

            if info.sender != state.lp {
                return contract_error("only the lp can withdraw");
            }

            let capital_denom = if let Some(denom_value) = capital_denom {
                if state.like_capital_denoms.contains(&denom_value) {
                    denom_value
                } else {
                    return contract_error("unsupported capital denom");
                }
            } else if state.like_capital_denoms.len() == 1 {
                state.like_capital_denoms.first().unwrap().clone()
            } else {
                return contract_error("no capital denom");
            };

            let response = match state.required_capital_attribute {
                None => {
                    let send_capital = BankMsg::Send {
                        to_address: to.to_string(),
                        amount: coins(amount.into(), capital_denom),
                    };
                    Response::new().add_message(send_capital)
                }
                Some(required_capital_attribute) => {
                    if !query_attributes(deps, &to)
                        .any(|attr| attr.name == required_capital_attribute)
                    {
                        return contract_error(
                            format!(
                                "{} does not have required attribute of {}",
                                &to, &required_capital_attribute
                            )
                            .as_str(),
                        );
                    }

                    let marker_transfer = transfer_marker_coins(
                        amount.into(),
                        &capital_denom,
                        to,
                        env.contract.address,
                    )?;
                    Response::new().add_message(marker_transfer)
                }
            };
            Ok(response)
        }
    }
}

fn query_attributes(
    deps: DepsMut<ProvenanceQuery>,
    address: &Addr,
) -> IntoIter<provwasm_std::Attribute> {
    ProvenanceQuerier::new(&deps.querier)
        .get_attributes(address.clone(), None as Option<String>)
        .unwrap()
        .attributes
        .into_iter()
}

fn remove_asset_exchange_authorization(
    storage: &mut dyn Storage,
    exchanges: Vec<AssetExchange>,
    to: Option<Addr>,
    memo: Option<String>,
    authorization_required: bool,
) -> Result<(), ContractError> {
    match asset_exchange_authorization_storage(storage).may_load()? {
        Some(mut authorizations) => {
            let authorization = AssetExchangeAuthorization {
                exchanges,
                to,
                memo,
            };
            let index = authorizations.iter().position(|e| &authorization == e);
            match index {
                Some(index) => {
                    authorizations.remove(index);
                    asset_exchange_authorization_storage(storage).save(&authorizations)?;
                }
                None => {
                    if authorization_required {
                        return Err(ContractError::from(
                            "no previously authorized asset exchange matched",
                        ));
                    }
                }
            }
        }
        None => {
            if authorization_required {
                return Err(ContractError::from(
                    "no previously authorized asset exchange matched",
                ));
            }
        }
    }

    Ok(())
}

#[entry_point]
pub fn query(deps: Deps<ProvenanceQuery>, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetState {} => to_binary(&state_storage_read(deps.storage).load()?),
        QueryMsg::GetAssetExchangeAuthorizations {} => to_binary(
            &asset_exchange_authorization_storage_read(deps.storage)
                .may_load()?
                .unwrap_or_default(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::send_msg;
    use crate::mock::{execute_args, load_markers};
    use crate::mock::{marker_transfer_msg, msg_at_index};
    use crate::msg::AssetExchange;
    use crate::state::asset_exchange_authorization_storage_read;
    use crate::state::State;
    use cosmwasm_std::testing::MockStorage;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::testing::{MockApi, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::Addr;
    use cosmwasm_std::OwnedDeps;
    use provwasm_mocks::{mock_dependencies, ProvenanceMockQuerier};
    use provwasm_std::MarkerMsgParams;

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

    pub fn capital_coin_deps(
        update_state: Option<fn(&mut State)>,
    ) -> OwnedDeps<MockStorage, MockApi, ProvenanceMockQuerier, ProvenanceQuery> {
        let mut deps = mock_dependencies(&[]);

        let mut state = State::test_capital_coin();
        if let Some(update) = update_state {
            update(&mut state);
        }
        state_storage(&mut deps.storage).save(&state).unwrap();

        deps
    }

    pub fn restricted_capital_coin_deps(
        update_state: Option<fn(&mut State)>,
    ) -> OwnedDeps<MockStorage, MockApi, ProvenanceMockQuerier, ProvenanceQuery> {
        let mut deps = mock_dependencies(&[]);

        let mut state = State::test_restricted_capital_coin();
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
                exchanges: vec![AssetExchange {
                    investment: Some(1_000),
                    commitment_in_shares: Some(1_000),
                    capital_denom: Some(String::from("stable_coin")),
                    capital: Some(1_000),
                    date: None,
                }],
                to: Some(Addr::unchecked("lp_side_account")),
                memo: Some(String::from("memo")),
            },
        )
        .unwrap();

        // verify asset exchange authorization saved
        assert_eq!(
            1,
            asset_exchange_authorization_storage_read(&deps.storage)
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
                exchanges: vec![AssetExchange {
                    investment: Some(1_000),
                    commitment_in_shares: Some(1_000),
                    capital_denom: Some(String::from("stable_coin")),
                    capital: Some(1_000),
                    date: None,
                }],
                to: Some(Addr::unchecked("lp_side_account")),
                memo: Some(String::from("memo")),
            },
        );

        // verify error
        assert!(res.is_err());
    }

    #[test]
    fn authorize_asset_exchange_required_cap_denom_not_specified() {
        let mut deps = default_deps(Some(|state| {
            state.like_capital_denoms = vec![String::from("a"), String::from("b")]
        }));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::AuthorizeAssetExchange {
                exchanges: vec![AssetExchange {
                    investment: Some(1_000),
                    commitment_in_shares: Some(1_000),
                    capital_denom: None,
                    capital: Some(1_000),
                    date: None,
                }],
                to: Some(Addr::unchecked("lp_side_account")),
                memo: Some(String::from("memo")),
            },
        );

        // verify error
        assert!(res.is_err());
    }

    #[test]
    fn authorize_asset_exchange_unsupported_denom() {
        let mut deps = default_deps(Some(|state| {
            state.like_capital_denoms = vec![String::from("a")]
        }));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::AuthorizeAssetExchange {
                exchanges: vec![AssetExchange {
                    investment: Some(1_000),
                    commitment_in_shares: Some(1_000),
                    capital_denom: Some(String::from("b")),
                    capital: Some(1_000),
                    date: None,
                }],
                to: Some(Addr::unchecked("lp_side_account")),
                memo: Some(String::from("memo")),
            },
        );

        // verify error
        assert!(res.is_err());
    }

    #[test]
    fn cancel_asset_exchange_authorization() {
        let mut deps = default_deps(None);

        let exchange = AssetExchange {
            investment: Some(1_000),
            commitment_in_shares: Some(1_000),
            capital_denom: Some(String::from("stable_coin")),
            capital: Some(1_000),
            date: None,
        };
        let to = Some(Addr::unchecked("lp_side_account"));
        let memo = Some(String::from("memo"));

        asset_exchange_authorization_storage(&mut deps.storage)
            .save(&vec![AssetExchangeAuthorization {
                exchanges: vec![exchange.clone()],
                to: to.clone(),
                memo: memo.clone(),
            }])
            .unwrap();

        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::CancelAssetExchangeAuthorization {
                exchanges: vec![exchange.clone()],
                to: to.clone(),
                memo: memo.clone(),
            },
        )
        .unwrap();

        // verify asset exchange authorization removed
        assert_eq!(
            0,
            asset_exchange_authorization_storage_read(&deps.storage)
                .load()
                .unwrap()
                .len()
        );
    }

    #[test]
    fn cancel_asset_exchange_authorization_bad_actor() {
        let mut deps = default_deps(None);

        let exchange = AssetExchange {
            investment: Some(1_000),
            commitment_in_shares: Some(1_000),
            capital_denom: Some(String::from("stable_coin")),
            capital: Some(1_000),
            date: None,
        };
        let to = Some(Addr::unchecked("lp_side_account"));
        let memo = Some(String::from("memo"));

        asset_exchange_authorization_storage(&mut deps.storage)
            .save(&vec![AssetExchangeAuthorization {
                exchanges: vec![exchange.clone()],
                to: to.clone(),
                memo: memo.clone(),
            }])
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::CancelAssetExchangeAuthorization {
                exchanges: vec![exchange.clone()],
                to: to.clone(),
                memo: memo.clone(),
            },
        );

        // verify error
        assert!(res.is_err());
    }

    #[test]
    fn complete_asset_exchange_accept_only() {
        let mut deps = capital_coin_deps(None);
        load_markers(&mut deps.querier);
        let exchange = AssetExchange {
            investment: Some(1_000),
            commitment_in_shares: Some(1_000),
            capital_denom: Some(String::from("stable_coin")),
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
                exchanges: vec![exchange.clone()],
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
                exchanges: vec![exchange.clone()],
                to,
                memo
            },
            msg
        );

        // verify no funds sent
        assert_eq!(0, funds.len());
    }

    #[test]
    fn complete_asset_exchange_accept_multiple_cap_denom() {
        let mut deps = capital_coin_deps(Some(|state| {
            state.like_capital_denoms = vec![String::from("cap_a"), String::from("cap_b")];
        }));
        load_markers(&mut deps.querier);
        let exchanges = vec![
            AssetExchange {
                investment: Some(1_000),
                commitment_in_shares: Some(1_000),
                capital_denom: Some(String::from("cap_a")),
                capital: Some(1_000),
                date: None,
            },
            AssetExchange {
                investment: Some(1_000),
                commitment_in_shares: Some(1_000),
                capital_denom: Some(String::from("cap_b")),
                capital: Some(1_000),
                date: None,
            },
        ];
        let to = Some(Addr::unchecked("lp_side_account"));
        let memo = Some(String::from("memo"));
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::CompleteAssetExchange {
                exchanges: exchanges.clone(),
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
                exchanges: exchanges.clone(),
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
        let mut deps = capital_coin_deps(None);
        load_markers(&mut deps.querier);
        let exchange = AssetExchange {
            investment: Some(-1_000),
            commitment_in_shares: Some(-1_000),
            capital_denom: Some(String::from("stable_coin")),
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
                exchanges: vec![exchange.clone(), exchange.clone()],
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
                exchanges: vec![exchange.clone(), exchange.clone()],
                to,
                memo
            },
            msg
        );

        // verify funds sent
        assert_eq!(3, funds.len());
        let capital = funds.get(0).unwrap();
        assert_eq!(2_000, capital.amount.u128());
        let commitment = funds.get(1).unwrap();
        assert_eq!(2_000, commitment.amount.u128());
        let investment = funds.get(2).unwrap();
        assert_eq!(2_000, investment.amount.u128());
    }

    #[test]
    fn complete_asset_exchange_restricted_marker_send_only() {
        let mut deps = restricted_capital_coin_deps(None);
        deps.querier
            .with_attributes("raise_1", &[("capital.test", "", "")]);
        load_markers(&mut deps.querier);
        let exchange = AssetExchange {
            investment: Some(-1_000),
            commitment_in_shares: Some(-1_000),
            capital_denom: Some(String::from("restricted_capital_coin")),
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
                exchanges: vec![exchange.clone(), exchange.clone()],
                to: to.clone(),
                memo: memo.clone(),
            },
        )
        .unwrap();

        // verify exec message sent
        assert_eq!(2, res.messages.len());
        let (recipient, msg, funds) = execute_args::<RaiseExecuteMsg>(msg_at_index(&res, 1));
        assert_eq!("raise_1", recipient);
        assert_eq!(
            RaiseExecuteMsg::CompleteAssetExchange {
                exchanges: vec![exchange.clone(), exchange.clone()],
                to,
                memo
            },
            msg
        );

        // verify funds sent
        assert_eq!(
            &MarkerMsgParams::TransferMarkerCoins {
                coin: coin(2_000, "restricted_capital_coin"),
                to: Addr::unchecked("raise_1"),
                from: Addr::unchecked(MOCK_CONTRACT_ADDR),
            },
            marker_transfer_msg(msg_at_index(&res, 0)),
        );
        assert_eq!(2, funds.len());
        // let capital = funds.get(0).unwrap();
        // assert_eq!(2_000, capital.amount.u128());
        let commitment = funds.get(0).unwrap();
        assert_eq!(2_000, commitment.amount.u128());
        let investment = funds.get(1).unwrap();
        assert_eq!(2_000, investment.amount.u128());
    }

    #[test]
    fn complete_asset_exchange_admin() {
        let mut deps = capital_coin_deps(None);
        load_markers(&mut deps.querier);
        let exchange = AssetExchange {
            investment: Some(1_000),
            commitment_in_shares: Some(1_000),
            capital_denom: Some(String::from("stable_coin")),
            capital: Some(1_000),
            date: None,
        };
        let to = Some(Addr::unchecked("lp_side_account"));
        let memo = Some(String::from("memo"));

        asset_exchange_authorization_storage(&mut deps.storage)
            .save(&vec![AssetExchangeAuthorization {
                exchanges: vec![exchange.clone()],
                to: to.clone(),
                memo: memo.clone(),
            }])
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("admin", &vec![]),
            HandleMsg::CompleteAssetExchange {
                exchanges: vec![exchange.clone()],
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
                exchanges: vec![exchange.clone()],
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
            asset_exchange_authorization_storage_read(&deps.storage)
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
            commitment_in_shares: Some(1_000),
            capital_denom: Some(String::from("stable_coin")),
            capital: Some(1_000),
            date: None,
        };
        let to = Some(Addr::unchecked("lp_side_account"));
        let memo = Some(String::from("memo"));

        asset_exchange_authorization_storage(&mut deps.storage)
            .save(&vec![AssetExchangeAuthorization {
                exchanges: vec![exchange.clone()],
                to: to.clone(),
                memo: memo.clone(),
            }])
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bad_actor", &vec![]),
            HandleMsg::CompleteAssetExchange {
                exchanges: vec![exchange.clone()],
                to: to.clone(),
                memo: memo.clone(),
            },
        );

        // verify error
        assert!(res.is_err());
    }

    #[test]
    fn withdraw() {
        let mut deps = capital_coin_deps(None);
        load_markers(&mut deps.querier);
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::IssueWithdrawal {
                to: Addr::unchecked("lp_side_account"),
                amount: 10_000,
                capital_denom: None,
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
    fn withdraw_restricted_marker() {
        let mut deps = restricted_capital_coin_deps(None);
        deps.querier
            .with_attributes("lp_side_account", &[("capital.test", "", "")]);
        load_markers(&mut deps.querier);
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::IssueWithdrawal {
                to: Addr::unchecked("lp_side_account"),
                amount: 10_000,
                capital_denom: None,
            },
        )
        .unwrap();

        // verify send message sent
        assert_eq!(1, res.messages.len());
        assert_eq!(
            &MarkerMsgParams::TransferMarkerCoins {
                coin: coin(10_000, "restricted_capital_coin"),
                to: Addr::unchecked("lp_side_account"),
                from: Addr::unchecked(MOCK_CONTRACT_ADDR),
            },
            marker_transfer_msg(msg_at_index(&res, 0)),
        );
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
                capital_denom: None,
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn withdraw_required_cap_denom_not_specified() {
        let mut deps = default_deps(Some(|state| {
            state.like_capital_denoms = vec![String::from("a"), String::from("b")];
        }));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::IssueWithdrawal {
                to: Addr::unchecked("lp_side_account"),
                amount: 10_000,
                capital_denom: None,
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn withdraw_unsupported_cap_denom() {
        let mut deps = default_deps(Some(|state| {
            state.like_capital_denoms = vec![String::from("a")];
        }));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("lp", &vec![]),
            HandleMsg::IssueWithdrawal {
                to: Addr::unchecked("lp_side_account"),
                amount: 10_000,
                capital_denom: Some(String::from("b")),
            },
        );
        assert!(res.is_err());
    }
}
