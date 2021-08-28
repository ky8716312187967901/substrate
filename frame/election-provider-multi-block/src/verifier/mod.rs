// This file is part of Substrate.

// Copyright (C) 2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # The verifier pallet
//!
//! TODO

// Only these items are public from this pallet.
pub use pallet::{Config, Pallet};

mod pallet;

// internal imports
use crate::{helpers, SolutionOf};
use frame_election_provider_support::{PageIndex, Supports};
use pallet::{QueuedSolution, VerifyingSolution};
use sp_npos_elections::ElectionScore;
use std::fmt::Debug;

/// Errors that can happen in the feasibility check.
#[derive(Debug, Eq, PartialEq)]
pub enum FeasibilityError {
	/// Wrong number of winners presented.
	WrongWinnerCount,
	/// The snapshot is not available.
	///
	/// Kinda defensive: The pallet should technically never attempt to do a feasibility check
	/// when no snapshot is present.
	SnapshotUnavailable,
	/// Internal error from the election crate.
	NposElection(sp_npos_elections::Error),
	/// A vote is invalid.
	InvalidVote,
	/// A voter is invalid.
	InvalidVoter,
	/// A winner is invalid.
	InvalidWinner,
	/// The given score was invalid.
	InvalidScore,
	/// The provided round is incorrect.
	InvalidRound,
	/// Solution does not have a good enough score.
	ScoreTooLow,
}

impl From<sp_npos_elections::Error> for FeasibilityError {
	fn from(e: sp_npos_elections::Error) -> Self {
		FeasibilityError::NposElection(e)
	}
}

/// The interface of something that cna be the verifier.
pub trait Verifier {
	type Solution;
	type AccountId;

	/// This is a page of the solution that we want to verify next, store it.
	///
	/// This should be used to load solutions into this pallet.
	fn set_unverified_solution_page(
		remaining: PageIndex,
		page_solution: Option<Self::Solution>,
	) -> Result<(), ()>;

	/// Indicate that the previous calls to `set_unverified_solution_page` are now enough to form
	/// one full solution.
	///
	/// Fails previous calls to `set_unverified_solution_page` to form exactly `T::Pages` pages.
	/// Fails if
	fn seal_unverified_solution(claimed_score: ElectionScore) -> Result<(), ()>;

	/// The score of the current best solution. `None` if there is no best solution.
	fn queued_solution() -> Option<ElectionScore>;

	/// Check if the claimed score is sufficient.
	fn check_claimed_score(claimed_score: ElectionScore) -> bool;

	/// Get the current stage of the verification process.
	///
	/// Returns `Some(n)` if there's a ongoing verification; where `n` is the remaining number
	/// of blocks for the verification process. Returns `None` if there isn't a verification
	/// ongoing.
	fn status() -> Option<PageIndex>;

	/// Clear everything, there's nothing else for you to do until further notice.
	fn kill();

	/// Get the best verified solution, if any.
	///
	/// It is the responsibility of the call site to call this function with all appropriate
	/// `page` arguments.
	// TODO maybe rename to get_queued_solution_page
	fn get_valid_page(page: PageIndex) -> Option<Supports<Self::AccountId>>;

	/// Perform the feasibility check of the given solution page.
	///
	/// This will not check the score or winner-count, since they can only be checked in
	/// context.
	///
	/// Corresponding snapshots are assumed to be available.
	///
	/// A page that is `None` must always be valid.
	///
	/// IMPORTANT: this does not check any scores.
	fn feasibility_check_page(
		partial_solution: Option<Self::Solution>,
		page: PageIndex,
	) -> Result<Supports<Self::AccountId>, FeasibilityError>;

	/// Forcibly write this solution into the current valid variant.
	///
	/// This typically should only be called when you know that this solution is better than we
	/// we have currently queued. The provided score is assumed to be correct.
	///
	/// For now this is only needed for single page solution, thus the api will only support
	/// that.
	fn force_set_single_page_verified_solution(
		partial_solution: Supports<Self::AccountId>,
		verified_score: ElectionScore,
	);
}

impl<T: Config> Verifier for Pallet<T> {
	type Solution = SolutionOf<T>;
	type AccountId = T::AccountId;

	fn set_unverified_solution_page(
		page_index: PageIndex,
		page_solution: Option<Self::Solution>,
	) -> Result<(), ()> {
		VerifyingSolution::<T>::put_page(page_index, page_solution)
	}

	fn seal_unverified_solution(claimed_score: ElectionScore) -> Result<(), ()> {
		VerifyingSolution::<T>::seal_unverified_solution(claimed_score)
	}

	fn check_claimed_score(claimed_score: ElectionScore) -> bool {
		Self::ensure_correct_final_score_quality(claimed_score).is_ok()
	}

	fn queued_solution() -> Option<ElectionScore> {
		QueuedSolution::<T>::queued_solution()
	}

	fn status() -> Option<PageIndex> {
		todo!()
	}

	fn kill() {
		VerifyingSolution::<T>::kill();
		QueuedSolution::<T>::kill();
	}

	fn get_valid_page(page: PageIndex) -> Option<Supports<Self::AccountId>> {
		QueuedSolution::<T>::get_valid_page(page)
	}

	fn feasibility_check_page(
		maybe_partial_solution: Option<Self::Solution>,
		page: PageIndex,
	) -> Result<Supports<Self::AccountId>, FeasibilityError> {
		match maybe_partial_solution {
			Some(partial_solution) => Self::feasibility_check_page_inner(partial_solution, page),
			None => Ok(Default::default()),
		}
	}

	fn force_set_single_page_verified_solution(
		partial_supports: Supports<Self::AccountId>,
		verified_score: ElectionScore,
	) {
		QueuedSolution::<T>::force_set_single_page_valid(0, partial_supports, verified_score);
	}
}