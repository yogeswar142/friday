pub mod linux;
pub mod macos;
pub mod mock;
pub mod traits;
pub mod windows;

pub use linux::LinuxPlatformBackend;
pub use macos::MacOSPlatformBackend;
pub use mock::MockPlatformBackend;
pub use traits::{CursorBackend, EventCallback, InputBackend, ScreenBackend};
pub use windows::WindowsPlatformBackend;

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
