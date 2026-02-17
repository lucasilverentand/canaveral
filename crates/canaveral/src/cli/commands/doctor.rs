//! Doctor command - check environment for required tools and configurations

use std::path::PathBuf;
use std::process::Command;

use clap::Args;
use console::style;
use serde::Serialize;
use tracing::info;

use crate::cli::{Cli, OutputFormat};

/// Check environment for required tools and configurations
#[derive(Debug, Args)]
pub struct DoctorCommand {
    /// Show suggestions for fixing issues
    #[arg(long)]
    pub fix: bool,

    /// Only check specific categories
    #[arg(long, value_delimiter = ',')]
    pub only: Option<Vec<CheckCategory>>,

    /// Skip specific categories
    #[arg(long, value_delimiter = ',')]
    pub skip: Option<Vec<CheckCategory>>,

    /// Check for a specific framework
    #[arg(long)]
    pub framework: Option<FrameworkCheck>,
}

/// Categories of checks
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CheckCategory {
    /// Rust toolchain
    Rust,
    /// Framework CLIs (Flutter, Expo, etc.)
    Frameworks,
    /// Platform tools (Xcode, Android SDK)
    Platform,
    /// Code signing setup
    Signing,
    /// Store credentials
    Stores,
    /// Git configuration
    Git,
    /// Environment variables
    Env,
}

/// Framework to check
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum FrameworkCheck {
    Flutter,
    Expo,
    ReactNative,
    Tauri,
    NativeIos,
    NativeAndroid,
}

/// Result of a single check
#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub message: Option<String>,
    pub version: Option<String>,
    pub fix_suggestion: Option<String>,
}

/// Status of a check
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
    Skip,
}

/// Summary of all checks
#[derive(Debug, Serialize)]
pub struct DoctorSummary {
    pub checks: Vec<CheckResult>,
    pub ok_count: usize,
    pub warn_count: usize,
    pub fail_count: usize,
    pub skip_count: usize,
}

impl DoctorCommand {
    /// Execute the doctor command
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(fix = self.fix, "executing doctor command");
        let mut checks = Vec::new();

