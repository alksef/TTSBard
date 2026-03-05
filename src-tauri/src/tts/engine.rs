use async_trait::async_trait;

#[async_trait]
pub trait TtsEngine: Send + Sync {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String>;
    #[allow(dead_code)]
    fn is_configured(&self) -> bool;
    #[allow(dead_code)]
    fn name(&self) -> &str;
}
