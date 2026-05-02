use xancode::Codec;

#[async_trait::async_trait]
pub trait Consumer<T: Codec + Send>: Send + Sync {
    async fn consume(&self) -> std::io::Result<Option<T>>;
}
