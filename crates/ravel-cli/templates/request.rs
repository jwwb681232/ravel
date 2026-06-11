use ravel_http::form_request::FormRequest;
use ravel_http::validation::{FieldRule, Rule};
use serde::Deserialize;

/// Form request: {{name}}
#[derive(Debug, Deserialize)]
pub struct {{name}} {
    pub name: String,
    pub email: String,
    // Add your fields here
}

impl FormRequest for {{name}} {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("name", vec![Rule::Required, Rule::Min(3)]),
            FieldRule::new("email", vec![Rule::Required, Rule::Email]),
        ]
    }

    // Optional — uncomment to customize:
    //
    // fn authorize(&self) -> bool {
    //     self.role == "admin"
    // }
    //
    // fn messages() -> std::collections::HashMap<String, String> {
    //     let mut m = std::collections::HashMap::new();
    //     m.insert("name.required".into(), "Please enter your name".into());
    //     m
    // }
}
