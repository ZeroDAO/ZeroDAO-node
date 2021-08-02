use sp_runtime::{DispatchResult,DispatchError};
pub trait ChallengeInfo {
    
}

pub trait ChallengeBase<AccountId, AppId, Balance> {
    
    fn is_all_harvest(app_id: &AppId) -> bool;

    fn new(
        app_id: &AppId,
        who: &AccountId,
        path_finder: &AccountId,
        fee: Balance,
        staking: Balance,
        target: &AccountId,
        quantity: u32,
        value: u32,
    ) -> DispatchResult;

    fn next(
        app_id: &AppId,
        who: &AccountId,
        target: &AccountId,
        count: u32,
        up: impl FnOnce(bool,u32) -> Result<u32, DispatchError>,
    ) -> DispatchResult;
}
