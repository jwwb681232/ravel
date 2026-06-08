use anyhow::Result;
use ravel_generator::Generator;

pub fn handle(g: &Generator, name: &str) -> Result<()> {
    let file_path = format!("app/Models/{}.rs", name);

    let content = format!(
        r#"pub struct {name};

impl {name} {{
    pub fn new() -> Self {{
        Self
    }}
}}
"#
    );

    g.create_file(&file_path, &content)?;

    Ok(())
}
