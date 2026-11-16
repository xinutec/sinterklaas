//! The draw: everyone buys for someone, nobody for themselves.

use rand::Rng;
use rand::seq::SliceRandom;

/// A uniformly random derangement of `0..n`: `out[giver] = receiver`, never
/// equal. `None` below two people, where none exists.
///
/// Rejection sampling: a random permutation is a derangement with probability
/// about 1/e, so the expected number of shuffles is under three.
pub fn derangement<R: Rng + ?Sized>(n: usize, rng: &mut R) -> Option<Vec<usize>> {
    if n < 2 {
        return None;
    }
    let mut perm: Vec<usize> = (0..n).collect();
    loop {
        perm.shuffle(rng);
        if perm
            .iter()
            .enumerate()
            .all(|(giver, &receiver)| giver != receiver)
        {
            return Some(perm);
        }
    }
}
