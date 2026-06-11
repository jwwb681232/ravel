use ravel_eloquent::Model;
use serde::{Serialize, Deserialize};

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[model(table = "posts", timestamps)]
struct Post {
    #[model(id)]
    id: i32,

    #[model(string, 255)]
    title: String,

    #[model(text)]
    content: String,

    #[model(integer)]
    user_id: i32,

    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}
