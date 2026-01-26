//! Camera HAL stub for unsupported platforms

use super::{CameraConfig, CameraError, CameraResult, CameraSettings, FrameBuffer};

/// Initialize the camera (stub - returns NotSupported)
pub fn camera_initialize(_config: CameraConfig) -> CameraResult<()> {
    Err(CameraError::NotSupported)
}

/// Deinitialize the camera (stub - returns NotSupported)
pub fn camera_deinitialize() -> CameraResult<()> {
    Err(CameraError::NotSupported)
}

/// Capture a frame (stub - returns NotSupported)
pub fn camera_capture_frame() -> CameraResult<FrameBuffer> {
    Err(CameraError::NotSupported)
}

/// Get current camera settings (stub - returns NotSupported)
pub fn camera_get_settings() -> CameraResult<CameraSettings> {
    Err(CameraError::NotSupported)
}

/// Set camera settings (stub - returns NotSupported)
pub fn camera_set_settings(_settings: CameraSettings) -> CameraResult<()> {
    Err(CameraError::NotSupported)
}

/// Check if camera is initialized (stub - always returns false)
pub fn camera_is_initialized() -> bool {
    false
}