        // Determine which categories to check
        let categories = self.get_categories();

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!("{}", style("Checking environment...").bold());
            println!();
        }

        // Run checks for each category
        for category in &categories {
            let category_checks = match category {
                CheckCategory::Rust => self.check_rust(),
                CheckCategory::Frameworks => self.check_frameworks(),
                CheckCategory::Platform => self.check_platform(),
                CheckCategory::Signing => self.check_signing(),
                CheckCategory::Stores => self.check_stores(),
                CheckCategory::Git => self.check_git(),
                CheckCategory::Env => self.check_env(),
            };
            checks.extend(category_checks);
        }

        // Calculate summary
        let ok_count = checks
            .iter()
            .filter(|c| c.status == CheckStatus::Ok)
            .count();
        let warn_count = checks
            .iter()
            .filter(|c| c.status == CheckStatus::Warn)
            .count();
        let fail_count = checks
            .iter()
            .filter(|c| c.status == CheckStatus::Fail)
            .count();
        let skip_count = checks
            .iter()
            .filter(|c| c.status == CheckStatus::Skip)
            .count();

        let summary = DoctorSummary {
            checks: checks.clone(),
            ok_count,
            warn_count,
            fail_count,
            skip_count,
        };

        // Output results
        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            }
            OutputFormat::Text => {
                self.print_results(&checks, cli);
                self.print_summary(&summary);

                if self.fix && (fail_count > 0 || warn_count > 0) {
                    println!();
                    println!("{}", style("Suggested fixes:").bold());
                    for check in &checks {
                        if check.status == CheckStatus::Fail || check.status == CheckStatus::Warn {
                            if let Some(ref fix) = check.fix_suggestion {
                                println!(
                                    "  {} {}: {}",
                                    status_icon(check.status),
                                    style(&check.name).bold(),
                                    fix
                                );
                            }
                        }
                    }
                }
            }
        }

        // Return error if there are failures
        if fail_count > 0 {
            anyhow::bail!("{} check(s) failed", fail_count);
        }

        Ok(())
    }

    fn get_categories(&self) -> Vec<CheckCategory> {
        let all_categories = vec![
            CheckCategory::Rust,
            CheckCategory::Frameworks,
            CheckCategory::Platform,
            CheckCategory::Signing,
            CheckCategory::Stores,
            CheckCategory::Git,
            CheckCategory::Env,
        ];

        if let Some(ref only) = self.only {
            return only.clone();
        }

        if let Some(ref skip) = self.skip {
            return all_categories
                .into_iter()
                .filter(|c| !skip.contains(c))
                .collect();
        }

        all_categories
    }

    fn check_rust(&self) -> Vec<CheckResult> {
        let mut results = Vec::new();

        // Check rustc
        match get_command_version("rustc", &["--version"]) {
            Some(version) => {
                let status = if parse_rust_version(&version).is_some_and(|v| v >= (1, 75, 0)) {
                    CheckStatus::Ok
                } else {
                    CheckStatus::Warn
                };
                results.push(CheckResult {
                    name: "Rust compiler".to_string(),
                    status,
                    message: Some(version.clone()),
                    version: Some(version),
                    fix_suggestion: Some("Run 'rustup update' to update Rust".to_string()),
                });
            }
            None => {
                results.push(CheckResult {
                    name: "Rust compiler".to_string(),
                    status: CheckStatus::Fail,
                    message: Some("Not found".to_string()),
                    version: None,
                    fix_suggestion: Some("Install Rust from https://rustup.rs".to_string()),
                });
            }
        }

        // Check cargo
        match get_command_version("cargo", &["--version"]) {
            Some(version) => {
                results.push(CheckResult {
                    name: "Cargo".to_string(),
                    status: CheckStatus::Ok,
                    message: Some(version.clone()),
                    version: Some(version),
                    fix_suggestion: None,
                });
            }
            None => {
                results.push(CheckResult {
                    name: "Cargo".to_string(),
                    status: CheckStatus::Fail,
                    message: Some("Not found".to_string()),
                    version: None,
                    fix_suggestion: Some("Install Rust from https://rustup.rs".to_string()),
                });
            }
        }

        results
    }

    fn check_frameworks(&self) -> Vec<CheckResult> {
        let mut results = Vec::new();

        // Filter by specific framework if requested
        let check_flutter =
            self.framework.is_none() || self.framework == Some(FrameworkCheck::Flutter);
        let check_expo = self.framework.is_none() || self.framework == Some(FrameworkCheck::Expo);
        let check_rn =
            self.framework.is_none() || self.framework == Some(FrameworkCheck::ReactNative);
        let check_tauri = self.framework.is_none() || self.framework == Some(FrameworkCheck::Tauri);

        // Flutter
        if check_flutter {
            match get_command_version("flutter", &["--version"]) {
                Some(version) => {
                    let short_version = version.lines().next().unwrap_or(&version).to_string();
                    results.push(CheckResult {
                        name: "Flutter".to_string(),
                        status: CheckStatus::Ok,
                        message: Some(short_version.clone()),
                        version: Some(short_version),
                        fix_suggestion: None,
                    });
                }
                None => {
                    results.push(CheckResult {
                        name: "Flutter".to_string(),
                        status: CheckStatus::Skip,
                        message: Some("Not installed".to_string()),
                        version: None,
                        fix_suggestion: Some(
                            "Install Flutter from https://flutter.dev".to_string(),
                        ),
                    });
                }
            }
        }

        // Expo CLI
        if check_expo {
            match get_command_version("eas", &["--version"]) {
                Some(version) => {
                    results.push(CheckResult {
                        name: "EAS CLI".to_string(),
                        status: CheckStatus::Ok,
                        message: Some(version.clone()),
                        version: Some(version),
                        fix_suggestion: None,
                    });
                }
                None => {
                    results.push(CheckResult {
                        name: "EAS CLI".to_string(),
                        status: CheckStatus::Skip,
                        message: Some("Not installed".to_string()),
                        version: None,
                        fix_suggestion: Some("Run 'npm install -g eas-cli'".to_string()),
                    });
                }
            }
        }

        // React Native CLI
        if check_rn {
            match get_command_version("npx", &["react-native", "--version"]) {
                Some(version) => {
                    results.push(CheckResult {
                        name: "React Native CLI".to_string(),
                        status: CheckStatus::Ok,
                        message: Some(version.clone()),
                        version: Some(version),
                        fix_suggestion: None,
                    });
                }
                None => {
                    results.push(CheckResult {
                        name: "React Native CLI".to_string(),
                        status: CheckStatus::Skip,
                        message: Some("Not installed".to_string()),
                        version: None,
                        fix_suggestion: Some("Run 'npm install -g react-native-cli'".to_string()),
                    });
                }
            }
        }

        // Tauri CLI
        if check_tauri {
            match get_command_version("cargo", &["tauri", "--version"]) {
                Some(version) => {
                    results.push(CheckResult {
                        name: "Tauri CLI".to_string(),
                        status: CheckStatus::Ok,
                        message: Some(version.clone()),
                        version: Some(version),
                        fix_suggestion: None,
                    });
                }
                None => {
                    results.push(CheckResult {
                        name: "Tauri CLI".to_string(),
                        status: CheckStatus::Skip,
                        message: Some("Not installed".to_string()),
                        version: None,
                        fix_suggestion: Some("Run 'cargo install tauri-cli'".to_string()),
                    });
                }
            }
        }

        // Node.js (common for many frameworks)
        match get_command_version("node", &["--version"]) {
            Some(version) => {
                results.push(CheckResult {
                    name: "Node.js".to_string(),
                    status: CheckStatus::Ok,
                    message: Some(version.clone()),
                    version: Some(version),
                    fix_suggestion: None,
                });
            }
            None => {
                results.push(CheckResult {
                    name: "Node.js".to_string(),
                    status: CheckStatus::Skip,
                    message: Some("Not installed".to_string()),
                    version: None,
                    fix_suggestion: Some("Install Node.js from https://nodejs.org".to_string()),
                });
            }
        }

        results
    }

    fn check_platform(&self) -> Vec<CheckResult> {
        let mut results = Vec::new();

        #[cfg(target_os = "macos")]
        {
            // Xcode
            match get_command_version("xcodebuild", &["-version"]) {
                Some(version) => {
                    let short = version.lines().next().unwrap_or(&version).to_string();
                    results.push(CheckResult {
                        name: "Xcode".to_string(),
                        status: CheckStatus::Ok,
                        message: Some(short.clone()),
                        version: Some(short),
                        fix_suggestion: None,
                    });
                }
                None => {
                    results.push(CheckResult {
                        name: "Xcode".to_string(),
                        status: CheckStatus::Fail,
                        message: Some("Not found".to_string()),
                        version: None,
                        fix_suggestion: Some("Install Xcode from the App Store".to_string()),
                    });
                }
            }

            // Xcode Command Line Tools
            match Command::new("xcode-select").args(["--print-path"]).output() {
                Ok(output) if output.status.success() => {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    results.push(CheckResult {
                        name: "Xcode Command Line Tools".to_string(),
                        status: CheckStatus::Ok,
                        message: Some(path),
                        version: None,
                        fix_suggestion: None,
                    });
                }
                _ => {
                    results.push(CheckResult {
                        name: "Xcode Command Line Tools".to_string(),
                        status: CheckStatus::Fail,
                        message: Some("Not installed".to_string()),
                        version: None,
                        fix_suggestion: Some("Run 'xcode-select --install'".to_string()),
                    });
                }
            }

            // CocoaPods
            match get_command_version("pod", &["--version"]) {
                Some(version) => {
                    results.push(CheckResult {
                        name: "CocoaPods".to_string(),
                        status: CheckStatus::Ok,
                        message: Some(version.clone()),
                        version: Some(version),
                        fix_suggestion: None,
                    });
                }
                None => {
                    results.push(CheckResult {
                        name: "CocoaPods".to_string(),
                        status: CheckStatus::Warn,
                        message: Some("Not installed".to_string()),
                        version: None,
                        fix_suggestion: Some("Run 'sudo gem install cocoapods'".to_string()),
                    });
                }
            }
        }

        // Android SDK
        let android_home = std::env::var("ANDROID_HOME")
            .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
            .ok();

        match android_home {
            Some(ref path) if PathBuf::from(path).exists() => {
                results.push(CheckResult {
                    name: "Android SDK".to_string(),
                    status: CheckStatus::Ok,
                    message: Some(path.clone()),
                    version: None,
                    fix_suggestion: None,
                });

                // Check for common tools
                let sdk_path = PathBuf::from(path);

                // adb
                let adb_path = sdk_path.join("platform-tools").join("adb");
                if adb_path.exists() || which::which("adb").is_ok() {
                    results.push(CheckResult {
                        name: "Android ADB".to_string(),
                        status: CheckStatus::Ok,
                        message: Some("Found".to_string()),
                        version: None,
                        fix_suggestion: None,
                    });
                } else {
                    results.push(CheckResult {
                        name: "Android ADB".to_string(),
                        status: CheckStatus::Warn,
                        message: Some("Not found".to_string()),
                        version: None,
                        fix_suggestion: Some("Install via Android SDK Manager".to_string()),
                    });
                }
            }
            _ => {
                results.push(CheckResult {
                    name: "Android SDK".to_string(),
                    status: CheckStatus::Skip,
                    message: Some("ANDROID_HOME not set".to_string()),
                    version: None,
                    fix_suggestion: Some(
                        "Set ANDROID_HOME environment variable to your Android SDK path"
                            .to_string(),
                    ),
                });
            }
        }

        // Java/JDK
        match get_command_version("java", &["-version"]) {
            Some(version) => {
                let short = version.lines().next().unwrap_or(&version).to_string();
                results.push(CheckResult {
                    name: "Java".to_string(),
                    status: CheckStatus::Ok,
                    message: Some(short.clone()),
                    version: Some(short),
                    fix_suggestion: None,
                });
            }
            None => {
                results.push(CheckResult {
                    name: "Java".to_string(),
                    status: CheckStatus::Skip,
                    message: Some("Not installed".to_string()),
                    version: None,
                    fix_suggestion: Some("Install JDK 17+ for Android development".to_string()),
                });
            }
        }

        results
    }

    fn check_signing(&self) -> Vec<CheckResult> {
        let mut results = Vec::new();

        #[cfg(target_os = "macos")]
        {
            // Check for signing identities
            match Command::new("security")
                .args(["find-identity", "-v", "-p", "codesigning"])
                .output()
            {
                Ok(output) if output.status.success() => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    let identity_count = output_str
                        .lines()
                        .filter(|l| {
                            l.contains("iPhone") || l.contains("Apple") || l.contains("Mac")
                        })
                        .count();

                    if identity_count > 0 {
                        results.push(CheckResult {
                            name: "Code signing identities".to_string(),
                            status: CheckStatus::Ok,
                            message: Some(format!("{} identity(ies) found", identity_count)),
                            version: None,
                            fix_suggestion: None,
                        });
                    } else {
                        results.push(CheckResult {
                            name: "Code signing identities".to_string(),
                            status: CheckStatus::Warn,
                            message: Some("No identities found".to_string()),
                            version: None,
                            fix_suggestion: Some(
                                "Import certificates from Apple Developer Portal or use 'canaveral match sync'".to_string(),
                            ),
                        });
                    }
                }
                _ => {
                    results.push(CheckResult {
                        name: "Code signing identities".to_string(),
                        status: CheckStatus::Fail,
                        message: Some("Could not check".to_string()),
                        version: None,
                        fix_suggestion: None,
                    });
                }
            }

            // Check for provisioning profiles
            let profiles_dir =
                dirs::home_dir().map(|h| h.join("Library/MobileDevice/Provisioning Profiles"));

            if let Some(dir) = profiles_dir {
                if dir.exists() {
                    let profile_count = std::fs::read_dir(&dir)
                        .map(|entries| entries.filter(|e| e.is_ok()).count())
                        .unwrap_or(0);

                    if profile_count > 0 {
                        results.push(CheckResult {
                            name: "Provisioning profiles".to_string(),
                            status: CheckStatus::Ok,
                            message: Some(format!("{} profile(s) found", profile_count)),
                            version: None,
                            fix_suggestion: None,
                        });
                    } else {
                        results.push(CheckResult {
                            name: "Provisioning profiles".to_string(),
                            status: CheckStatus::Warn,
                            message: Some("No profiles found".to_string()),
                            version: None,
                            fix_suggestion: Some(
                                "Download profiles from Apple Developer Portal or use 'canaveral match sync'".to_string(),
                            ),
                        });
                    }
                }
            }
        }

        // Android keystore check
        let debug_keystore = dirs::home_dir().map(|h| h.join(".android/debug.keystore"));
        if let Some(ref path) = debug_keystore {
            if path.exists() {
                results.push(CheckResult {
                    name: "Android debug keystore".to_string(),
                    status: CheckStatus::Ok,
                    message: Some(path.display().to_string()),
                    version: None,
                    fix_suggestion: None,
                });
            } else {
                results.push(CheckResult {
                    name: "Android debug keystore".to_string(),
                    status: CheckStatus::Skip,
                    message: Some("Not found (will be created on first build)".to_string()),
                    version: None,
                    fix_suggestion: None,
                });
            }
        }

        results
    }

    fn check_stores(&self) -> Vec<CheckResult> {
        let mut results = Vec::new();

        // App Store Connect
        let asc_key_id = std::env::var("APP_STORE_CONNECT_API_KEY_ID").ok();
        let asc_issuer = std::env::var("APP_STORE_CONNECT_ISSUER_ID").ok();
        let asc_key = std::env::var("APP_STORE_CONNECT_API_KEY")
            .ok()
            .or_else(|| std::env::var("APP_STORE_CONNECT_API_KEY_PATH").ok());

        if asc_key_id.is_some() && asc_issuer.is_some() && asc_key.is_some() {
            results.push(CheckResult {
                name: "App Store Connect API".to_string(),
                status: CheckStatus::Ok,
                message: Some("Credentials configured".to_string()),
                version: None,
                fix_suggestion: None,
            });
        } else {
            let mut missing = Vec::new();
            if asc_key_id.is_none() {
                missing.push("APP_STORE_CONNECT_API_KEY_ID");
            }
            if asc_issuer.is_none() {
                missing.push("APP_STORE_CONNECT_ISSUER_ID");
            }
            if asc_key.is_none() {
                missing.push("APP_STORE_CONNECT_API_KEY or APP_STORE_CONNECT_API_KEY_PATH");
            }

            results.push(CheckResult {
                name: "App Store Connect API".to_string(),
                status: CheckStatus::Skip,
                message: Some(format!("Missing: {}", missing.join(", "))),
                version: None,
                fix_suggestion: Some(
                    "Create an API key at https://appstoreconnect.apple.com/access/api".to_string(),
                ),
            });
        }

        // Google Play Console
        let gpc_key = std::env::var("GOOGLE_PLAY_SERVICE_ACCOUNT_KEY")
            .ok()
            .or_else(|| std::env::var("GOOGLE_PLAY_JSON_KEY").ok())
            .or_else(|| std::env::var("SUPPLY_JSON_KEY").ok());

        if gpc_key.is_some() {
            // Validate it's a valid path or JSON
            let is_valid = gpc_key
                .as_ref()
                .is_some_and(|k| PathBuf::from(k).exists() || k.trim().starts_with('{'));

            if is_valid {
                results.push(CheckResult {
                    name: "Google Play Console".to_string(),
                    status: CheckStatus::Ok,
                    message: Some("Service account configured".to_string()),
                    version: None,
                    fix_suggestion: None,
                });
            } else {
                results.push(CheckResult {
                    name: "Google Play Console".to_string(),
                    status: CheckStatus::Warn,
                    message: Some("Key path does not exist".to_string()),
                    version: None,
                    fix_suggestion: Some("Verify the service account JSON file path".to_string()),
                });
            }
        } else {
            results.push(CheckResult {
                name: "Google Play Console".to_string(),
                status: CheckStatus::Skip,
                message: Some("Service account not configured".to_string()),
                version: None,
                fix_suggestion: Some(
                    "Create a service account at https://play.google.com/console".to_string(),
                ),
            });
        }

        // Firebase (optional)
        let firebase_token = std::env::var("FIREBASE_TOKEN").ok();
        let google_app_creds = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok();

        if firebase_token.is_some() || google_app_creds.is_some() {
            results.push(CheckResult {
                name: "Firebase".to_string(),
                status: CheckStatus::Ok,
                message: Some("Credentials configured".to_string()),
                version: None,
                fix_suggestion: None,
            });
        } else {
            results.push(CheckResult {
                name: "Firebase".to_string(),
                status: CheckStatus::Skip,
                message: Some("Not configured (optional)".to_string()),
                version: None,
                fix_suggestion: Some("Run 'firebase login:ci' to get a token".to_string()),
            });
        }

        results
    }

    fn check_git(&self) -> Vec<CheckResult> {
        let mut results = Vec::new();

        // Git version
        match get_command_version("git", &["--version"]) {
            Some(version) => {
                results.push(CheckResult {
                    name: "Git".to_string(),
                    status: CheckStatus::Ok,
                    message: Some(version.clone()),
                    version: Some(version),
                    fix_suggestion: None,
                });
            }
            None => {
                results.push(CheckResult {
                    name: "Git".to_string(),
                    status: CheckStatus::Fail,
                    message: Some("Not found".to_string()),
                    version: None,
                    fix_suggestion: Some("Install Git from https://git-scm.com".to_string()),
                });
            }
        }

        // Git user configuration
        let user_name = Command::new("git")
            .args(["config", "--global", "user.name"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

        let user_email = Command::new("git")
            .args(["config", "--global", "user.email"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

        if user_name.is_some() && user_email.is_some() {
            results.push(CheckResult {
                name: "Git user config".to_string(),
                status: CheckStatus::Ok,
                message: Some(format!("{} <{}>", user_name.unwrap(), user_email.unwrap())),
                version: None,
                fix_suggestion: None,
            });
        } else {
            results.push(CheckResult {
                name: "Git user config".to_string(),
                status: CheckStatus::Warn,
                message: Some("Not configured".to_string()),
                version: None,
                fix_suggestion: Some(
                    "Run 'git config --global user.name' and 'git config --global user.email'"
                        .to_string(),
                ),
            });
        }

        // GPG signing (optional)
        match which::which("gpg") {
            Ok(_) => {
                let signing_key = Command::new("git")
                    .args(["config", "--global", "user.signingkey"])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .filter(|s| !s.is_empty());

                if signing_key.is_some() {
                    results.push(CheckResult {
                        name: "Git GPG signing".to_string(),
                        status: CheckStatus::Ok,
                        message: Some("Configured".to_string()),
                        version: None,
                        fix_suggestion: None,
                    });
                } else {
                    results.push(CheckResult {
                        name: "Git GPG signing".to_string(),
                        status: CheckStatus::Skip,
                        message: Some("Not configured (optional)".to_string()),
                        version: None,
                        fix_suggestion: Some(
                            "Configure with 'git config --global user.signingkey <key-id>'"
                                .to_string(),
                        ),
                    });
                }
            }
            Err(_) => {
                results.push(CheckResult {
                    name: "GPG".to_string(),
                    status: CheckStatus::Skip,
                    message: Some("Not installed (optional)".to_string()),
                    version: None,
                    fix_suggestion: Some("Install GPG for commit signing".to_string()),
                });
            }
        }

        results
    }

    fn check_env(&self) -> Vec<CheckResult> {
        let mut results = Vec::new();

        // CI environment detection
        let ci_env = detect_ci_environment();
        if let Some(ci) = ci_env {
            results.push(CheckResult {
                name: "CI Environment".to_string(),
                status: CheckStatus::Ok,
                message: Some(ci),
                version: None,
                fix_suggestion: None,
            });
        } else {
            results.push(CheckResult {
                name: "CI Environment".to_string(),
                status: CheckStatus::Skip,
                message: Some("Not detected (running locally)".to_string()),
                version: None,
                fix_suggestion: None,
            });
        }

        // PATH sanity check
        let path = std::env::var("PATH").unwrap_or_default();
        if path.is_empty() {
            results.push(CheckResult {
                name: "PATH".to_string(),
                status: CheckStatus::Fail,
                message: Some("Empty".to_string()),
                version: None,
                fix_suggestion: Some("Check your shell configuration".to_string()),
            });
        } else {
            results.push(CheckResult {
                name: "PATH".to_string(),
                status: CheckStatus::Ok,
                message: Some(format!("{} entries", path.split(':').count())),
                version: None,
                fix_suggestion: None,
            });
        }

        // HOME directory
        if let Some(home) = dirs::home_dir() {
            results.push(CheckResult {
                name: "HOME".to_string(),
                status: CheckStatus::Ok,
                message: Some(home.display().to_string()),
                version: None,
                fix_suggestion: None,
            });
        } else {
            results.push(CheckResult {
                name: "HOME".to_string(),
                status: CheckStatus::Warn,
                message: Some("Not set".to_string()),
                version: None,
                fix_suggestion: Some("Set HOME environment variable".to_string()),
            });
        }

        results
    }

    fn print_results(&self, checks: &[CheckResult], cli: &Cli) {
        if cli.quiet {
            return;
        }

        for check in checks {
            let icon = status_icon(check.status);
            let name = &check.name;
            let msg = check.message.as_deref().unwrap_or("");

            match check.status {
                CheckStatus::Ok => {
                    println!("  {} {} {}", icon, style(name).green(), style(msg).dim());
                }
                CheckStatus::Warn => {
                    println!("  {} {} {}", icon, style(name).yellow(), style(msg).dim());
                }
                CheckStatus::Fail => {
                    println!("  {} {} {}", icon, style(name).red(), style(msg).dim());
                }
                CheckStatus::Skip => {
                    println!("  {} {} {}", icon, style(name).dim(), style(msg).dim());
                }
            }
        }
    }

    fn print_summary(&self, summary: &DoctorSummary) {
        println!();
        let total = summary.ok_count + summary.warn_count + summary.fail_count + summary.skip_count;

        if summary.fail_count == 0 && summary.warn_count == 0 {
            println!(
                "{} All {} checks passed!",
                style("✓").green().bold(),
                summary.ok_count
            );
        } else {
            println!(
                "Summary: {} ok, {} warnings, {} failed, {} skipped (out of {})",
                style(summary.ok_count).green(),
                style(summary.warn_count).yellow(),
                style(summary.fail_count).red(),
                style(summary.skip_count).dim(),
                total
            );

            if summary.fail_count > 0 {
                println!();
                println!(
                    "{} {} issue(s) found. Run '{}' for suggestions.",
                    style("!").red().bold(),
                    summary.fail_count + summary.warn_count,
                    style("canaveral doctor --fix").cyan()
                );
            }
        }
    }
}

/// Get status icon for a check
fn status_icon(status: CheckStatus) -> console::StyledObject<&'static str> {
    match status {
        CheckStatus::Ok => style("[OK]").green(),
        CheckStatus::Warn => style("[WARN]").yellow(),
        CheckStatus::Fail => style("[FAIL]").red(),
        CheckStatus::Skip => style("[SKIP]").dim(),
    }
}

/// Get version output from a command
fn get_command_version(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            // Some commands (like java) output to stderr
            let output = if stdout.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                stdout.trim().to_string()
            };
            output
        })
}

