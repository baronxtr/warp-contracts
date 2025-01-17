use crate::execute::{account, controller, job};
use crate::query::condition;

use crate::state::{ACCOUNTS, CONFIG, FINISHED_JOBS, PENDING_JOBS};
use crate::{query, state::STATE, ContractError};
use cosmwasm_std::{
    entry_point, to_binary, Attribute, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response,
    StdError, StdResult, SubMsgResult, Uint64,
};
use warp_protocol::controller::account::Account;
use warp_protocol::controller::controller::{
    Config, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, State,
};
use warp_protocol::controller::job::{Job, JobStatus};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        current_job_id: Uint64::zero() + Uint64::one() * Uint64::pow(Uint64::new(1), 100),
    };

    let config = Config {
        owner: deps
            .api
            .addr_validate(&msg.owner.unwrap_or(info.sender.to_string()))?,
        warp_account_code_id: msg.warp_account_code_id,
        minimum_reward: msg.minimum_reward,
        creation_fee_percentage: msg.creation_fee,
        cancellation_fee_percentage: msg.cancellation_fee,
    };

    if config.creation_fee_percentage.u128() > 100 {
        return Err(ContractError::CreationFeeTooHigh {});
    }

    if config.cancellation_fee_percentage.u128() > 100 {
        return Err(ContractError::CancellationFeeTooHigh {});
    }

    STATE.save(deps.storage, &state)?;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateJob(data) => job::create_job(deps, env, info, data),
        ExecuteMsg::DeleteJob(data) => job::delete_job(deps, env, info, data),
        ExecuteMsg::UpdateJob(data) => job::update_job(deps, env, info, data),
        ExecuteMsg::ExecuteJob(data) => job::execute_job(deps, env, info, data),

        ExecuteMsg::CreateAccount(_) => account::create_account(deps, env, info),

        ExecuteMsg::UpdateConfig(data) => controller::update_config(deps, env, info, data),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::QueryResolveJobCondition(data) => {
            to_binary(&condition::query_condition_active(deps, env, data)?)
        }
        QueryMsg::QueryJob(data) => to_binary(&query::job::query_job(deps, env, data)?),
        QueryMsg::QueryJobs(data) => to_binary(&query::job::query_jobs(deps, env, data)?),
        QueryMsg::QueryResolveCondition(data) => {
            to_binary(&condition::query_resolve_condition(deps, env, data)?)
        }

        QueryMsg::SimulateQuery(data) => {
            to_binary(&query::controller::query_simulate_query(deps, env, data)?)
        }

        QueryMsg::QueryAccount(data) => to_binary(&query::account::query_account(deps, env, data)?),
        QueryMsg::QueryAccounts(data) => {
            to_binary(&query::account::query_accounts(deps, env, data)?)
        }

        QueryMsg::QueryConfig(data) => {
            to_binary(&query::controller::query_config(deps, env, data)?)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    CONFIG.save(
        deps.storage,
        &Config {
            owner: config.owner,
            warp_account_code_id: config.warp_account_code_id,
            minimum_reward: config.minimum_reward,
            creation_fee_percentage: config.creation_fee_percentage,
            cancellation_fee_percentage: config.cancellation_fee_percentage,
        },
    )?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        //account creation
        0 => {
            let reply = msg.result.into_result().map_err(StdError::generic_err)?;

            let event = reply
                .events
                .iter()
                .find(|event| {
                    event
                        .attributes
                        .iter()
                        .any(|attr| attr.key == "action" && attr.value == "instantiate")
                })
                .ok_or_else(|| StdError::generic_err("cannot find `instantiate` event"))?;

            let owner = event
                .attributes
                .iter()
                .cloned()
                .find(|attr| attr.key == "owner")
                .ok_or_else(|| StdError::generic_err("cannot find `owner` attribute"))?
                .value;

            let address = event
                .attributes
                .iter()
                .cloned()
                .find(|attr| attr.key == "contract_addr")
                .ok_or_else(|| StdError::generic_err("cannot find `contract_addr` attribute"))?
                .value;

            if ACCOUNTS().has(deps.storage, deps.api.addr_validate(&owner.clone())?) {
                return Err(ContractError::AccountAlreadyExists {});
            }

            ACCOUNTS().save(
                deps.storage,
                deps.api.addr_validate(&owner.clone())?,
                &Account {
                    owner: deps.api.addr_validate(&owner.clone())?,
                    account: deps.api.addr_validate(&address)?,
                },
            )?;
            Ok(Response::new()
                .add_attribute("action", "save_account")
                .add_attribute("owner", owner)
                .add_attribute("account_address", address))
        }
        //job execution
        _ => {
            let new_status = match msg.result {
                SubMsgResult::Ok(_) => JobStatus::Executed,
                SubMsgResult::Err(_) => JobStatus::Failed,
            };

            let job = PENDING_JOBS().load(deps.storage, msg.id)?;
            PENDING_JOBS().remove(deps.storage, msg.id)?;

            let _new_job = FINISHED_JOBS().update(deps.storage, msg.id, |j| match j {
                None => Ok(Job {
                    id: job.id,
                    owner: job.owner,
                    last_update_time: job.last_update_time,
                    name: job.name,
                    status: new_status,
                    condition: job.condition,
                    msgs: job.msgs,
                    reward: job.reward,
                }),
                Some(_) => Err(ContractError::JobAlreadyFinished {}),
            })?;

            let res_attrs = match msg.result {
                SubMsgResult::Err(e) => vec![Attribute::new("transaction_error", e)],
                _ => vec![],
            };

            Ok(Response::new()
                .add_attribute("action", "execute_reply")
                .add_attribute("job_id", job.id)
                .add_attribute("job_status", serde_json_wasm::to_string(&job.status)?)
                .add_attributes(res_attrs)) //todo: trying no attrs
        }
    }
}
