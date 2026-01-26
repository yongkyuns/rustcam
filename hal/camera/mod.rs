//! Camera HAL
//!
//! Provides camera functionality for capturing images and video frames.
//! Implementation is selected at compile time based on platform feature.
//!
//! - Linux: Uses V4L2 API for webcam capture
//! - NuttX ESP32S3: Uses ESP-IDF esp_camera library via C wrapper

// Platform-specific implementations

// NuttX uses ESP-IDF esp_camera via C wrapper
#[cfg(feature = "platform-nuttx")]
mod nuttx;
#[cfg(feature = "platform-nuttx")]
pub use nuttx::*;

// Linux uses V4L2 API
#[cfg(feature = "platform-linux")]
mod linux;
#[cfg(feature = "platform-linux")]
pub use linux::*;

// Fallback stub for other platforms
#[cfg(not(any(feature = "platform-linux", feature = "platform-nuttx")))]
mod none;
#[cfg(not(any(feature = "platform-linux", feature = "platform-nuttx")))]
pub use none::*;

use core::fmt;

/// Camera operation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraError {
    /// Camera not initialized
    NotInitialized,
    /// Already initialized
    AlreadyInitialized,
    /// Camera device not found
    DeviceNotFound,
    /// Failed to open device
    OpenFailed,
    /// Failed to configure camera
    ConfigurationFailed,
    /// Failed to capture frame
    CaptureFailed,
    /// Invalid format or resolution
    InvalidFormat,
    /// Buffer allocation failed
    BufferAllocationFailed,
    /// Timeout waiting for frame
    Timeout,
    /// Operation not supported on this platform
    NotSupported,
    /// System error with errno
    SystemError(i32),
}

impl fmt::Display for CameraError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CameraError::NotInitialized => write!(f, "Camera not initialized"),
            CameraError::AlreadyInitialized => write!(f, "Camera already initialized"),
            CameraError::DeviceNotFound => write!(f, "Camera device not found"),
            CameraError::OpenFailed => write!(f, "Failed to open camera device"),
            CameraError::ConfigurationFailed => write!(f, "Failed to configure camera"),
            CameraError::CaptureFailed => write!(f, "Failed to capture frame"),
            CameraError::InvalidFormat => write!(f, "Invalid format or resolution"),
            CameraError::BufferAllocationFailed => write!(f, "Buffer allocation failed"),
            CameraError::Timeout => write!(f, "Timeout waiting for frame"),
            CameraError::NotSupported => write!(f, "Not supported on this platform"),
            CameraError::SystemError(e) => write!(f, "System error: {}", e),
        }
    }
}

/// Result type for camera operations
pub type CameraResult<T> = Result<T, CameraError>;

/// Pixel format for camera frames
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum PixelFormat {
    /// JPEG compressed format (most efficient for ESP32-CAM)
    #[default]
    Jpeg = 0,
    /// RGB565 (16-bit, 5-6-5 format)
    Rgb565 = 1,
    /// RGB888 (24-bit, 8-8-8 format)
    Rgb888 = 2,
    /// YUV422 format
    Yuv422 = 3,
    /// Grayscale (8-bit)
    Grayscale = 4,
}

impl fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PixelFormat::Jpeg => write!(f, "JPEG"),
            PixelFormat::Rgb565 => write!(f, "RGB565"),
            PixelFormat::Rgb888 => write!(f, "RGB888"),
            PixelFormat::Yuv422 => write!(f, "YUV422"),
            PixelFormat::Grayscale => write!(f, "Grayscale"),
        }
    }
}

/// Camera resolution presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Resolution {
    /// 160x120
    Qqvga = 0,
    /// 176x144
    Qcif = 1,
    /// 240x176
    Hqvga = 2,
    /// 320x240
    Qvga = 3,
    /// 400x296
    Cif = 4,
    /// 480x320
    Hvga = 5,
    /// 640x480 (default, good balance)
    #[default]
    Vga = 6,
    /// 800x600
    Svga = 7,
    /// 1024x768
    Xga = 8,
    /// 1280x720
    Hd = 9,
    /// 1280x1024
    Sxga = 10,
    /// 1600x1200
    Uxga = 11,
}

