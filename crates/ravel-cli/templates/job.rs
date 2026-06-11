use ravel_macros::Job;
use serde::{Serialize, Deserialize};

/// Job: {{name}}
#[derive(Serialize, Deserialize, Job)]
#[job(name = "{{snake}}")]
pub struct {{name}} {
    // Add your payload fields here — example:
    // pub user_id: i32,
    // pub email: String,
}

impl {{name}} {
    /// Core job logic — called by the generated `Job::handle()`.
    pub async fn execute(&self) -> anyhow::Result<()> {
        tracing::info!("{{name}}: executing");
        // TODO: implement your job logic here

        Ok(())
    }
}
