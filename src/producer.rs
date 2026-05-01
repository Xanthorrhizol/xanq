use crate::message::Message;
use crate::queue::Queue;
use tokio::sync::Mutex;

#[async_trait::async_trait]
pub trait Producer<T: Message + Send + 'static> {
    fn queue(&self) -> &Mutex<Queue>;
    async fn produce(&self, message: T) {
        self.queue().lock().await.push_back(message.encode());
    }
}
