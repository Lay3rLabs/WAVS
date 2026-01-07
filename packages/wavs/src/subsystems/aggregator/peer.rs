#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Peer {
    Me,
    Other(String),
}
