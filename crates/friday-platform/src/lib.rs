pub mod traits;
pub mod mock;
pub mod linux;
pub mod windows;
pub mod macos;

pub use traits::{CursorBackend, EventCallback, InputBackend, ScreenBackend};
pub use mock::MockPlatformBackend;
pub use linux::LinuxPlatformBackend;
pub use windows::WindowsPlatformBackend;
pub use macos::MacOSPlatformBackend;

/// Instantiates the active platform backend for the target OS
pub fn get_native_backend() -> std::sync::Arc<dyn InputBackend> {
    #[cfg(target_os = "linux")]
    {
        std::sync::Arc::new(LinuxPlatformBackend::new())
    }

    #[cfg(target_os = "windows")]
    {
        std::sync::Arc::new(WindowsPlatformBackend::new())
    }

    #[cfg(target_os = "macos")]
    {
        std::sync::Arc::new(MacOSPlatformBackend::new())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        std::sync::Arc::new(MockPlatformBackend::default_1080p())
    }
}
