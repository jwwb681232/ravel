//! Migration binary — use `ravel migrate` in your project root instead.
//! ```bash
//! ravel migrate           # run pending migrations
//! ravel migrate:rollback   # rollback last migration
//! ravel migrate:status     # show migration status
//! ```

#[tokio::main]
async fn main() {
    println!("💡 Use `ravel migrate` from your project root to manage migrations.");
    println!();
    println!("   ravel migrate           # up");
    println!("   ravel migrate:rollback   # down");
    println!("   ravel migrate:status     # show status");
    println!("   ravel migrate:fresh     # drop all + re-apply");
}
