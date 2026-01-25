//! Screenshot framing and compositing
//!
//! Add device frames, text overlays, and backgrounds to screenshots
//! for app store marketing materials.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{FrameworkError, Result};

use super::devices::DeviceType;

/// Frame configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameConfig {
    /// Template to use
    pub template: FrameTemplate,

    /// Background color (hex)
    pub background_color: String,

    /// Title text
    pub title: Option<String>,

    /// Subtitle text
    pub subtitle: Option<String>,

    /// Title font
    pub title_font: String,

    /// Title font size
    pub title_font_size: u32,

    /// Title color (hex)
    pub title_color: String,

    /// Subtitle font
    pub subtitle_font: String,

    /// Subtitle font size
    pub subtitle_font_size: u32,

    /// Subtitle color (hex)
    pub subtitle_color: String,

    /// Padding from edges
    pub padding: u32,

    /// Device frame shadow
    pub show_shadow: bool,

    /// Text position
    pub text_position: TextPosition,
}

impl Default for FrameConfig {
    fn default() -> Self {
        Self {
            template: FrameTemplate::Minimal,
            background_color: "#FFFFFF".to_string(),
            title: None,
            subtitle: None,
            title_font: "SF Pro Display".to_string(),
            title_font_size: 72,
            title_color: "#000000".to_string(),
            subtitle_font: "SF Pro Display".to_string(),
            subtitle_font_size: 48,
            subtitle_color: "#666666".to_string(),
            padding: 100,
            show_shadow: true,
            text_position: TextPosition::Top,
        }
    }
}

impl FrameConfig {
    /// Create a new frame config
    pub fn new() -> Self {
        Self::default()
    }

    /// Set title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set subtitle
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Set background color
    pub fn with_background(mut self, color: impl Into<String>) -> Self {
        self.background_color = color.into();
        self
    }

    /// Set template
    pub fn with_template(mut self, template: FrameTemplate) -> Self {
        self.template = template;
        self
    }

    /// Set text position
    pub fn with_text_position(mut self, position: TextPosition) -> Self {
        self.text_position = position;
        self
    }
}

/// Frame template style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameTemplate {
    /// No device frame, just background
    Minimal,
    /// Device frame with screenshot
    DeviceFrame,
    /// Device frame with perspective
    Perspective,
    /// Side by side comparison
    SideBySide,
    /// Floating device
    Floating,
}

/// Text position relative to device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextPosition {
    /// Text above device
    Top,
    /// Text below device
    Bottom,
    /// Text to the left
    Left,
    /// Text to the right
    Right,
}

/// Screenshot framer
pub struct ScreenshotFramer {
    /// Frame configuration
    config: FrameConfig,
}

impl ScreenshotFramer {
    /// Create a new framer with config
    pub fn new(config: FrameConfig) -> Self {
        Self { config }
    }

