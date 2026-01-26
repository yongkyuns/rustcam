//! Linux Camera implementation using V4L2 API
//!
//! Uses V4L2 (Video for Linux 2) API with memory-mapped buffers for
//! efficient webcam capture on Linux systems.

use super::{
    CameraConfig, CameraError, CameraResult, CameraSettings, FrameBuffer, PixelFormat,
};
use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::sync::Mutex;

// ============================================================================
// V4L2 Constants and Structures
// ============================================================================

// V4L2 ioctl commands
const VIDIOC_QUERYCAP: libc::c_ulong = 0x80685600;
const VIDIOC_S_FMT: libc::c_ulong = 0xC0D05605;
const VIDIOC_G_FMT: libc::c_ulong = 0xC0D05604;
const VIDIOC_REQBUFS: libc::c_ulong = 0xC0145608;
const VIDIOC_QUERYBUF: libc::c_ulong = 0xC0585609;
const VIDIOC_QBUF: libc::c_ulong = 0xC058560F;
const VIDIOC_DQBUF: libc::c_ulong = 0xC0585611;
const VIDIOC_STREAMON: libc::c_ulong = 0x40045612;
const VIDIOC_STREAMOFF: libc::c_ulong = 0x40045613;
const VIDIOC_G_CTRL: libc::c_ulong = 0xC008561B;
const VIDIOC_S_CTRL: libc::c_ulong = 0xC008561C;

// V4L2 pixel formats
const V4L2_PIX_FMT_MJPEG: u32 = 0x47504A4D; // 'MJPG'
const V4L2_PIX_FMT_JPEG: u32 = 0x4745504A; // 'JPEG'
const V4L2_PIX_FMT_YUYV: u32 = 0x56595559; // 'YUYV'
const V4L2_PIX_FMT_RGB565: u32 = 0x50424752; // 'RGBP'
const V4L2_PIX_FMT_RGB24: u32 = 0x33424752; // 'RGB3'
const V4L2_PIX_FMT_GREY: u32 = 0x59455247; // 'GREY'
const V4L2_PIX_FMT_NV12: u32 = 0x3231564E; // 'NV12'

// V4L2 buffer types and memory types
const V4L2_BUF_TYPE_VIDEO_CAPTURE: u32 = 1;
const V4L2_MEMORY_MMAP: u32 = 1;
const V4L2_FIELD_ANY: u32 = 0;

// V4L2 control IDs
const V4L2_CID_BRIGHTNESS: u32 = 0x00980900;
const V4L2_CID_CONTRAST: u32 = 0x00980901;
const V4L2_CID_SATURATION: u32 = 0x00980902;
const V4L2_CID_HFLIP: u32 = 0x00980914;
const V4L2_CID_VFLIP: u32 = 0x00980915;

// Buffer count
const BUFFER_COUNT: usize = 4;

// ============================================================================
// V4L2 Structures (simplified, matching kernel ABI)
// ============================================================================

#[repr(C)]
struct V4l2Capability {
    driver: [u8; 16],
    card: [u8; 32],
    bus_info: [u8; 32],
    version: u32,
    capabilities: u32,
    device_caps: u32,
    reserved: [u32; 3],
}

#[repr(C)]
struct V4l2PixFormat {
    width: u32,
    height: u32,
    pixelformat: u32,
    field: u32,
    bytesperline: u32,
    sizeimage: u32,
    colorspace: u32,
    priv_: u32,
    flags: u32,
    enc: u32,
    quantization: u32,
    xfer_func: u32,
}

#[repr(C)]
struct V4l2Format {
    type_: u32,
    fmt: V4l2FormatUnion,
}

#[repr(C)]
union V4l2FormatUnion {
    pix: std::mem::ManuallyDrop<V4l2PixFormat>,
    raw_data: [u8; 200],
}

#[repr(C)]
struct V4l2RequestBuffers {
    count: u32,
    type_: u32,
    memory: u32,
    capabilities: u32,
    flags: u8,
    reserved: [u8; 3],
}

#[repr(C)]
struct V4l2Timecode {
    type_: u32,
    flags: u32,
    frames: u8,
    seconds: u8,
    minutes: u8,
    hours: u8,
    userbits: [u8; 4],
}

