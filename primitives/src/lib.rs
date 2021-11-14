#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    codec::{Decode, Encode},
    RuntimeDebug,
};
use sp_runtime::{Perbill, traits::{AtLeast32Bit, AtLeast32BitUnsigned,Zero}};
use sp_std::convert::TryInto;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Debug, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
    ZDAO,
    SOCI,
}

pub type AppId = [u8; 8];

/// Balance of an account.
pub type Balance = u128;

pub const SWEEPER_PERIOD: u64 = 500;

/// When other users receive their earnings, they receive that percentage of the earnings.
pub const SWEEPER_PICKUP_RATIO: Perbill = Perbill::from_perthousand(20);

pub mod per_social_currency {
    use super::Perbill;

    pub const MIN_TRUST_COUNT: u32 = 150;
    /// Reserve the owner's free balance. The percentage can be adjusted by the community.
    pub const PRE_RESERVED: Perbill = Perbill::from_percent(10);

    /// Transfer to the social currency of users trusted by the owner.
    pub const PRE_SHARE: Perbill = Perbill::from_percent(10);

    /// Share to all users.
    pub const PRE_BURN: Perbill = Perbill::from_percent(10);

    /// Pathfinder's fee
    pub const PRE_FEE: Perbill = Perbill::from_percent(10);

    // Used to solve the verifier's Dilemma and refresh seed.
    // pub const PRE_REWARD: Perbill = Perbill::from_percent(10);
}

/// The system is in the state of the algorithm.
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TIRStep {
    Free,
    Seed,
    Reputation,
}

impl Default for TIRStep {
    fn default() -> TIRStep {
        TIRStep::Free
    }
}

pub mod fee {
    use super::*;

    pub trait SweeperFee
    where
        Self: Sized,
    {
        /// Whether to allow `sweeper` participation when the last active time 
        /// is `last` and the current time is `now`.
        fn is_allowed_sweeper<B: AtLeast32BitUnsigned>(last: B, now: B) -> bool;

        /// Returns a checked `fee` and the remaining amount.
        fn checked_with_fee<B: AtLeast32BitUnsigned>(
            &self,
            last: B,
            now: B,
        ) -> Option<(Self, Self)>;

        ///  Returns the unchecked `fee` and the remaining amount.
        fn with_fee(&self) -> (Self, Self);
    }

    impl SweeperFee for Balance {
        fn is_allowed_sweeper<B: AtLeast32BitUnsigned>(last: B, now: B) -> bool {
            let now_into = TryInto::<u64>::try_into(now).ok().unwrap();
            let last_into = TryInto::<u64>::try_into(last).ok().unwrap();
            last_into + SWEEPER_PERIOD < now_into
        }

        fn checked_with_fee<B: AtLeast32BitUnsigned>(
            &self,
            last: B,
            now: B,
        ) -> Option<(Self, Self)> {
            match Balance::is_allowed_sweeper(last, now) {
                true => Some(self.with_fee()),
                false => None,
            }
        }

        fn with_fee(&self) -> (Self, Self) {
            let sweeper_fee = SWEEPER_PICKUP_RATIO.mul_floor(*self);
            (sweeper_fee, self.saturating_sub(sweeper_fee))
        }
    }
}

/// Returns the natural logarithm approximated to an integer, up to a maximum of 8.
pub fn appro_ln(value: u32) -> u32 {
    if value < 3 {
        1
    }else if value < 8 {
        2
    }else if value < 21 {
        3
    }else if value < 55 {
        4
    }else if value < 149 {
        5
    }else if value < 404 {
        6
    }else if value < 1097 {
        7
    }else {
        8
    }
}

/// The state of the challenge game.
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum ChallengeStatus {
    Free,
    Examine,
    Reply,
    Evidence,
    Arbitral,
}

impl Default for ChallengeStatus {
    fn default() -> Self {
        ChallengeStatus::Examine
    }
}

/// A pool of funds secured by staking and earnings.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct Pool {
    pub staking: Balance,
    pub earnings: Balance,
}

/// Information on the progress of the breakpoint transfer.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, Default, RuntimeDebug)]
pub struct Progress {
    pub total: u32,
    pub done: u32,
}

/// Challenge game metadata.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct Metadata<AccountId, BlockNumber> {
    /// Current pool of funds.
    pub pool: Pool,

    /// Whether `pathfinder` and the challenger are co-beneficiaries.
    pub joint_benefits: bool,

    /// Information on the progress of the breakpoint transfer.
    pub progress: Progress,

    /// Last updated.
    pub last_update: BlockNumber,

    /// Remarks data for storing useful information.
    pub remark: u32,

    /// Current score.
    pub score: u64,

    /// `AccountId` of `pathfinder`.
    pub pathfinder: AccountId,

    /// The current state of challenge.
    pub status: ChallengeStatus,

    /// The `AccountId` of challenger.
    pub challenger: AccountId,
}

impl<AccountId, BlockNumber> Metadata<AccountId, BlockNumber>
where
    AccountId: Ord + Clone,
    BlockNumber: Copy + AtLeast32Bit,
{
    /// The total amount of the pool of funds.
    pub fn total_amount(&self) -> Option<Balance> {
        self.pool.staking.checked_add(self.pool.earnings)
    }

    /// Are all uploads complete.
    pub fn is_all_done(&self) -> bool {
        self.progress.total == self.progress.done
    }

    /// Whether the upload is not finished.
    pub fn check_progress(&self) -> bool {
        self.progress.total >= self.progress.done
    }

    /// `who` is the challenger or not.
    pub fn is_challenger(&self, who: &AccountId) -> bool {
        self.challenger == *who
    }

    /// `who` is the pathfinder or not.
    pub fn is_pathfinder(&self, who: &AccountId) -> bool {
        self.pathfinder == *who
    }

    /// Reset progress.
    pub fn new_progress(&mut self, total: u32) -> &mut Self {
        self.progress.total = total;
        self.progress.done = Zero::zero();
        self
    }

    /// Advances progress by `count` bars.
    pub fn next(&mut self, count: u32) -> &mut Self {
        self.progress.done = self.progress.done.saturating_add(count);
        self
    }

    /// Set the challenge status to `status`.
    pub fn set_status(&mut self, status: &ChallengeStatus) {
        self.status = *status;
    }

    /// Start again.
    pub fn restart(&mut self, full_probative: bool) {
        self.status = ChallengeStatus::Free;
        self.joint_benefits = false;
        if full_probative {
            self.pathfinder = self.challenger.clone();
        }
    }
}

