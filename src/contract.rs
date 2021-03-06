use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ConfigResponse, RefDataResponse, ReferenceData};
use crate::state::{RefData, State, config, config_read};
use std::collections::HashMap;
use num::BigUint;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        refs: HashMap::new(),
    };
    config(deps.storage).save(&state)?;
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Relay { symbols, rates, resolve_times, request_ids } => update_refs(deps, &symbols, &rates, &resolve_times, &request_ids),
    }
}

pub fn update_refs(deps: DepsMut, symbols: &[String], new_rates: &[u64], new_resolve_times: &[u64], new_request_ids: &[u64]) -> Result<Response, ContractError> {
    let len = symbols.len();
    if new_rates.len() != len || new_request_ids.len() != len || new_resolve_times.len() != len {
        return Err(ContractError::DifferentArrayLength {});
    }
    let mut state = config(deps.storage).load()?;
    for idx in 0..len {
        state.refs.insert(symbols[idx].clone(), RefData {
            rate: new_rates[idx],
            resolve_time: new_resolve_times[idx],
            request_id: new_request_ids[idx],
        });
    };
    config(deps.storage).save(&state)?;
    Ok(Response::default())
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetRefs {} => to_binary(&query_refs(deps)?),
        QueryMsg::GetReferenceData { base, quote } => {
            let base_ref_data = get_ref_data(deps, env.clone(), base).unwrap();
            let quote_ref_data = get_ref_data(deps, env.clone(), quote).unwrap();
            to_binary(&ReferenceData {
                rate: (base_ref_data.rate * BigUint::from(1e18 as u128)) / quote_ref_data.rate,
                last_updated_base: BigUint::from(base_ref_data.last_update),
                last_updated_quote: BigUint::from(quote_ref_data.last_update),
            })
        }
    }
}

fn query_refs(deps: Deps) -> StdResult<ConfigResponse> {
    let state = config_read(deps.storage).load()?;
    Ok(state)
}

fn get_ref_data(deps: Deps, env: Env, symbol: String) -> Result<RefDataResponse, ContractError> {
    if symbol == String::from("USD") {
        return Ok(RefDataResponse {
            rate: BigUint::from(1e9 as u128),
            last_update: BigUint::from(env.block.time.nanos()),
        });
    }
    let state = config_read(deps.storage).load()?;
    let ref_data = state.refs.get(&symbol).unwrap();
    if ref_data.resolve_time <= 0 {
        return Err(ContractError::RefDataNotAvailable {});
    }
    return Ok(RefDataResponse {
        rate: BigUint::from(ref_data.rate),
        last_update:BigUint::from(ref_data.resolve_time),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{from_binary};
    use std::collections::HashMap;

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetRefs{}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(HashMap::new(), value.refs);
    }

    #[test]
    fn insert_one() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::Relay { symbols: vec![String::from("ETH")], rates: vec![1u64], resolve_times: vec![2u64], request_ids: vec![3u64] };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetRefs {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        let mut mock_map = HashMap::new();

        mock_map.insert(String::from("ETH"), RefData{rate: 1u64, resolve_time: 2u64, request_id: 3u64});

        assert_eq!(mock_map, value.refs);
    }

    #[test]
    fn insert_batch() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::Relay { symbols: vec![String::from("ETH"), String::from("BAND")], rates: vec![1u64, 100u64], resolve_times: vec![2u64, 200u64], request_ids: vec![3u64, 300u64] };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetRefs {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        let mut mock_map = HashMap::new();

        mock_map.insert(String::from("ETH"), RefData{rate: 1u64, resolve_time: 2u64, request_id: 3u64});
        mock_map.insert(String::from("BAND"), RefData{rate: 100u64, resolve_time: 200u64, request_id: 300u64});

        assert_eq!(mock_map, value.refs);
    }

    #[test]
    fn update_rate() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::Relay { symbols: vec![String::from("MATIC")], rates: vec![12u64], resolve_times: vec![124824u64], request_ids: vec![69u64] };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetRefs {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();

        let mut mock_map01 = HashMap::new();
        mock_map01.insert(String::from("MATIC"), RefData{rate: 12u64, resolve_time: 124824u64, request_id: 69u64});
        assert_eq!(mock_map01, value.refs);

        let info = mock_info("sender", &[]);
        let msg = ExecuteMsg::Relay { symbols: vec![String::from("MATIC")], rates: vec![24u64], resolve_times: vec![124824u64], request_ids: vec![69u64] };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetRefs {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();

        let mut mock_map02 = HashMap::new();
        mock_map02.insert(String::from("MATIC"), RefData{rate: 24u64, resolve_time: 124824u64, request_id: 69u64});
        assert_eq!(mock_map02, value.refs);
    }

    #[test]
    fn query_test_valid() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::Relay { symbols: vec![String::from("MATIC")], rates: vec![112u64], resolve_times: vec![1625108298000000000u64], request_ids: vec![124u64] };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let _info = mock_info("querier", &[]);
        let msg = QueryMsg::GetReferenceData { base: String::from("USD"), quote: String::from("MATIC") };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: ReferenceData = from_binary(&res).unwrap();

        assert_eq!(ReferenceData{rate: BigUint::from(8928571428571428571428571u128), last_updated_base: BigUint::from(1571797419879305533u128), last_updated_quote: BigUint::from(1625108298000000000u128)}, value);
    }
}