#[repr(C)]
struct V4l2Buffer {
    index: u32,        // offset 0
    type_: u32,        // offset 4
    bytesused: u32,    // offset 8
    flags: u32,        // offset 12
    field: u32,        // offset 16
    _pad1: u32,        // offset 20 (padding for alignment)
    timestamp: libc::timeval, // offset 24 (16 bytes)
    timecode: V4l2Timecode,   // offset 40 (16 bytes)
    sequence: u32,     // offset 56
    memory: u32,       // offset 60
    m: V4l2BufferUnion, // offset 64 (8 bytes)
    length: u32,       // offset 72
    reserved2: u32,    // offset 76
    request_fd: i32,   // offset 80
    reserved: u32,     // offset 84 (to make total 88 bytes)
}

#[repr(C)]
union V4l2BufferUnion {
    offset: u32,
    userptr: libc::c_ulong,
    planes: *mut libc::c_void,
    fd: i32,
}

#[repr(C)]
struct V4l2Control {
    id: u32,
    value: i32,
}

// ============================================================================
// Camera State
// ============================================================================

struct MappedBuffer {
    ptr: *mut libc::c_void,
    length: usize,
}

unsafe impl Send for MappedBuffer {}
unsafe impl Sync for MappedBuffer {}

struct CameraState {
    file: Option<File>,
    buffers: Vec<MappedBuffer>,
    streaming: bool,
    width: u32,
    height: u32,
    format: PixelFormat,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            file: None,
            buffers: Vec::new(),
            streaming: false,
            width: 640,
            height: 480,
            format: PixelFormat::Jpeg,
        }
    }
}

static CAMERA_STATE: Mutex<CameraState> = Mutex::new(CameraState {
    file: None,
    buffers: Vec::new(),
    streaming: false,
    width: 640,
    height: 480,
    format: PixelFormat::Jpeg,
});

// ============================================================================
// Helper Functions
// ============================================================================

fn pixel_format_to_v4l2(fmt: PixelFormat) -> u32 {
    match fmt {
        PixelFormat::Jpeg => V4L2_PIX_FMT_MJPEG,
        PixelFormat::Rgb565 => V4L2_PIX_FMT_RGB565,
        PixelFormat::Rgb888 => V4L2_PIX_FMT_RGB24,
        PixelFormat::Yuv422 => V4L2_PIX_FMT_YUYV,
        PixelFormat::Grayscale => V4L2_PIX_FMT_GREY,
    }
}

fn v4l2_to_pixel_format(v4l2_fmt: u32) -> PixelFormat {
    match v4l2_fmt {
        V4L2_PIX_FMT_MJPEG | V4L2_PIX_FMT_JPEG => PixelFormat::Jpeg,
        V4L2_PIX_FMT_RGB565 => PixelFormat::Rgb565,
        V4L2_PIX_FMT_RGB24 => PixelFormat::Rgb888,
        V4L2_PIX_FMT_YUYV | V4L2_PIX_FMT_NV12 => PixelFormat::Yuv422,
        V4L2_PIX_FMT_GREY => PixelFormat::Grayscale,
        _ => PixelFormat::Yuv422, // Default to YUV for unknown formats
    }
}

unsafe fn ioctl<T>(fd: i32, request: libc::c_ulong, arg: *mut T) -> i32 {
    libc::ioctl(fd, request, arg)
}

