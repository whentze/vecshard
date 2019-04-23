#[derive(Debug)]
pub struct CantMerge<T, E> {
    pub left: T,
    pub right: T,
    pub reason: E,
}

#[derive(Debug, PartialEq, Eq)]
pub enum WouldMove {
    DifferentAllocations,
    NotAdjacent,
    WrongOrder,
}
