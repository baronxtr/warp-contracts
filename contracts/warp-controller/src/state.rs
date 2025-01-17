use cosmwasm_std::Addr;
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, Map, MultiIndex, UniqueIndex};
use warp_protocol::controller::account::Account;

use warp_protocol::controller::controller::{Config, State};
use warp_protocol::controller::job::Job;

pub struct JobIndexes<'a> {
    pub reward: UniqueIndex<'a, (u128, u64), Job>,
    pub publish_time: MultiIndex<'a, u64, Job, u64>,
}

impl IndexList<Job> for JobIndexes<'_> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Job>> + '_> {
        let v: Vec<&dyn Index<Job>> = vec![&self.reward, &self.publish_time];
        Box::new(v.into_iter())
    }
}

#[allow(non_snake_case)]
pub fn PENDING_JOBS<'a>() -> IndexedMap<'a, u64, Job, JobIndexes<'a>> {
    let indexes = JobIndexes {
        reward: UniqueIndex::new(
            |job| (job.reward.u128(), job.id.u64()),
            "pending_jobs__reward",
        ),
        publish_time: MultiIndex::new(
            |_pk, job| job.last_update_time.u64(),
            "pending_jobs",
            "pending_jobs__publish_timestamp",
        ),
    };
    IndexedMap::new("pending_jobs", indexes)
}

#[allow(non_snake_case)]
pub fn FINISHED_JOBS<'a>() -> IndexedMap<'a, u64, Job, JobIndexes<'a>> {
    let indexes = JobIndexes {
        reward: UniqueIndex::new(
            |job| (job.reward.u128(), job.id.u64()),
            "finished_jobs__reward",
        ),
        publish_time: MultiIndex::new(
            |_pk, job| job.last_update_time.u64(),
            "finished_jobs",
            "finished_jobs__publish_timestamp",
        ),
    };
    IndexedMap::new("finished_jobs", indexes)
}

pub struct AccountIndexes<'a> {
    pub account: UniqueIndex<'a, Addr, Account>,
}

impl IndexList<Account> for AccountIndexes<'_> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Account>> + '_> {
        let v: Vec<&dyn Index<Account>> = vec![&self.account];
        Box::new(v.into_iter())
    }
}

#[allow(non_snake_case)]
pub fn ACCOUNTS<'a>() -> IndexedMap<'a, Addr, Account, AccountIndexes<'a>> {
    let indexes = AccountIndexes {
        account: UniqueIndex::new(|account| account.account.clone(), "accounts__account"),
    };
    IndexedMap::new("accounts", indexes)
}

pub const QUERY_PAGE_SIZE: u32 = 50;
pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