fn find_camera_device() -> Option<String> {
    // Try video devices, checking if they support capture
    for i in 0..10 {
        let path = format!("/dev/video{}", i);
        if std::path::Path::new(&path).exists() {
            // Try to open and check if it's a capture device
            if let Ok(file) = OpenOptions::new().read(true).write(true).open(&path) {
                let fd = file.as_raw_fd();
                let mut cap: V4l2Capability = unsafe { std::mem::zeroed() };
                if unsafe { ioctl(fd, VIDIOC_QUERYCAP, &mut cap) } >= 0 {
                    // Check if device supports video capture (capability bit 0x1)
                    if cap.capabilities & 0x1 != 0 {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn unmap_buffers(buffers: &mut Vec<MappedBuffer>) {
    for buf in buffers.drain(..) {
        if !buf.ptr.is_null() {
            unsafe {
                libc::munmap(buf.ptr, buf.length);
            }
        }
    }
}

// ============================================================================
// Public API Implementation
// ============================================================================

/// Initialize the camera with the given configuration
pub fn camera_initialize(config: CameraConfig) -> CameraResult<()> {
    let mut state = CAMERA_STATE.lock().unwrap();

    if state.file.is_some() {
        return Err(CameraError::AlreadyInitialized);
    }

    // Find and open camera device
    let device_path = find_camera_device().ok_or(CameraError::DeviceNotFound)?;

    // Open with O_NONBLOCK for proper select() support
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&device_path)
        .map_err(|_| CameraError::OpenFailed)?;

    let fd = file.as_raw_fd();

    // Query capabilities
    let mut cap: V4l2Capability = unsafe { std::mem::zeroed() };
    if unsafe { ioctl(fd, VIDIOC_QUERYCAP, &mut cap) } < 0 {
        return Err(CameraError::DeviceNotFound);
    }

    // First get current format from the device
    let mut fmt: V4l2Format = unsafe { std::mem::zeroed() };
    fmt.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    if unsafe { ioctl(fd, VIDIOC_G_FMT, &mut fmt) } < 0 {
        return Err(CameraError::ConfigurationFailed);
    }

    // Try to set requested format - attempt multiple formats as fallback
    let formats_to_try = [
        pixel_format_to_v4l2(config.format),
        V4L2_PIX_FMT_NV12,   // NV12 (Intel cameras, v4l2loopback)
        V4L2_PIX_FMT_YUYV,   // Common fallback
        V4L2_PIX_FMT_MJPEG,  // MJPEG
        V4L2_PIX_FMT_RGB24,  // RGB24
    ];

    let mut format_set = false;

    for pixfmt in formats_to_try {
        let mut try_fmt: V4l2Format = unsafe { std::mem::zeroed() };
        try_fmt.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        try_fmt.fmt.pix = std::mem::ManuallyDrop::new(V4l2PixFormat {
            width: config.resolution.width(),
            height: config.resolution.height(),
            pixelformat: pixfmt,
            field: V4L2_FIELD_ANY,
            bytesperline: 0,
            sizeimage: 0,
            colorspace: 0,
            priv_: 0,
            flags: 0,
            enc: 0,
            quantization: 0,
            xfer_func: 0,
        });

        if unsafe { ioctl(fd, VIDIOC_S_FMT, &mut try_fmt) } >= 0 {
            fmt = try_fmt;
            format_set = true;
            break;
        }
    }

    // If no format could be set, use device's current format (e.g., v4l2loopback)
    if !format_set {
        // Re-get current format as fallback
        fmt.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        if unsafe { ioctl(fd, VIDIOC_G_FMT, &mut fmt) } < 0 {
            return Err(CameraError::ConfigurationFailed);
        }
    }

    // Get actual format (driver may have changed it)
    if unsafe { ioctl(fd, VIDIOC_G_FMT, &mut fmt) } < 0 {
        return Err(CameraError::ConfigurationFailed);
    }

    let (actual_width, actual_height, actual_pixfmt) = unsafe {
        (fmt.fmt.pix.width, fmt.fmt.pix.height, fmt.fmt.pix.pixelformat)
    };

    // Request buffers
    let mut req: V4l2RequestBuffers = unsafe { std::mem::zeroed() };
    req.count = BUFFER_COUNT as u32;
    req.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    req.memory = V4L2_MEMORY_MMAP;

    if unsafe { ioctl(fd, VIDIOC_REQBUFS, &mut req) } < 0 {
        return Err(CameraError::BufferAllocationFailed);
    }

    // Map buffers
    let mut buffers = Vec::with_capacity(req.count as usize);
    for i in 0..req.count {
        let mut buf: V4l2Buffer = unsafe { std::mem::zeroed() };
        buf.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        buf.memory = V4L2_MEMORY_MMAP;
        buf.index = i;

        if unsafe { ioctl(fd, VIDIOC_QUERYBUF, &mut buf) } < 0 {
            unmap_buffers(&mut buffers);
            return Err(CameraError::BufferAllocationFailed);
        }

        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                buf.length as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                buf.m.offset as libc::off_t,
            )
        };

        if ptr == libc::MAP_FAILED {
            unmap_buffers(&mut buffers);
            return Err(CameraError::BufferAllocationFailed);
        }

        buffers.push(MappedBuffer {
            ptr,
            length: buf.length as usize,
        });
    }

    // Queue all buffers
    for i in 0..buffers.len() {
        let mut buf: V4l2Buffer = unsafe { std::mem::zeroed() };
        buf.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        buf.memory = V4L2_MEMORY_MMAP;
        buf.index = i as u32;

        if unsafe { ioctl(fd, VIDIOC_QBUF, &mut buf) } < 0 {
            unmap_buffers(&mut buffers);
            return Err(CameraError::ConfigurationFailed);
        }
    }

    // Start streaming
    let buf_type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    if unsafe { ioctl(fd, VIDIOC_STREAMON, &buf_type as *const u32 as *mut u32) } < 0 {
        unmap_buffers(&mut buffers);
        return Err(CameraError::ConfigurationFailed);
    }

    state.file = Some(file);
    state.buffers = buffers;
    state.streaming = true;
    state.width = actual_width;
    state.height = actual_height;
    state.format = v4l2_to_pixel_format(actual_pixfmt);

    Ok(())
}

/// Deinitialize the camera
pub fn camera_deinitialize() -> CameraResult<()> {
    let mut state = CAMERA_STATE.lock().unwrap();

    if state.file.is_none() {
        return Err(CameraError::NotInitialized);
    }

    let fd = state.file.as_ref().unwrap().as_raw_fd();

    // Stop streaming
    if state.streaming {
        let buf_type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        unsafe { ioctl(fd, VIDIOC_STREAMOFF, &buf_type as *const u32 as *mut u32) };
        state.streaming = false;
    }

    // Unmap buffers
    unmap_buffers(&mut state.buffers);

    // Close device
    state.file = None;

    Ok(())
}

/// Capture a single frame
pub fn camera_capture_frame() -> CameraResult<FrameBuffer> {
    let state = CAMERA_STATE.lock().unwrap();

    let file = state.file.as_ref().ok_or(CameraError::NotInitialized)?;
    let fd = file.as_raw_fd();

    // Wait for frame data using select() with timeout
    let mut retries = 10;
    loop {
        unsafe {
            let mut fds: libc::fd_set = std::mem::zeroed();
            libc::FD_ZERO(&mut fds);
            libc::FD_SET(fd, &mut fds);

            let mut tv = libc::timeval {
                tv_sec: 1,
                tv_usec: 0,
            };

            let ret = libc::select(fd + 1, &mut fds, std::ptr::null_mut(), std::ptr::null_mut(), &mut tv);
            if ret < 0 {
                let errno = *libc::__errno_location();
                if errno == libc::EINTR {
                    continue;
                }
                return Err(CameraError::CaptureFailed);
            }
            if ret == 0 {
                retries -= 1;
                if retries == 0 {
                    return Err(CameraError::Timeout);
                }
                continue;
            }
            break;
        }
    }

    // Dequeue a buffer
    let mut buf: V4l2Buffer = unsafe { std::mem::zeroed() };
    buf.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    buf.memory = V4L2_MEMORY_MMAP;

    if unsafe { ioctl(fd, VIDIOC_DQBUF, &mut buf) } < 0 {
        let errno = unsafe { *libc::__errno_location() };
        if errno == libc::EAGAIN {
            return Err(CameraError::Timeout);
        }
        return Err(CameraError::CaptureFailed);
    }

    let buffer_index = buf.index as usize;
    let bytes_used = buf.bytesused as usize;

    if buffer_index >= state.buffers.len() {
        // Re-queue the buffer even on error
        unsafe { ioctl(fd, VIDIOC_QBUF, &mut buf) };
        return Err(CameraError::CaptureFailed);
    }

    // Copy data from mapped buffer
    let mapped_buf = &state.buffers[buffer_index];
    let data = unsafe { std::slice::from_raw_parts(mapped_buf.ptr as *const u8, bytes_used) };
    let data_vec = data.to_vec();

    // Calculate timestamp
    let timestamp = (buf.timestamp.tv_sec as u64) * 1_000_000 + (buf.timestamp.tv_usec as u64);

    // Re-queue the buffer - reset required fields
    buf.bytesused = 0;
    buf.flags = 0;
    if unsafe { ioctl(fd, VIDIOC_QBUF, &mut buf) } < 0 {
        // Log but don't fail - we already have the frame
    }

    Ok(FrameBuffer {
        width: state.width,
        height: state.height,
        format: state.format,
        data: data_vec,
        timestamp,
    })
}

/// Get current camera settings
pub fn camera_get_settings() -> CameraResult<CameraSettings> {
    let state = CAMERA_STATE.lock().unwrap();

    let file = state.file.as_ref().ok_or(CameraError::NotInitialized)?;
    let fd = file.as_raw_fd();

    let mut settings = CameraSettings::auto();

    // Try to get brightness
    let mut ctrl = V4l2Control {
        id: V4L2_CID_BRIGHTNESS,
        value: 0,
    };
    if unsafe { ioctl(fd, VIDIOC_G_CTRL, &mut ctrl) } >= 0 {
        settings.brightness = ctrl.value.clamp(-128, 127) as i8;
    }

    // Try to get contrast
    ctrl.id = V4L2_CID_CONTRAST;
    ctrl.value = 0;
    if unsafe { ioctl(fd, VIDIOC_G_CTRL, &mut ctrl) } >= 0 {
        settings.contrast = ctrl.value.clamp(-128, 127) as i8;
    }

    // Try to get saturation
    ctrl.id = V4L2_CID_SATURATION;
    ctrl.value = 0;
    if unsafe { ioctl(fd, VIDIOC_G_CTRL, &mut ctrl) } >= 0 {
        settings.saturation = ctrl.value.clamp(-128, 127) as i8;
    }

    // Try to get hflip
    ctrl.id = V4L2_CID_HFLIP;
    ctrl.value = 0;
    if unsafe { ioctl(fd, VIDIOC_G_CTRL, &mut ctrl) } >= 0 {
        settings.hmirror = ctrl.value != 0;
    }

    // Try to get vflip
    ctrl.id = V4L2_CID_VFLIP;
    ctrl.value = 0;
    if unsafe { ioctl(fd, VIDIOC_G_CTRL, &mut ctrl) } >= 0 {
        settings.vflip = ctrl.value != 0;
    }

    Ok(settings)
}

/// Set camera settings
pub fn camera_set_settings(settings: CameraSettings) -> CameraResult<()> {
    let state = CAMERA_STATE.lock().unwrap();

    let file = state.file.as_ref().ok_or(CameraError::NotInitialized)?;
    let fd = file.as_raw_fd();

    // Set brightness
    let mut ctrl = V4l2Control {
        id: V4L2_CID_BRIGHTNESS,
        value: settings.brightness as i32,
    };
    unsafe { ioctl(fd, VIDIOC_S_CTRL, &mut ctrl) };

    // Set contrast
    ctrl.id = V4L2_CID_CONTRAST;
    ctrl.value = settings.contrast as i32;
    unsafe { ioctl(fd, VIDIOC_S_CTRL, &mut ctrl) };

    // Set saturation
    ctrl.id = V4L2_CID_SATURATION;
    ctrl.value = settings.saturation as i32;
    unsafe { ioctl(fd, VIDIOC_S_CTRL, &mut ctrl) };

    // Set hflip
    ctrl.id = V4L2_CID_HFLIP;
    ctrl.value = if settings.hmirror { 1 } else { 0 };
    unsafe { ioctl(fd, VIDIOC_S_CTRL, &mut ctrl) };

    // Set vflip
    ctrl.id = V4L2_CID_VFLIP;
    ctrl.value = if settings.vflip { 1 } else { 0 };
    unsafe { ioctl(fd, VIDIOC_S_CTRL, &mut ctrl) };

    Ok(())
}

/// Check if camera is initialized
pub fn camera_is_initialized() -> bool {
    let state = CAMERA_STATE.lock().unwrap();
    state.file.is_some()
}
