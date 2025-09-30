use crate::error::AppError;
use std::path::Path;

/// Validates the configuration settings
///
/// # Arguments
/// * `api_domain` - The API domain to validate
/// * `log_file_path` - Optional log file path to validate
///
/// # Returns
/// * `Ok(())` - Configuration is valid
/// * `Err(AppError)` - Configuration validation failed
///
/// # Validation Rules
/// - API domain cannot be empty
/// - API domain must be a valid URL or domain name
/// - If log file path is provided, it cannot be empty
/// - Log file path parent directory must exist or be creatable
pub fn validate_config(api_domain: &str, log_file_path: &Option<String>) -> Result<(), AppError> {
    // Validate API domain
    if api_domain.is_empty() {
        return Err(AppError::config_error("API domain cannot be empty"));
    }

    // Check if API domain looks like a valid URL or domain
    if !api_domain.starts_with("http://") && !api_domain.starts_with("https://") {
        // If it doesn't start with protocol, it should at least look like a domain
        if !api_domain.contains('.') && !api_domain.starts_with("localhost") {
            return Err(AppError::config_error(
                "API domain must be a valid URL or domain name",
            ));
        }
    }

    // Validate log file path if provided
    if let Some(log_path) = log_file_path {
        if log_path.is_empty() {
            return Err(AppError::config_error("Log file path cannot be empty"));
        }

        // Check if parent directory exists or can be created
        if let Some(parent) = Path::new(log_path).parent()
            && !parent.exists()
        {
            // Try to create the directory to validate the path
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::config_error(format!(
                    "Cannot create log directory '{}': {}",
                    parent.display(),
                    e
                ))
            })?;
        }
    }

    Ok(())
}