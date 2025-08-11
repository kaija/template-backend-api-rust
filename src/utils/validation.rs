use validator::{ValidationError, ValidationErrors};

/// Validate email format
pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    if email.is_empty() {
        return Err(ValidationError::new("Email cannot be empty"));
    }

    if !email.contains('@') {
        return Err(ValidationError::new("Invalid email format"));
    }

    Ok(())
}

/// Validate name format
pub fn validate_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::new("Name cannot be empty"));
    }

    if name.len() > 255 {
        return Err(ValidationError::new("Name too long"));
    }

    Ok(())
}

/// Convert validation errors to a readable string
pub fn format_validation_errors(errors: &ValidationErrors) -> String {
    let mut messages = Vec::new();

    for (field, field_errors) in errors.field_errors() {
        for error in field_errors {
            let message = error.message
                .as_ref()
                .map(|m| m.to_string())
                .unwrap_or_else(|| format!("Invalid value for field '{}'", field));
            messages.push(message);
        }
    }

    messages.join(", ")
}
