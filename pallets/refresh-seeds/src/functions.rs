// Copyright 2021 ZeroDAO
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::*;
use sha1::{Digest, Sha1};
use sp_std::vec;

impl<T: Config> Pallet<T> {
    /// Return to current block
    pub fn now() -> T::BlockNumber {
        system::Module::<T>::block_number()
    }

    /// For data of type `&[u8]` `sha1`.
    // Collisions have no impact on safety.
    // sha1 is safe enough.
    pub fn sha1_hasher(data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha1::new();
        hasher.update(data);
        hasher
            .finalize()
            .iter()
            .flat_map(|n| {
                vec![n / 16u8, n % 16u8]
                    .iter()
                    .map(|u| if u < &10u8 { u + 48u8 } else { u - 10u8 + 97u8 })
                    .collect::<Vec<u8>>()
            })
            .collect::<Vec<u8>>()
    }

    /// Generate `full_order` with `start` as the start user and `stop` as the end user at depth `deep.
    pub fn make_full_order(
        start: &T::AccountId,
        stop: &T::AccountId,
        deep: usize,
    ) -> Vec<u8> {
        let mut points = T::AccountId::encode(start);
        points.extend(T::AccountId::encode(stop).iter().cloned());
        let points_hash = Self::sha1_hasher(&points);
        let index = points_hash.len() - (deep * RANGE);
        points_hash[index..].to_vec()
    }

    /// Insert a new hash result.
    pub fn insert_hash(target: &T::AccountId, hash_set: OrderedSet<ResultHash>) -> DispatchResult {
        <ResultHashsSets<T>>::try_mutate(target, |c| {
            ensure!(
                (c.len() as u8) < DEEP,
                Error::<T>::MaximumDepth
            );
            c.push(hash_set);
            Ok(())
        })
    }

    /// Insert `score` into `score_list` and save it under `ScoreList`.
    pub fn score_list_insert(score_list: &mut Vec<u64>, score: &u64) {
        let index = score_list
            .binary_search(score)
            .unwrap_or_else(|index| index);
        score_list.insert(index, *score);
        <ScoreList<T>>::put(score_list);
    }

    pub(crate) fn try_get_rhash(
        target: &T::AccountId,
    ) -> Result<Vec<OrderedSet<ResultHash>>, DispatchError> {
        <ResultHashsSets<T>>::try_get(target).map_err(|_| Error::<T>::NonExistent.into())
    }

    pub(crate) fn is_all_timeout() -> bool {
        let last = T::Reputation::get_last_refresh_at();
        last + T::ConfirmationPeriod::get() < Self::now()
    }

    pub(crate) fn is_all_harvest() -> bool {
        <Candidates<T>>::iter_values().next().is_none()
    }

    pub(crate) fn check_step() -> DispatchResult {
        ensure!(
            T::Reputation::is_step(&TIRStep::Seed),
            Error::<T>::StepNotMatch
        );
        Ok(())
    }

    pub(crate) fn hand_first_time(score_list: &mut Vec<u64>) {
        let max_seed_count = T::MaxSeedCount::get() as usize;
        let len = score_list.len();
        if len > max_seed_count {
            *score_list = score_list[(len - max_seed_count)..].to_vec();
        }
        T::SeedsBase::remove_all();
        Self::deposit_event(Event::SeedsSelected(score_list.len() as u32));
    }

    // pub(crate) fn check_hash(data: &[u8], hash: &[u8; 8]) -> bool {
    //     Self::sha1_hasher(data)[..8] == hash[..]
    // }

    pub(crate) fn get_pathfinder_paths(
        target: &T::AccountId,
        index: &u32,
    ) -> Result<Path<T::AccountId>, DispatchError> {
        let paths = <Paths<T>>::try_get(&target).map_err(|_| Error::<T>::PathDoesNotExist)?;
        let index = *index as usize;
        ensure!(paths.len() > index, Error::<T>::IndexExceedsMaximum);
        Ok(paths[index].clone())
    }

