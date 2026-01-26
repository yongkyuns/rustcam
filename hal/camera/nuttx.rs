//! NuttX Camera implementation using V4L2-like API via C wrapper
//!
//! This implementation calls into a C wrapper (camera_wrapper.c) that handles
//! all the V4L2 interactions. This simplifies FFI and handles the complex
//! buffer management on the C side.

use super::{
    CameraConfig, CameraError, CameraResult, CameraSettings, FrameBuffer, PixelFormat, Resolution,
};
use core::ffi::c_int;

// ============================================================================
// C Wrapper FFI Bindings
// ============================================================================

extern "C" {
    /// Initialize camera subsystem
    fn rust_camera_wrapper_init(format: c_int, resolution: c_int, quality: c_int) -> c_int;

    /// Deinitialize camera subsystem
    fn rust_camera_wrapper_deinit() -> c_int;

    /// Capture a single frame
    fn rust_camera_wrapper_capture(
        width: *mut u32,
        height: *mut u32,
        format: *mut c_int,
        len: *mut usize,
        buf: *mut *const u8,
    ) -> c_int;

    /// Return frame buffer after processing
    fn rust_camera_wrapper_return_frame();

    /// Check if camera is initialized
    fn rust_camera_wrapper_is_initialized() -> c_int;

    /// Get sensor settings
    fn rust_camera_wrapper_get_sensor(
        brightness: *mut i8,
        contrast: *mut i8,
        saturation: *mut i8,
    ) -> c_int;

    /// Set sensor settings
    fn rust_camera_wrapper_set_sensor(
        brightness: i8,
        contrast: i8,
        saturation: i8,
        hmirror: c_int,
        vflip: c_int,
    ) -> c_int;
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert Resolution enum to C integer
fn resolution_to_int(res: Resolution) -> c_int {
    match res {
        Resolution::Qqvga => 0,
        Resolution::Qcif => 1,
        Resolution::Hqvga => 2,
        Resolution::Qvga => 3,
        Resolution::Cif => 4,
        Resolution::Hvga => 5,
        Resolution::Vga => 6,
        Resolution::Svga => 7,
        Resolution::Xga => 8,
        Resolution::Hd => 9,
        Resolution::Sxga => 10,
        Resolution::Uxga => 11,
    }
}

/// Convert PixelFormat enum to C integer
fn format_to_int(fmt: PixelFormat) -> c_int {
    match fmt {
        PixelFormat::Jpeg => 0,
        PixelFormat::Rgb565 => 1,
        PixelFormat::Rgb888 => 2,
        PixelFormat::Yuv422 => 3,
        PixelFormat::Grayscale => 4,
    }
}

/// Convert C integer to PixelFormat enum
fn int_to_format(val: c_int) -> PixelFormat {
    match val {
        0 => PixelFormat::Jpeg,
        1 => PixelFormat::Rgb565,
        2 => PixelFormat::Rgb888,
        3 => PixelFormat::Yuv422,
        4 => PixelFormat::Grayscale,
        _ => PixelFormat::Jpeg,
    }
}

// ============================================================================
// Public API Implementation
// ============================================================================

/// Initialize the camera with the given configuration
pub fn camera_initialize(config: CameraConfig) -> CameraResult<()> {
    let format = format_to_int(config.format);
    let resolution = resolution_to_int(config.resolution);
    let quality = config.jpeg_quality as c_int;

    let rc = unsafe { rust_camera_wrapper_init(format, resolution, quality) };

    if rc == 0 {
        Ok(())
    } else if rc == -libc::EALREADY {
        Err(CameraError::AlreadyInitialized)
    } else if rc == -libc::ENOENT || rc == -libc::ENODEV {
        Err(CameraError::DeviceNotFound)
    } else if rc == -libc::ENOTSUP {
        Err(CameraError::NotSupported)
    } else if rc == -libc::ENOMEM {
        Err(CameraError::BufferAllocationFailed)
    } else {
        Err(CameraError::SystemError(-rc))
    }
}

/// Deinitialize the camera
pub fn camera_deinitialize() -> CameraResult<()> {
    let rc = unsafe { rust_camera_wrapper_deinit() };

    if rc == 0 {
        Ok(())
    } else if rc == -libc::ENODEV {
        Err(CameraError::NotInitialized)
    } else {
        Err(CameraError::SystemError(-rc))
    }
}

/// Capture a single frame
///
/// Returns a FrameBuffer containing the captured image data.
/// The frame data is copied to a new Vec, so it's safe to use after this call.
pub fn camera_capture_frame() -> CameraResult<FrameBuffer> {
    let mut width: u32 = 0;
    let mut height: u32 = 0;
    let mut format: c_int = 0;
    let mut len: usize = 0;
    let mut buf: *const u8 = core::ptr::null();

    let rc = unsafe {
        rust_camera_wrapper_capture(&mut width, &mut height, &mut format, &mut len, &mut buf)
    };

    if rc != 0 {
        return if rc == -libc::ENODEV {
            Err(CameraError::NotInitialized)
        } else if rc == -libc::ETIMEDOUT {
            Err(CameraError::Timeout)
        } else {
            Err(CameraError::CaptureFailed)
        };
    }

    if buf.is_null() || len == 0 {
        unsafe { rust_camera_wrapper_return_frame() };
        return Err(CameraError::CaptureFailed);
    }

    // Copy data from C buffer to Rust Vec
    let data = unsafe { core::slice::from_raw_parts(buf, len) }.to_vec();

    // Return the frame buffer to C side
    unsafe { rust_camera_wrapper_return_frame() };

    Ok(FrameBuffer {
        width,
        height,
        format: int_to_format(format),
        data,
        timestamp: 0,
    })
}

/// Get current camera settings
pub fn camera_get_settings() -> CameraResult<CameraSettings> {
    let mut brightness: i8 = 0;
    let mut contrast: i8 = 0;
    let mut saturation: i8 = 0;

    let rc =
        unsafe { rust_camera_wrapper_get_sensor(&mut brightness, &mut contrast, &mut saturation) };

    if rc != 0 {
        return if rc == -libc::ENODEV {
            Err(CameraError::NotInitialized)
        } else {
            Err(CameraError::SystemError(-rc))
        };
    }

    Ok(CameraSettings {
        brightness,
        contrast,
        saturation,
        awb: true, // Assume enabled by default
        awb_gain: true,
        aec: true,
        ae_level: 0,
        agc: true,
        gainceiling: 0,
        hmirror: false,
        vflip: false,
    })
}

/// Set camera settings
pub fn camera_set_settings(settings: CameraSettings) -> CameraResult<()> {
    let rc = unsafe {
        rust_camera_wrapper_set_sensor(
            settings.brightness,
            settings.contrast,
            settings.saturation,
            if settings.hmirror { 1 } else { 0 },
            if settings.vflip { 1 } else { 0 },
        )
    };

    if rc == 0 {
        Ok(())
    } else if rc == -libc::ENODEV {
        Err(CameraError::NotInitialized)
    } else {
        Err(CameraError::SystemError(-rc))
    }
}

/// Check if camera is initialized
pub fn camera_is_initialized() -> bool {
    unsafe { rust_camera_wrapper_is_initialized() != 0 }
}