    /// Frame a screenshot
    pub fn frame(
        &self,
        screenshot_path: &Path,
        device_type: DeviceType,
        output_path: &Path,
    ) -> Result<()> {
        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| FrameworkError::Context {
                context: "create framed output dir".to_string(),
                message: e.to_string(),
            })?;
        }

        match self.config.template {
            FrameTemplate::Minimal => self.frame_minimal(screenshot_path, output_path),
            FrameTemplate::DeviceFrame => {
                self.frame_with_device(screenshot_path, device_type, output_path)
            }
            FrameTemplate::Perspective => {
                self.frame_perspective(screenshot_path, device_type, output_path)
            }
            FrameTemplate::SideBySide => {
                self.frame_side_by_side(screenshot_path, device_type, output_path)
            }
            FrameTemplate::Floating => {
                self.frame_floating(screenshot_path, device_type, output_path)
            }
        }
    }

    /// Create minimal frame (just background + text)
    fn frame_minimal(&self, screenshot_path: &Path, output_path: &Path) -> Result<()> {
        // Use ImageMagick for image manipulation
        let mut args = vec![
            screenshot_path.to_str().unwrap_or_default().to_string(),
        ];

        // Add background
        args.extend([
            "-background".to_string(),
            self.config.background_color.clone(),
            "-gravity".to_string(),
            "center".to_string(),
        ]);

        // Add padding
        args.extend([
            "-extent".to_string(),
            format!(
                "{}x{}",
                self.get_canvas_width(),
                self.get_canvas_height()
            ),
        ]);

        // Add title if present
        if let Some(ref title) = self.config.title {
            args.extend([
                "-gravity".to_string(),
                self.gravity_for_position().to_string(),
                "-fill".to_string(),
                self.config.title_color.clone(),
                "-pointsize".to_string(),
                self.config.title_font_size.to_string(),
                "-annotate".to_string(),
                format!("+0+{}", self.config.padding),
                title.clone(),
            ]);
        }

        // Add subtitle if present
        if let Some(ref subtitle) = self.config.subtitle {
            args.extend([
                "-gravity".to_string(),
                self.gravity_for_position().to_string(),
                "-fill".to_string(),
                self.config.subtitle_color.clone(),
                "-pointsize".to_string(),
                self.config.subtitle_font_size.to_string(),
                "-annotate".to_string(),
                format!("+0+{}", self.config.padding + self.config.title_font_size + 20),
                subtitle.clone(),
            ]);
        }

        args.push(output_path.to_str().unwrap_or_default().to_string());

        self.run_convert(&args)
    }

    /// Create frame with device mockup
    fn frame_with_device(
        &self,
        screenshot_path: &Path,
        device_type: DeviceType,
        output_path: &Path,
    ) -> Result<()> {
        // Get device frame path
        let frame_path = self.get_device_frame_path(device_type)?;

        // Composite screenshot into device frame
        let args = vec![
            frame_path.to_str().unwrap_or_default().to_string(),
            screenshot_path.to_str().unwrap_or_default().to_string(),
            "-gravity".to_string(),
            "center".to_string(),
            "-composite".to_string(),
            "-background".to_string(),
            self.config.background_color.clone(),
            "-extent".to_string(),
            format!("{}x{}", self.get_canvas_width(), self.get_canvas_height()),
            output_path.to_str().unwrap_or_default().to_string(),
        ];

        self.run_convert(&args)
    }

    /// Create perspective frame
    fn frame_perspective(
        &self,
        screenshot_path: &Path,
        _device_type: DeviceType,
        output_path: &Path,
    ) -> Result<()> {
        // Apply perspective transform
        let args = vec![
            screenshot_path.to_str().unwrap_or_default().to_string(),
            "-matte".to_string(),
            "-virtual-pixel".to_string(),
            "transparent".to_string(),
            "-distort".to_string(),
            "Perspective".to_string(),
            "0,0 100,50  0,100 100,100  100,0 200,0  100,100 200,100".to_string(),
            "-background".to_string(),
            self.config.background_color.clone(),
            output_path.to_str().unwrap_or_default().to_string(),
        ];

        self.run_convert(&args)
    }

    /// Create side by side frame
    fn frame_side_by_side(
        &self,
        screenshot_path: &Path,
        _device_type: DeviceType,
        output_path: &Path,
    ) -> Result<()> {
        // For side by side, we'd need multiple screenshots
        // For now, just duplicate the screenshot
        let args = vec![
            screenshot_path.to_str().unwrap_or_default().to_string(),
            screenshot_path.to_str().unwrap_or_default().to_string(),
            "+append".to_string(),
            "-background".to_string(),
            self.config.background_color.clone(),
            output_path.to_str().unwrap_or_default().to_string(),
        ];

        self.run_convert(&args)
    }

    /// Create floating frame
    fn frame_floating(
        &self,
        screenshot_path: &Path,
        _device_type: DeviceType,
        output_path: &Path,
    ) -> Result<()> {
        // Add shadow and rotation for floating effect
        let args = vec![
            screenshot_path.to_str().unwrap_or_default().to_string(),
            "-rotate".to_string(),
            "-5".to_string(),
            "(".to_string(),
            "+clone".to_string(),
            "-background".to_string(),
            "black".to_string(),
            "-shadow".to_string(),
            "80x10+0+10".to_string(),
            ")".to_string(),
            "+swap".to_string(),
            "-background".to_string(),
            self.config.background_color.clone(),
            "-layers".to_string(),
            "merge".to_string(),
            output_path.to_str().unwrap_or_default().to_string(),
        ];

        self.run_convert(&args)
    }

    /// Run ImageMagick convert command
    fn run_convert(&self, args: &[String]) -> Result<()> {
        let output = std::process::Command::new("convert")
            .args(args)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("convert {}", args.join(" ")),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(FrameworkError::CommandFailed {
                command: format!("convert {}", args.join(" ")),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    /// Get gravity string for text position
    fn gravity_for_position(&self) -> &str {
        match self.config.text_position {
            TextPosition::Top => "North",
            TextPosition::Bottom => "South",
            TextPosition::Left => "West",
            TextPosition::Right => "East",
        }
    }

    /// Get canvas width
    fn get_canvas_width(&self) -> u32 {
        // Default to iPhone 6.7" width with padding
        1290 + (self.config.padding * 2)
    }

    /// Get canvas height
    fn get_canvas_height(&self) -> u32 {
        // Default to iPhone 6.7" height with padding for text
        2796 + (self.config.padding * 3)
    }

    /// Get device frame path
    fn get_device_frame_path(&self, device_type: DeviceType) -> Result<PathBuf> {
        // Device frames would be bundled with the tool or downloaded
        let frame_name = match device_type {
            DeviceType::IPhone => "iphone.png",
            DeviceType::IPhoneDynamicIsland => "iphone_dynamic_island.png",
            DeviceType::IPad => "ipad.png",
            DeviceType::IPadPro => "ipad_pro.png",
            DeviceType::AndroidPhone => "android_phone.png",
            DeviceType::AndroidTablet => "android_tablet.png",
        };

        // Look for frames in standard locations
        let locations = vec![
            PathBuf::from(format!("~/.canaveral/frames/{}", frame_name)),
            PathBuf::from(format!("/usr/local/share/canaveral/frames/{}", frame_name)),
            PathBuf::from(format!("frames/{}", frame_name)),
        ];

        for location in &locations {
            let expanded = shellexpand::tilde(location.to_str().unwrap_or_default());
            let path = PathBuf::from(expanded.as_ref());
            if path.exists() {
                return Ok(path);
            }
        }

        Err(FrameworkError::Context {
            context: "find device frame".to_string(),
            message: format!(
                "Device frame '{}' not found. Install frames with 'canaveral frames install'",
                frame_name
            ),
        })
    }
}

impl Default for ScreenshotFramer {
    fn default() -> Self {
        Self::new(FrameConfig::default())
    }
}

/// Frame preset for common styles
pub mod presets {
    use super::*;

    /// Clean Apple-style preset
    pub fn apple_style() -> FrameConfig {
        FrameConfig {
            template: FrameTemplate::DeviceFrame,
            background_color: "#FFFFFF".to_string(),
            title_font: "SF Pro Display".to_string(),
            title_font_size: 72,
            title_color: "#000000".to_string(),
            subtitle_font: "SF Pro Display".to_string(),
            subtitle_font_size: 48,
            subtitle_color: "#666666".to_string(),
            padding: 120,
            show_shadow: true,
            text_position: TextPosition::Top,
            ..Default::default()
        }
    }

    /// Gradient background preset
    pub fn gradient() -> FrameConfig {
        FrameConfig {
            template: FrameTemplate::Floating,
            background_color: "gradient:#667eea-#764ba2".to_string(),
            title_color: "#FFFFFF".to_string(),
            subtitle_color: "#EEEEEE".to_string(),
            ..apple_style()
        }
    }

    /// Minimal preset (no device frame)
    pub fn minimal() -> FrameConfig {
        FrameConfig {
            template: FrameTemplate::Minimal,
            background_color: "#F5F5F5".to_string(),
            padding: 80,
            show_shadow: false,
            ..Default::default()
        }
    }

    /// Dark mode preset
    pub fn dark() -> FrameConfig {
        FrameConfig {
            template: FrameTemplate::DeviceFrame,
            background_color: "#1C1C1E".to_string(),
            title_color: "#FFFFFF".to_string(),
            subtitle_color: "#8E8E93".to_string(),
            ..Default::default()
        }
    }
}

/// Localized text for screenshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizedText {
    /// Locale code
    pub locale: String,

    /// Title text
    pub title: String,

    /// Subtitle text
    pub subtitle: Option<String>,
}

/// Screenshot text configuration with localization
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScreenshotText {
    /// Localized text variants
    pub texts: Vec<LocalizedText>,
}

impl ScreenshotText {
    /// Create new text config
    pub fn new() -> Self {
        Self::default()
    }

    /// Add localized text
    pub fn with_locale(
        mut self,
        locale: impl Into<String>,
        title: impl Into<String>,
        subtitle: Option<String>,
    ) -> Self {
        self.texts.push(LocalizedText {
            locale: locale.into(),
            title: title.into(),
            subtitle,
        });
        self
    }

    /// Get text for locale
    pub fn get(&self, locale: &str) -> Option<&LocalizedText> {
        self.texts
            .iter()
            .find(|t| t.locale == locale)
            .or_else(|| {
                // Fallback to base language (e.g., "en" for "en_US")
                let base = locale.split('_').next()?;
                self.texts.iter().find(|t| t.locale.starts_with(base))
            })
            .or_else(|| {
                // Fallback to English
                self.texts.iter().find(|t| t.locale.starts_with("en"))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_config_default() {
        let config = FrameConfig::default();
        assert_eq!(config.background_color, "#FFFFFF");
        assert_eq!(config.template, FrameTemplate::Minimal);
    }

    #[test]
    fn test_frame_config_builder() {
        let config = FrameConfig::new()
            .with_title("My App")
            .with_subtitle("The best app ever")
            .with_background("#000000");

        assert_eq!(config.title, Some("My App".to_string()));
        assert_eq!(config.subtitle, Some("The best app ever".to_string()));
        assert_eq!(config.background_color, "#000000");
    }

    #[test]
    fn test_presets() {
        let apple = presets::apple_style();
        assert_eq!(apple.template, FrameTemplate::DeviceFrame);

        let dark = presets::dark();
        assert_eq!(dark.background_color, "#1C1C1E");
    }

    #[test]
    fn test_localized_text() {
        let text = ScreenshotText::new()
            .with_locale("en_US", "Hello", Some("Welcome".to_string()))
            .with_locale("de_DE", "Hallo", Some("Willkommen".to_string()));

        let en = text.get("en_US").unwrap();
        assert_eq!(en.title, "Hello");

        let de = text.get("de_DE").unwrap();
        assert_eq!(de.title, "Hallo");

        // Fallback to English for unknown locale
        let fr = text.get("fr_FR").unwrap();
        assert_eq!(fr.title, "Hello");
    }
}