    pub(crate) fn do_harvest_challenge(
        who: &T::AccountId,
        target: &T::AccountId,
    ) -> DispatchResult {
        <Candidates<T>>::try_mutate(target, |c| {
            if let Some(score) = T::ChallengeBase::harvest(who, &APP_ID, target)? {
                c.score = score;
            }
            Self::remove_challenge(target);
            Ok(())
        })
    }

    pub(crate) fn get_ends(path: &Path<T::AccountId>) -> (&T::AccountId, &T::AccountId) {
        Self::get_nodes_ends(&path.nodes[..])
    }

    pub(crate) fn get_nodes_ends(nodes: &[T::AccountId]) -> (&T::AccountId, &T::AccountId) {
        let stop = nodes.last().unwrap();
        (&nodes[0], stop)
    }

    pub(crate) fn candidate_insert(targer: &T::AccountId, pathfinder: &T::AccountId, score: &u64) {
        <Candidates<T>>::insert(
            targer,
            Candidate {
                score: *score,
                pathfinder: pathfinder.clone(),
                has_challenge: false,
                add_at: Self::now(),
            },
        );
        let mut score_list = Self::get_score_list();
        Self::score_list_insert(&mut score_list, score);
    }

    pub(crate) fn mutate_score(old_score: &u64, new_score: &u64) {
        let mut score_list = Self::get_score_list();
        if let Ok(index) = score_list.binary_search(old_score) {
            score_list.remove(index);
        }
        Self::score_list_insert(&mut score_list, new_score);
    }

    pub(crate) fn check_mid_path(
        mid_path: &[T::AccountId],
        start: &T::AccountId,
        stop: &T::AccountId,
    ) -> Result<Vec<T::AccountId>, DispatchError> {
        let mut nodes = mid_path.to_vec();
        nodes.insert(0, start.clone());
        nodes.push(stop.clone());
        T::TrustBase::valid_nodes(&nodes[..])?;
        Ok(nodes.to_vec())
    }

    pub(crate) fn restart(target: &T::AccountId, pathfinder: &T::AccountId, score: &u64) {
        <Candidates<T>>::mutate(&target, |c| {
            Self::mutate_score(&c.score, score);
            c.score = *score;
            c.pathfinder = pathfinder.clone();
        });
        Self::remove_challenge(target);
        Self::deposit_event(Event::ChallengeRestarted(target.clone(), *score));
    }

    pub(crate) fn remove_challenge(target: &T::AccountId) {
        <Paths<T>>::remove(&target);
        <ResultHashsSets<T>>::remove(&target);
        <MissedPaths<T>>::remove(&target);
    }

    pub(crate) fn checked_nodes(nodes: &[T::AccountId], target: &T::AccountId) -> DispatchResult {
        ensure!(nodes.len() >= 2, Error::<T>::PathTooShort);
        ensure!(nodes.contains(target), Error::<T>::NoTargetNode);
        T::TrustBase::valid_nodes(nodes)?;
        Ok(())
    }

    pub(crate) fn checked_paths_vec(
        paths: &[Path<T::AccountId>],
        target: &T::AccountId,
        order: &[u8],
        deep: usize,
    ) -> DispatchResult {
        for p in paths {
            ensure!(
                p.total > 0 && p.total < MAX_SHORTEST_PATH,
                Error::<T>::PathTooLong
            );
            let (start, stop) = Self::get_ends(p);
            ensure!(
                Self::make_full_order(start, stop, deep) == *order,
                Error::<T>::OrderNotMatch
            );
            Self::checked_nodes(&p.nodes[..], target)?;
        }
        Ok(())
    }

    pub(crate) fn get_next_order(
        target: &T::AccountId,
        old_order: &u64,
        index: &usize,
    ) -> Result<u64, Error<T>> {
        match Self::try_get_rhash(target) {
            Ok(r_hashs_sets) => {
                let mut full_order = Self::get_full_order(&r_hashs_sets[..], old_order, index)?;
                full_order.try_to_u64().ok_or(Error::<T>::ConverError)
            }
            Err(_) => Ok(0u64),
        }
    }

