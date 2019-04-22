#[derive(Debug)]
pub enum WouldMove {
    DifferentAllocations(usize, usize),
    NotAdjacent(usize, usize),
    WrongOrder,
}