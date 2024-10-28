use crate::Digest;

pub trait Engine {
    // TODO: refine this type better
    type Error;

    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, Self::Error>;

    // TODO: paginate this
    fn list_digests(&self) -> Result<Vec<Digest>, Self::Error>;

    /// This will execute a contract that implements the layer_avs:task-queue wit interface
    fn execute_queue(
        &self,
        _digest: Digest,
        _request: Vec<u8>,
        _timestamp: u64,
    ) -> Result<Vec<u8>, Self::Error>;
}