    // index
    pub(crate) fn get_full_order(
        result_hashs_sets: &[OrderedSet<ResultHash>],
        old_order: &u64,
        index: &usize,
    ) -> Result<FullOrder, Error<T>> {
        match result_hashs_sets.is_empty() {
            false => {
                let next_level_order = result_hashs_sets.last().unwrap().0[*index].order;
                let mut full_order = FullOrder::from_u64(old_order, result_hashs_sets.len());
                full_order.connect(&next_level_order);
                Ok(full_order)
            }
            true => Ok(FullOrder::default()),
        }
    }

    pub(crate) fn update_result_hashs(
        target: &T::AccountId,
        hashs: &[PostResultHash],
        do_verify: bool,
        index: u32,
        next: bool,
    ) -> DispatchResult {
        let new_r_hashs = hashs
            .iter()
            .map(|h| h.to_result_hash())
            .collect::<Vec<ResultHash>>();
        let mut r_hashs_sets = <ResultHashsSets<T>>::get(target);
        let current_deep = r_hashs_sets.len();

        match next {
            true => {
                ensure!((current_deep as u8) <= DEEP, Error::<T>::MaximumDepth);
                ensure!(!r_hashs_sets.is_empty(), Error::<T>::DataEmpty);
                let mut r_hashs_vec = r_hashs_sets[current_deep - 1].0.clone();
                r_hashs_vec.extend_from_slice(&new_r_hashs[..]);
                let full_hashs_set = OrderedSet::from(r_hashs_vec.clone());
                ensure!(
                    r_hashs_vec.len() == full_hashs_set.len(),
                    Error::<T>::DataDuplication
                );
                r_hashs_sets[current_deep - 1] = full_hashs_set;
            }
            false => {
                ensure!((current_deep as u8) < DEEP, Error::<T>::MaximumDepth);
                let r_hashs_set = OrderedSet::from(new_r_hashs.clone());
                ensure!(
                    new_r_hashs.len() == r_hashs_set.len(),
                    Error::<T>::DataDuplication
                );
                r_hashs_sets.push(r_hashs_set);
            }
        }

        if do_verify {
            Self::verify_result_hashs(&r_hashs_sets[..], index, target)?;
        }

        <ResultHashsSets<T>>::mutate(target, |rs| *rs = r_hashs_sets);
        Ok(())
    }

    pub(crate) fn verify_paths(
        paths: &[Path<T::AccountId>],
        target: &T::AccountId,
        result_hash: &ResultHash,
    ) -> DispatchResult {
        let total_score =
            paths
                .iter()
                .try_fold::<_, _, Result<u32, DispatchError>>(0u32, |acc, p| {
                    Self::checked_nodes(&p.nodes[..], target)?;
                    ensure!(p.total < 100, Error::<T>::LengthTooLong);
                    // Two-digit accuracy
                    let score = 100 / p.total;
                    Ok(acc.saturating_add(score))
                })?;

        ensure!(
            total_score as u64 == result_hash.score,
            Error::<T>::ScoreMismatch
        );
        Ok(())
    }

    pub(crate) fn verify_result_hashs(
        result_hashs: &[OrderedSet<ResultHash>],
        index: u32,
        target: &T::AccountId,
    ) -> DispatchResult {
        let deep = result_hashs.len();

        if deep == 0 {
            return Ok(());
        }
        // let mut data: Vec<u8> = Vec::default();

        let fold_score = result_hashs[deep - 1]
            .0
            .iter()
            .try_fold::<_, _, Result<u64, Error<T>>>(0u64, |acc, r| {
                // if deep > 1 {
                //     data.extend_from_slice(&r.hash);
                // }
                ensure!(r.order.len() == RANGE, Error::<T>::OrderNotMatch);
                acc.checked_add(r.score).ok_or(Error::<T>::Overflow)
            })?;
        let total_score = match deep {
            1 => Self::get_candidate(&target).score,
            _ => {
                // ensure!(
                //     Self::check_hash(
                //         data.as_slice(),
                //         &result_hashs[deep - 2].0[index as usize].hash
                //     ),
                //     Error::<T>::HashMismatch
                // );
                result_hashs[deep - 2].0[index as usize].score
            }
        };
        ensure!(fold_score == total_score, Error::<T>::ScoreMismatch);
        Ok(())
    }

