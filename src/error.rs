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

impl<T> Display for CantMerge<T, WouldMove> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Can not perform merge in-place because {}", self.reason)
    }
}

impl<T: Debug> Error for CantMerge<T, WouldMove> {}
