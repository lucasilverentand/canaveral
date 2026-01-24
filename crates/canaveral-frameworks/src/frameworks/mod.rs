//! Framework adapter implementations
//!
//! Each framework adapter knows how to detect, build, test, and manage versions
//! for a specific framework (Flutter, Expo, React Native, native, etc.).

pub mod flutter;
pub mod expo;
pub mod react_native;
pub mod native_ios;
pub mod native_android;
pub mod tauri;

// Re-export adapters
pub use flutter::FlutterAdapter;
pub use expo::ExpoAdapter;
pub use react_native::ReactNativeAdapter;
pub use native_ios::NativeIosAdapter;
pub use native_android::NativeAndroidAdapter;
pub use tauri::TauriAdapter;

use crate::registry::FrameworkRegistry;

/// Register all built-in framework adapters
pub fn register_all(registry: &mut FrameworkRegistry) {
    // Build adapters (order matters for detection priority)
    registry.register_build(FlutterAdapter::new());
    registry.register_build(ExpoAdapter::new());
    registry.register_build(ReactNativeAdapter::new());
    registry.register_build(TauriAdapter::new());
    registry.register_build(NativeIosAdapter::new());
    registry.register_build(NativeAndroidAdapter::new());
}
