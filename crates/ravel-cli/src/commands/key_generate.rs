//! ravel key:generate — generate a random application key.
//!
//! Generates a 32-byte cryptographically random key and writes it to `.env`
//! as `APP_KEY=base64:...`.

use anyhow::{Context, Result};
use rand::RngCore;
use std::fs;
use std::path::Path;

pub fn handle() -> Result<()> {
    let key = generate_key();
    let env_line = format!("APP_KEY=base64:{}", key);

    let env_path = Path::new(".env");

    if env_path.exists() {
        let content = fs::read_to_string(env_path).context("Reading .env")?;

        if content.contains("APP_KEY=") {
            let new_content: String = content
                .lines()
                .map(|line| {
                    if line.starts_with("APP_KEY=") {
                        env_line.clone()
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(env_path, new_content).context("Writing .env")?;
            println!("🔑 Application key updated in .env");
        } else {
            let mut new_content = content;
            if !new_content.ends_with('\n') {
                new_content.push('\n');
            }
            new_content.push_str(&env_line);
            new_content.push('\n');
            fs::write(env_path, new_content).context("Writing .env")?;
            println!("🔑 Application key added to .env");
        }
    } else {
        fs::write(env_path, format!("{}\n", env_line)).context("Creating .env")?;
        println!("🔑 .env created with application key");
    }

    println!("   APP_KEY=base64:{}", key);

    Ok(())
}

/// Generate a 32-byte cryptographically random key, base64-encoded.
fn generate_key() -> String {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &key)
}
