//! Metadata templating system for shared content across locales.
//!
//! This module provides a templating system that allows sharing common content
//! across multiple locales with variable substitution and inheritance.
//!
//! ## Example
//!
//! ```rust
//! use canaveral_metadata::templates::{TemplateVariables, process_template};
//!
//! let mut vars = TemplateVariables::default();
//! vars.app_name = Some("MyApp".to_string());
//! vars.company_name = Some("Acme Inc.".to_string());
//!
//! let template = "Welcome to {{app_name}} by {{company_name}}!";
//! let result = process_template(template, &vars);
//! assert_eq!(result, "Welcome to MyApp by Acme Inc.!");
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use tracing::debug;

use crate::types::apple::{AppleLocalizedMetadata, AppleMetadata};
use crate::types::google_play::{GooglePlayLocalizedMetadata, GooglePlayMetadata};
use crate::{MetadataError, Result};

/// Template variables that can be substituted in metadata strings.
///
/// This struct contains common variables that are often shared across locales,
/// such as app name, company name, and support email. Custom variables can be
/// added via the `custom` field.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateVariables {
    /// App name (can be overridden per locale).
    pub app_name: Option<String>,
    /// Company/developer name.
    pub company_name: Option<String>,
    /// Support email address.
    pub support_email: Option<String>,
    /// Current version string.
    pub version: Option<String>,
    /// Custom variables for additional substitutions.
    #[serde(flatten)]
    pub custom: HashMap<String, String>,
}

impl TemplateVariables {
    /// Creates a new empty `TemplateVariables` instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `TemplateVariables` with the given app name.
    pub fn with_app_name(app_name: impl Into<String>) -> Self {
        Self {
            app_name: Some(app_name.into()),
            ..Default::default()
        }
    }

    /// Sets the app name and returns self for chaining.
    pub fn app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = Some(name.into());
        self
    }

    /// Sets the company name and returns self for chaining.
    pub fn company_name(mut self, name: impl Into<String>) -> Self {
        self.company_name = Some(name.into());
        self
    }

    /// Sets the support email and returns self for chaining.
    pub fn support_email(mut self, email: impl Into<String>) -> Self {
        self.support_email = Some(email.into());
        self
    }

    /// Sets the version and returns self for chaining.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Adds a custom variable and returns self for chaining.
    pub fn custom(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom.insert(key.into(), value.into());
        self
    }

    /// Adds a custom variable.
    pub fn set_custom(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.custom.insert(key.into(), value.into());
    }

    /// Gets the value for a variable by name.
    pub fn get(&self, name: &str) -> Option<&str> {
        match name {
            "app_name" => self.app_name.as_deref(),
            "company_name" => self.company_name.as_deref(),
            "support_email" => self.support_email.as_deref(),
            "version" => self.version.as_deref(),
            other => self.custom.get(other).map(|s| s.as_str()),
        }
    }

    /// Checks if a variable exists (has a value).
    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
}

/// Process a template string, replacing `{{variable}}` patterns with values.
///
/// Variables are specified in the format `{{variable_name}}`. Built-in variables
/// include `app_name`, `company_name`, `support_email`, and `version`. Custom
/// variables can be added to `TemplateVariables::custom`.
///
/// Variables that are not defined are left unchanged in the output.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::templates::{TemplateVariables, process_template};
///
/// let vars = TemplateVariables::default()
///     .app_name("MyApp")
///     .version("1.0.0");
///
/// let result = process_template("{{app_name}} v{{version}}", &vars);
/// assert_eq!(result, "MyApp v1.0.0");
/// ```
pub fn process_template(template: &str, vars: &TemplateVariables) -> String {
    let mut result = template.to_string();

    // Built-in variables
    if let Some(ref name) = vars.app_name {
        result = result.replace("{{app_name}}", name);
    }
    if let Some(ref company) = vars.company_name {
        result = result.replace("{{company_name}}", company);
    }
    if let Some(ref email) = vars.support_email {
        result = result.replace("{{support_email}}", email);
    }
    if let Some(ref version) = vars.version {
        result = result.replace("{{version}}", version);
    }

    // Custom variables
    for (key, value) in &vars.custom {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }

    result
}

