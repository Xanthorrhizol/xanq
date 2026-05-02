use xancode::Codec;

#[async_trait::async_trait]
pub trait Producer<T: Codec + Send + 'static>: Send + Sync {
    async fn produce(&self, message: T) -> std::io::Result<()>;
}
