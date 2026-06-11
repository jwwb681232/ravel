use ravel_eloquent::Model;

/// User model — one User has many Posts.
#[derive(Model, Clone, Debug)]
#[model(table = "users", timestamps)]
pub struct User {
    #[model(id)]
    pub id: i32,

    #[model(string, 255)]
    pub name: String,

    #[model(string, 254, unique)]
    pub email: String,

    #[model(hidden)]
    pub password: String,

    /// User has many Posts (foreign key: posts.user_id)
    pub posts: HasMany<Post>,
}
