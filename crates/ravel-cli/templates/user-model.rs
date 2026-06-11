use ravel_eloquent::Model;
use serde::{Serialize, Deserialize};

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[model(table = "users", timestamps)]
struct User {
    #[model(id)]
    id: i32,

    #[model(string, 255)]
    name: String,

    #[model(string, 254, unique)]
    email: String,

    #[model(hidden)]
    password: String,

    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}
