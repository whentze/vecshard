use crate::VecShard;
use std::error::Error;
use std::fmt::{self, Debug, Display};

/// A generic merge error.
///
/// This exists because the merge fns take ownership of their input shards, and you may want your shards back upon error.
#[derive(Debug)]
pub struct CantMerge<T, E> {
    pub left: VecShard<T>,
    pub right: VecShard<T>,
    pub reason: E,
}

/// A reason why an in-place merge was unsuccesful.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum WouldMove {
    DifferentAllocations,
    NotAdjacent,
    WrongOrder,
}

/// A reason why a no-alloc merge was unsuccesful.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum WouldAlloc {
    DifferentAllocations,
    OtherShardsLeft,
}

impl Display for WouldMove {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use WouldMove::*;
        write!(
            f,
            "the two shards are {}",
            match self {
                DifferentAllocations => "not from the same memory allocation.",
                NotAdjacent => "not directly adjacent in memory.",
                WrongOrder => "adjacent, but were passed in the reverse order.",
            }
        )
    }
}

impl Display for WouldAlloc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use WouldAlloc::*;
        write!(
            f,
            "the two shards are {}",
            match self {
                DifferentAllocations => "not from the same memory allocation.",
                OtherShardsLeft => "not directly adjacent in memory and can't be moved around because there are still other shards in the Vec",
            }
        )
    }
}

impl<T, R: Display> Display for CantMerge<T, R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Can't perform quick merge because {}", self.reason)
    }
}

impl<T: Debug, R: Debug + Display> Error for CantMerge<T, R> {}
