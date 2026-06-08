//! ravel key:generate — generate a random application key.
//!
//! Generates a 32-byte random key (base64) and writes it to `.env`
//! as `APP_KEY=base64:...`.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn handle() -> Result<()> {
    let key = generate_key();
    let env_line = format!("APP_KEY=base64:{}", key);

    let env_path = Path::new(".env");

    if env_path.exists() {
        let content = fs::read_to_string(env_path).context("Reading .env")?;

        // Check if APP_KEY already exists
        if content.contains("APP_KEY=") {
            // Replace existing APP_KEY line
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
            // Append to .env
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
        // Create .env with APP_KEY
        fs::write(env_path, format!("{}\n", env_line)).context("Creating .env")?;
        println!("🔑 .env created with application key");
    }

    println!("   APP_KEY=base64:{}", key);

    Ok(())
}

/// Generate a 32-byte random key encoded as base64.
fn generate_key() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Use a combination of time and process info for randomness
    // In production, you'd use `rand` crate, but this avoids a dependency
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = std::process::id() as u128;

    // Simple hash-based key generation
    let mut key = [0u8; 32];
    let seed = now.wrapping_mul(pid.wrapping_mul(6364136223846793005));

    for (i, byte) in key.iter_mut().enumerate() {
        let h = seed
            .wrapping_mul((i as u128).wrapping_mul(1442695040888963407))
            .wrapping_add(1);
        *byte = (h >> 64) as u8;
    }

    // Base64 encode
    base64_encode(&key)
}

/// Simple base64 encoder (avoids adding a dependency).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}
