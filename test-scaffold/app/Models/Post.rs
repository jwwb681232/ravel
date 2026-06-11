use ravel_eloquent::Model;

/// Post model — each Post belongs to a User.
#[derive(Model, Clone, Debug)]
#[model(table = "posts", timestamps)]
pub struct Post {
    #[model(id)]
    pub id: i32,

    #[model(string, 255)]
    pub title: String,

    #[model(text)]
    pub content: String,

    #[model(integer)]
    #[model(belongs_to, from = "user_id", to = "id")]
    pub user_id: i32,

    /// The User who wrote this post
    pub user: BelongsTo<User>,
}
