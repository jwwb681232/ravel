use ravel_eloquent::Model;
use serde::{Deserialize, Serialize};

/// Model: {{name}}
#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
#[model(table = "{{snake}}s")]
pub struct {{name}} {
    #[model(id)]
    pub id: i32,
    // Add your columns here — example:
    // pub name: String,
    // #[model(string, 254, unique)]
    // pub email: String,
    // #[model(nullable, text)]
    // pub bio: Option<String>,
}
