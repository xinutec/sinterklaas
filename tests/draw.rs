//! The draw: everyone buys for exactly one other person.

use rand::SeedableRng;
use rand::rngs::StdRng;
use sinterklaas::draw::derangement;

#[test]
fn nobody_draws_themselves_and_everyone_is_drawn_once() {
    let mut rng = StdRng::seed_from_u64(5);
    for n in 2..=12 {
        for _ in 0..200 {
            let to = derangement(n, &mut rng).expect("a draw for two or more");
            assert_eq!(to.len(), n);
            assert!(
                to.iter().enumerate().all(|(giver, &r)| giver != r),
                "{to:?}"
            );
            let mut seen = to.clone();
            seen.sort_unstable();
            assert_eq!(
                seen,
                (0..n).collect::<Vec<_>>(),
                "someone drawn twice: {to:?}"
            );
        }
    }
}

#[test]
fn fewer_than_two_people_cannot_draw() {
    let mut rng = StdRng::seed_from_u64(1);
    assert_eq!(derangement(0, &mut rng), None);
    assert_eq!(derangement(1, &mut rng), None);
}

#[test]
fn every_derangement_of_three_comes_up() {
    // Three people have two derangements: the two rotations. Both must occur,
    // or the draw is not random among the allowed outcomes.
    let mut rng = StdRng::seed_from_u64(7);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..200 {
        seen.insert(derangement(3, &mut rng).expect("three can draw"));
    }
    assert_eq!(seen.len(), 2, "{seen:?}");
}
