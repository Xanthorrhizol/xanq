use crate::message::Message;
use crate::queue::Queue;
use tokio::sync::Mutex;

#[async_trait::async_trait]
pub trait Consumer<T: Message> {
    fn queue(&self) -> &Mutex<Queue>;
    async fn consume(&self) -> Option<T> {
        let message = self.queue().lock().await.pop_front();
        message.map(|m| Message::decode(&m).ok()).flatten()
    }
    async fn peek(&self, id: usize) -> Option<T> {
        self.queue()
            .lock()
            .await
            .peek(id)
            .map(|m| Message::decode(&m).ok())
            .flatten()
    }
}
