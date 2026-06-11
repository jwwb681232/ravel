use ravel_macros::Job;
use serde::{Serialize, Deserialize};

/// Background job: send a welcome email after a post is created.
#[derive(Serialize, Deserialize, Job)]
#[job(name = "send_welcome_email")]
pub struct SendWelcomeEmail {
    pub user_id: i32,
}

impl SendWelcomeEmail {
    pub async fn execute(&self) -> anyhow::Result<()> {
        tracing::info!("Welcome email sent to user {}", self.user_id);
        Ok(())
    }
}