/// Check if a string contains template variables.
///
/// Returns `true` if the text contains any `{{...}}` patterns.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::templates::has_template_variables;
///
/// assert!(has_template_variables("Hello {{name}}!"));
/// assert!(!has_template_variables("Hello World!"));
/// ```
pub fn has_template_variables(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if let Some(&'{') = chars.peek() {
                chars.next();
                // Look for closing }}
                while let Some(inner) = chars.next() {
                    if inner == '}' {
                        if let Some(&'}') = chars.peek() {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Extract variable names from a template string.
///
/// Returns a list of all unique variable names found in `{{...}}` patterns.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::templates::extract_variable_names;
///
/// let names = extract_variable_names("{{app_name}} by {{company_name}}");
/// assert!(names.contains(&"app_name".to_string()));
/// assert!(names.contains(&"company_name".to_string()));
/// ```
pub fn extract_variable_names(template: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut remaining = template;

    while let Some(start) = remaining.find("{{") {
        remaining = &remaining[start + 2..];
        if let Some(end) = remaining.find("}}") {
            let name = remaining[..end].trim().to_string();
            if !name.is_empty() && !names.contains(&name) {
                names.push(name);
            }
            remaining = &remaining[end + 2..];
        } else {
            break;
        }
    }

    names
}

/// Validate that all variables in a template have values.
///
/// Returns an error if any variable in the template is not defined in `vars`.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::templates::{TemplateVariables, validate_template};
///
/// let vars = TemplateVariables::default().app_name("MyApp");
///
/// // This succeeds because app_name is defined
/// assert!(validate_template("Hello {{app_name}}!", &vars).is_ok());
///
/// // This fails because unknown_var is not defined
/// assert!(validate_template("Hello {{unknown_var}}!", &vars).is_err());
/// ```
pub fn validate_template(template: &str, vars: &TemplateVariables) -> Result<()> {
    let names = extract_variable_names(template);
    let missing: Vec<_> = names.iter().filter(|name| !vars.has(name)).collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(MetadataError::ValidationFailed(format!(
            "Missing template variables: {}",
            missing
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )))
    }
}

/// Apply template processing to all string fields in Apple metadata.
///
/// This function processes all localizations and replaces template variables
/// in text fields like name, description, subtitle, etc.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::{AppleMetadata, AppleLocalizedMetadata};
/// use canaveral_metadata::templates::{TemplateVariables, apply_templates_to_apple_metadata};
///
/// let mut metadata = AppleMetadata::new("com.example.app");
/// metadata.set_localization("en-US", AppleLocalizedMetadata::new(
///     "{{app_name}}",
///     "Welcome to {{app_name}} by {{company_name}}!"
/// ));
///
/// let vars = TemplateVariables::default()
///     .app_name("MyApp")
///     .company_name("Acme Inc.");
///
/// apply_templates_to_apple_metadata(&mut metadata, &vars);
///
/// let en = metadata.get_localization("en-US").unwrap();
/// assert_eq!(en.name, "MyApp");
/// assert_eq!(en.description, "Welcome to MyApp by Acme Inc.!");
/// ```
pub fn apply_templates_to_apple_metadata(metadata: &mut AppleMetadata, vars: &TemplateVariables) {
    debug!(locale_count = metadata.localizations.len(), "applying templates to Apple metadata");
    // Process each localization
    for localized in metadata.localizations.values_mut() {
        apply_templates_to_apple_localized(localized, vars);
    }

    // Process URLs
    if let Some(ref url) = metadata.privacy_policy_url {
        metadata.privacy_policy_url = Some(process_template(url, vars));
    }
    if let Some(ref url) = metadata.support_url {
        metadata.support_url = Some(process_template(url, vars));
    }
    if let Some(ref url) = metadata.marketing_url {
        metadata.marketing_url = Some(process_template(url, vars));
    }
    if let Some(ref copyright) = metadata.copyright {
        metadata.copyright = Some(process_template(copyright, vars));
    }
}

/// Apply template processing to a single Apple localized metadata.
fn apply_templates_to_apple_localized(
    localized: &mut AppleLocalizedMetadata,
    vars: &TemplateVariables,
) {
    localized.name = process_template(&localized.name, vars);
    localized.description = process_template(&localized.description, vars);

    if let Some(ref subtitle) = localized.subtitle {
        localized.subtitle = Some(process_template(subtitle, vars));
    }
    if let Some(ref keywords) = localized.keywords {
        localized.keywords = Some(process_template(keywords, vars));
    }
    if let Some(ref whats_new) = localized.whats_new {
        localized.whats_new = Some(process_template(whats_new, vars));
    }
    if let Some(ref promo) = localized.promotional_text {
        localized.promotional_text = Some(process_template(promo, vars));
    }
    if let Some(ref url) = localized.privacy_policy_url {
        localized.privacy_policy_url = Some(process_template(url, vars));
    }
    if let Some(ref url) = localized.support_url {
        localized.support_url = Some(process_template(url, vars));
    }
    if let Some(ref url) = localized.marketing_url {
        localized.marketing_url = Some(process_template(url, vars));
    }
}

/// Apply template processing to all string fields in Google Play metadata.
///
/// This function processes all localizations and replaces template variables
/// in text fields like title, descriptions, changelogs, etc.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::{GooglePlayMetadata, GooglePlayLocalizedMetadata};
/// use canaveral_metadata::templates::{TemplateVariables, apply_templates_to_google_play_metadata};
///
/// let mut metadata = GooglePlayMetadata::new("com.example.app");
/// metadata.set_localization("en-US", GooglePlayLocalizedMetadata::new(
///     "{{app_name}}",
///     "{{app_name}} - Best {{category}} app!",
///     "Welcome to {{app_name}} by {{company_name}}!"
/// ));
///
/// let vars = TemplateVariables::default()
///     .app_name("MyApp")
///     .company_name("Acme Inc.")
///     .custom("category", "productivity");
///
/// apply_templates_to_google_play_metadata(&mut metadata, &vars);
///
/// let en = metadata.get_localization("en-US").unwrap();
/// assert_eq!(en.title, "MyApp");
/// assert_eq!(en.short_description, "MyApp - Best productivity app!");
/// ```
pub fn apply_templates_to_google_play_metadata(
    metadata: &mut GooglePlayMetadata,
    vars: &TemplateVariables,
) {
    debug!(locale_count = metadata.localizations.len(), "applying templates to Google Play metadata");
    // Process each localization
    for localized in metadata.localizations.values_mut() {
        apply_templates_to_google_play_localized(localized, vars);
    }

    // Process URLs and contact info
    if let Some(ref url) = metadata.privacy_policy_url {
        metadata.privacy_policy_url = Some(process_template(url, vars));
    }
    if let Some(ref email) = metadata.contact_email {
        metadata.contact_email = Some(process_template(email, vars));
    }
    if let Some(ref website) = metadata.contact_website {
        metadata.contact_website = Some(process_template(website, vars));
    }
}

/// Apply template processing to a single Google Play localized metadata.
fn apply_templates_to_google_play_localized(
    localized: &mut GooglePlayLocalizedMetadata,
    vars: &TemplateVariables,
) {
    localized.title = process_template(&localized.title, vars);
    localized.short_description = process_template(&localized.short_description, vars);
    localized.full_description = process_template(&localized.full_description, vars);

    if let Some(ref video_url) = localized.video_url {
        localized.video_url = Some(process_template(video_url, vars));
    }

    // Process changelogs
    let processed_changelogs: HashMap<String, String> = localized
        .changelogs
        .iter()
        .map(|(version, changelog)| (version.clone(), process_template(changelog, vars)))
        .collect();
    localized.changelogs = processed_changelogs;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_template_basic() {
        let vars = TemplateVariables::default()
            .app_name("TestApp")
            .company_name("Test Corp");

        let result = process_template("{{app_name}} by {{company_name}}", &vars);
        assert_eq!(result, "TestApp by Test Corp");
    }

    #[test]
    fn test_process_template_custom_vars() {
        let vars = TemplateVariables::default()
            .custom("greeting", "Hello")
            .custom("target", "World");

        let result = process_template("{{greeting}}, {{target}}!", &vars);
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_process_template_missing_var() {
        let vars = TemplateVariables::default().app_name("TestApp");

        // Missing variables are left unchanged
        let result = process_template("{{app_name}} - {{missing}}", &vars);
        assert_eq!(result, "TestApp - {{missing}}");
    }

    #[test]
    fn test_has_template_variables() {
        assert!(has_template_variables("Hello {{name}}!"));
        assert!(has_template_variables("{{a}} and {{b}}"));
        assert!(!has_template_variables("No variables here"));
        assert!(!has_template_variables("Single { brace }"));
        assert!(!has_template_variables("Unclosed {{"));
    }

    #[test]
    fn test_extract_variable_names() {
        let names = extract_variable_names("{{app_name}} by {{company_name}} - {{app_name}}");
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"app_name".to_string()));
        assert!(names.contains(&"company_name".to_string()));
    }

    #[test]
    fn test_extract_variable_names_with_spaces() {
        let names = extract_variable_names("{{ var_with_spaces }}");
        assert_eq!(names, vec!["var_with_spaces"]);
    }

    #[test]
    fn test_validate_template_success() {
        let vars = TemplateVariables::default()
            .app_name("TestApp")
            .company_name("Test Corp");

        let result = validate_template("{{app_name}} by {{company_name}}", &vars);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_template_missing() {
        let vars = TemplateVariables::default().app_name("TestApp");

        let result = validate_template("{{app_name}} by {{company_name}}", &vars);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("company_name"));
    }

    #[test]
    fn test_template_variables_builder() {
        let vars = TemplateVariables::new()
            .app_name("App")
            .company_name("Company")
            .support_email("support@example.com")
            .version("1.0.0")
            .custom("key", "value");

        assert_eq!(vars.get("app_name"), Some("App"));
        assert_eq!(vars.get("company_name"), Some("Company"));
        assert_eq!(vars.get("support_email"), Some("support@example.com"));
        assert_eq!(vars.get("version"), Some("1.0.0"));
        assert_eq!(vars.get("key"), Some("value"));
        assert_eq!(vars.get("nonexistent"), None);
    }

    #[test]
    fn test_apply_templates_to_apple_metadata() {
        let mut metadata = AppleMetadata::new("com.example.app");
        metadata.set_localization(
            "en-US",
            AppleLocalizedMetadata::new("{{app_name}}", "Welcome to {{app_name}}!"),
        );
        metadata.copyright = Some("Copyright {{company_name}}".to_string());

        let vars = TemplateVariables::default()
            .app_name("MyApp")
            .company_name("Acme Inc.");

        apply_templates_to_apple_metadata(&mut metadata, &vars);

        let en = metadata.get_localization("en-US").unwrap();
        assert_eq!(en.name, "MyApp");
        assert_eq!(en.description, "Welcome to MyApp!");
        assert_eq!(metadata.copyright, Some("Copyright Acme Inc.".to_string()));
    }

    #[test]
    fn test_apply_templates_to_google_play_metadata() {
        let mut metadata = GooglePlayMetadata::new("com.example.app");
        let mut localized = GooglePlayLocalizedMetadata::new(
            "{{app_name}}",
            "{{app_name}} - {{tagline}}",
            "Full description for {{app_name}}",
        );
        localized.add_changelog("100", "{{app_name}} v{{version}} released!");
        metadata.set_localization("en-US", localized);

        let vars = TemplateVariables::default()
            .app_name("MyApp")
            .version("1.0.0")
            .custom("tagline", "Best App Ever");

        apply_templates_to_google_play_metadata(&mut metadata, &vars);

        let en = metadata.get_localization("en-US").unwrap();
        assert_eq!(en.title, "MyApp");
        assert_eq!(en.short_description, "MyApp - Best App Ever");
        assert_eq!(en.full_description, "Full description for MyApp");
        assert_eq!(
            en.changelogs.get("100"),
            Some(&"MyApp v1.0.0 released!".to_string())
        );
    }
}