    pub(crate) fn do_reply_num(
        challenger: &T::AccountId,
        target: &T::AccountId,
        mid_paths: &[Vec<T::AccountId>],
    ) -> DispatchResult {
        let count = mid_paths.len();
        let _ = T::ChallengeBase::reply(
            &APP_ID,
            challenger,
            target,
            Zero::zero(),
            Zero::zero(),
            |_, index, _| -> Result<u64, DispatchError> {
                let p_path = Self::get_pathfinder_paths(target, &index)?;
                ensure!((count as u32) == p_path.total, Error::<T>::LengthNotEqual);
                let (start, stop) = Self::get_ends(&p_path);
                for mid_path in mid_paths {
                    let _ = Self::check_mid_path(&mid_path[..], start, stop)?;
                }
                Ok(Zero::zero())
            },
        )?;
        Ok(())
    }

    pub(crate) fn evidence_of_missed(
        challenger: &T::AccountId,
        target: &T::AccountId,
        nodes: &[T::AccountId],
        index: u32,
    ) -> DispatchResult {
        Self::check_step()?;
        Self::checked_nodes(nodes, target)?;

        let (start, stop) = Self::get_nodes_ends(nodes);

        let deep =
            <ResultHashsSets<T>>::decode_len(target).ok_or(Error::<T>::ResultHashNotExit)?;

        let user_full_order = Self::make_full_order(start, stop, deep);
        let maybe_score = T::ChallengeBase::evidence(
            &APP_ID,
            challenger,
            target,
            |_, order| -> Result<bool, DispatchError> {
                let index = index as usize;
                match <Paths<T>>::try_get(target) {
                    Ok(path_vec) => {
                        ensure!(
                            FullOrder::from_u64(&order, deep + 1).0 == user_full_order,
                            Error::<T>::NotMatch
                        );
                        let mut same_ends = false;
                        for p in path_vec {
                            if *start == p.nodes[0] && stop == p.nodes.last().unwrap() {
                                ensure!(
                                    p.nodes.len() == nodes.len(),
                                    Error::<T>::LengthNotEqual
                                );
                                ensure!(p.nodes[..] != *nodes, Error::<T>::AlreadyExist);
                                same_ends = true;
                            }
                        }
                        Ok(!same_ends)
                    }
                    Err(_) => {
                        let result_hash_sets = Self::try_get_rhash(target)?;
                        let last_r_hash = &result_hash_sets.last().unwrap().0;
                        ensure!(index <= last_r_hash.len(), Error::<T>::IndexExceedsMaximum);
                        if deep > 1 {
                            ensure!(
                                FullOrder::from_u64(&order, deep).0
                                    == user_full_order[..RANGE * (deep - 1)],
                                Error::<T>::NotMatch
                            );
                        }
                        if index > 0 {
                            ensure!(
                                last_r_hash[index - 1].order[..] < user_full_order[..RANGE],
                                Error::<T>::PathIndexError
                            );
                        }
                        if index < last_r_hash.len() {
                            ensure!(
                                last_r_hash[index].order[..] > user_full_order[..RANGE],
                                Error::<T>::PathIndexError
                            );
                        }
                        // arbitration : Unable to determine the shortest path
                        Ok(true)
                    }
                }
            },
        )?;

        match maybe_score {
            Some(score) => Self::restart(target, challenger, &score),
            None => <MissedPaths<T>>::insert(target, nodes.to_vec()),
        }

        Self::deposit_event(Event::MissedPathPresented(challenger.clone(), target.clone(), index));
        Ok(())
    }
}