/// Parse Rust version string
fn parse_rust_version(version: &str) -> Option<(u32, u32, u32)> {
    // Format: rustc 1.75.0 (82e1608df 2023-12-21)
    let parts: Vec<&str> = version.split_whitespace().collect();
    if parts.len() >= 2 {
        let version_str = parts[1];
        let nums: Vec<&str> = version_str.split('.').collect();
        if nums.len() >= 3 {
            let major = nums[0].parse().ok()?;
            let minor = nums[1].parse().ok()?;
            let patch = nums[2].split('-').next()?.parse().ok()?;
            return Some((major, minor, patch));
        }
    }
    None
}

/// Detect CI environment
fn detect_ci_environment() -> Option<String> {
    // GitHub Actions
    if std::env::var("GITHUB_ACTIONS").is_ok() {
        return Some("GitHub Actions".to_string());
    }

    // GitLab CI
    if std::env::var("GITLAB_CI").is_ok() {
        return Some("GitLab CI".to_string());
    }

    // CircleCI
    if std::env::var("CIRCLECI").is_ok() {
        return Some("CircleCI".to_string());
    }

    // Travis CI
    if std::env::var("TRAVIS").is_ok() {
        return Some("Travis CI".to_string());
    }

    // Azure DevOps
    if std::env::var("TF_BUILD").is_ok() {
        return Some("Azure DevOps".to_string());
    }

    // Bitrise
    if std::env::var("BITRISE_IO").is_ok() {
        return Some("Bitrise".to_string());
    }

    // Jenkins
    if std::env::var("JENKINS_URL").is_ok() {
        return Some("Jenkins".to_string());
    }

    // Buildkite
    if std::env::var("BUILDKITE").is_ok() {
        return Some("Buildkite".to_string());
    }

    // Generic CI detection
    if std::env::var("CI").is_ok() {
        return Some("Unknown CI".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_version() {
        assert_eq!(
            parse_rust_version("rustc 1.75.0 (82e1608df 2023-12-21)"),
            Some((1, 75, 0))
        );
        assert_eq!(parse_rust_version("rustc 1.76.0-nightly"), Some((1, 76, 0)));
    }

    #[test]
    fn test_status_icon() {
        // Just verify it doesn't panic
        let _ = status_icon(CheckStatus::Ok);
        let _ = status_icon(CheckStatus::Warn);
        let _ = status_icon(CheckStatus::Fail);
        let _ = status_icon(CheckStatus::Skip);
    }
}
