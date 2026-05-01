pub trait Address {
    fn to_string(&self) -> String;
    fn retention_time(&self) -> u64 {
        0 // unlimited
    }
    fn retention_size(&self) -> u64 {
        0 // unlimited
    }
}