impl Resolution {
    /// Get width for this resolution
    pub fn width(&self) -> u32 {
        match self {
            Resolution::Qqvga => 160,
            Resolution::Qcif => 176,
            Resolution::Hqvga => 240,
            Resolution::Qvga => 320,
            Resolution::Cif => 400,
            Resolution::Hvga => 480,
            Resolution::Vga => 640,
            Resolution::Svga => 800,
            Resolution::Xga => 1024,
            Resolution::Hd => 1280,
            Resolution::Sxga => 1280,
            Resolution::Uxga => 1600,
        }
    }

    /// Get height for this resolution
    pub fn height(&self) -> u32 {
        match self {
            Resolution::Qqvga => 120,
            Resolution::Qcif => 144,
            Resolution::Hqvga => 176,
            Resolution::Qvga => 240,
            Resolution::Cif => 296,
            Resolution::Hvga => 320,
            Resolution::Vga => 480,
            Resolution::Svga => 600,
            Resolution::Xga => 768,
            Resolution::Hd => 720,
            Resolution::Sxga => 1024,
            Resolution::Uxga => 1200,
        }
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width(), self.height())
    }
}

/// Camera configuration
#[derive(Debug, Clone, Copy)]
pub struct CameraConfig {
    /// Pixel format
    pub format: PixelFormat,
    /// Resolution
    pub resolution: Resolution,
    /// JPEG quality (1-100, only used for JPEG format)
    pub jpeg_quality: u8,
    /// Frame buffer count (for double/triple buffering)
    pub fb_count: u8,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            format: PixelFormat::Jpeg,
            resolution: Resolution::Vga,
            jpeg_quality: 12,  // ESP32-CAM default
            fb_count: 1,
        }
    }
}

impl CameraConfig {
    /// Create a new camera configuration
    pub fn new(format: PixelFormat, resolution: Resolution) -> Self {
        Self {
            format,
            resolution,
            jpeg_quality: 12,
            fb_count: 1,
        }
    }

    /// Set JPEG quality (1-100, lower = higher compression)
    pub fn with_jpeg_quality(mut self, quality: u8) -> Self {
        self.jpeg_quality = quality.clamp(1, 100);
        self
    }

    /// Set frame buffer count
    pub fn with_fb_count(mut self, count: u8) -> Self {
        self.fb_count = count.clamp(1, 3);
        self
    }
}

/// Captured frame buffer
#[derive(Debug, Clone)]
pub struct FrameBuffer {
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Pixel format
    pub format: PixelFormat,
    /// Frame data
    pub data: Vec<u8>,
    /// Timestamp in microseconds (if available)
    pub timestamp: u64,
}

impl FrameBuffer {
    /// Create a new frame buffer
    pub fn new(width: u32, height: u32, format: PixelFormat, data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            format,
            data,
            timestamp: 0,
        }
    }

    /// Get the size of the frame data in bytes
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the frame buffer is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Camera sensor settings (adjustable parameters)
#[derive(Debug, Clone, Copy, Default)]
pub struct CameraSettings {
    /// Brightness (-2 to 2)
    pub brightness: i8,
    /// Contrast (-2 to 2)
    pub contrast: i8,
    /// Saturation (-2 to 2)
    pub saturation: i8,
    /// Auto White Balance enabled
    pub awb: bool,
    /// Auto White Balance gain enabled
    pub awb_gain: bool,
    /// Auto Exposure Control enabled
    pub aec: bool,
    /// Auto Exposure Control level (-2 to 2)
    pub ae_level: i8,
    /// Auto Gain Control enabled
    pub agc: bool,
    /// Gain ceiling (0-6)
    pub gainceiling: u8,
    /// Horizontal mirror
    pub hmirror: bool,
    /// Vertical flip
    pub vflip: bool,
}

impl CameraSettings {
    /// Create default settings with auto features enabled
    pub fn auto() -> Self {
        Self {
            brightness: 0,
            contrast: 0,
            saturation: 0,
            awb: true,
            awb_gain: true,
            aec: true,
            ae_level: 0,
            agc: true,
            gainceiling: 0,
            hmirror: false,
            vflip: false,
        }
    }
}
