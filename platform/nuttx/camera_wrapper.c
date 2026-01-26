/****************************************************************************
 * Camera Wrapper for NuttX
 *
 * This wrapper provides camera capture using NuttX V4L2 API.
 *
 * For ESP32-S3, a proper camera driver needs to be enabled in NuttX kernel
 * that exposes /dev/video device. Without a camera driver, this will return
 * -ENOTSUP.
 *
 * Currently, ESP32-S3 camera support requires:
 * 1. A kernel-space camera driver (not yet available in mainline NuttX)
 * 2. CONFIG_VIDEO and CONFIG_VIDEO_STREAM enabled
 *
 * This file provides stubs that return appropriate errors until camera
 * driver support is available.
 ****************************************************************************/

#include <nuttx/config.h>

#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdlib.h>

/****************************************************************************
 * Pre-processor Definitions
 ****************************************************************************/

#define CAMERA_DEV_PATH      "/dev/video0"
#define CAMERA_BUFFER_SIZE   (320 * 240 * 2)  /* Default QVGA RGB565 or JPEG */

/* Pixel format codes matching Rust enum */
#define PIXFMT_JPEG       0
#define PIXFMT_RGB565     1
#define PIXFMT_RGB888     2
#define PIXFMT_YUV422     3
#define PIXFMT_GRAYSCALE  4

/****************************************************************************
 * Private Data
 ****************************************************************************/

static volatile int g_camera_initialized = 0;
static int g_camera_fd = -1;
static uint8_t *g_frame_buffer = NULL;
static size_t g_frame_buffer_size = 0;
static size_t g_frame_len = 0;
static int g_width = 320;
static int g_height = 240;
static int g_format = PIXFMT_JPEG;

/****************************************************************************
 * Public Functions (FFI Interface)
 ****************************************************************************/

/****************************************************************************
 * Name: rust_camera_wrapper_init
 *
 * Description:
 *   Initialize camera subsystem.
 *
 * Parameters:
 *   format     - Pixel format (0=JPEG, 1=RGB565, etc.)
 *   resolution - Resolution enum (0=QQVGA, 6=VGA, etc.)
 *   quality    - JPEG quality (1-100, only for JPEG)
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_camera_wrapper_init(int format, int resolution, int quality)
{
  (void)quality;

  if (g_camera_initialized)
    {
      return -EALREADY;
    }

  printf("[CAM] Initializing camera...\n");

  /* Try to open the camera device */
  g_camera_fd = open(CAMERA_DEV_PATH, O_RDWR);
  if (g_camera_fd < 0)
    {
      int err = errno;
      printf("[CAM] Failed to open %s: %d\n", CAMERA_DEV_PATH, err);

      if (err == ENOENT)
        {
          printf("[CAM] Camera driver not available.\n");
          printf("[CAM] ESP32-S3 camera requires kernel driver support.\n");
          printf("[CAM] Enable CONFIG_VIDEO and camera driver in NuttX.\n");
        }

      return -err;
    }

  /* Set format and resolution - use resolution enum for sizing */
  g_format = format;
  switch (resolution)
    {
      case 0: g_width = 160;  g_height = 120;  break;  /* QQVGA */
      case 1: g_width = 176;  g_height = 144;  break;  /* QCIF */
      case 2: g_width = 240;  g_height = 176;  break;  /* HQVGA */
      case 3: g_width = 320;  g_height = 240;  break;  /* QVGA */
      case 4: g_width = 400;  g_height = 296;  break;  /* CIF */
      case 5: g_width = 480;  g_height = 320;  break;  /* HVGA */
      case 6: g_width = 640;  g_height = 480;  break;  /* VGA */
      case 7: g_width = 800;  g_height = 600;  break;  /* SVGA */
      case 8: g_width = 1024; g_height = 768;  break;  /* XGA */
      default: g_width = 320; g_height = 240;  break;  /* Default QVGA */
    }

  /* Allocate frame buffer */
  g_frame_buffer_size = g_width * g_height * 2;  /* RGB565 or compressed JPEG */
  if (format == PIXFMT_JPEG)
    {
      /* JPEG typically smaller */
      g_frame_buffer_size = g_width * g_height / 2;
    }

  g_frame_buffer = (uint8_t *)malloc(g_frame_buffer_size);
  if (!g_frame_buffer)
    {
      printf("[CAM] Failed to allocate frame buffer (%d bytes)\n",
             (int)g_frame_buffer_size);
      close(g_camera_fd);
      g_camera_fd = -1;
      return -ENOMEM;
    }

  g_camera_initialized = 1;
  printf("[CAM] Camera initialized successfully\n");

  return 0;
}

/****************************************************************************
 * Name: rust_camera_wrapper_deinit
 *
 * Description:
 *   Deinitialize camera subsystem.
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_camera_wrapper_deinit(void)
{
  if (!g_camera_initialized)
    {
      return -ENODEV;
    }

  printf("[CAM] Deinitializing camera...\n");

  if (g_camera_fd >= 0)
    {
      close(g_camera_fd);
      g_camera_fd = -1;
    }

  if (g_frame_buffer)
    {
      free(g_frame_buffer);
      g_frame_buffer = NULL;
    }

  g_frame_buffer_size = 0;
  g_frame_len = 0;
  g_camera_initialized = 0;
  printf("[CAM] Camera deinitialized\n");

  return 0;
}

/****************************************************************************
 * Name: rust_camera_wrapper_capture
 *
 * Description:
 *   Capture a single frame.
 *
 * Parameters:
 *   width  - Output: frame width
 *   height - Output: frame height
 *   format - Output: pixel format
 *   len    - Output: data length in bytes
 *   buf    - Output: pointer to frame data
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_camera_wrapper_capture(uint32_t *width, uint32_t *height,
                                 int *format, size_t *len,
                                 const uint8_t **buf)
{
  ssize_t ret;

  if (!g_camera_initialized)
    {
      return -ENODEV;
    }

  if (!width || !height || !format || !len || !buf)
    {
      return -EINVAL;
    }

  if (g_camera_fd < 0 || !g_frame_buffer)
    {
      return -ENODEV;
    }

  /* Read frame from camera device */
  ret = read(g_camera_fd, g_frame_buffer, g_frame_buffer_size);
  if (ret < 0)
    {
      printf("[CAM] Capture failed: %d\n", errno);
      return -errno;
    }

  if (ret == 0)
    {
      printf("[CAM] No data captured\n");
      return -EIO;
    }

  /* Store frame info */
  g_frame_len = (size_t)ret;
  *width = g_width;
  *height = g_height;
  *format = g_format;
  *len = g_frame_len;
  *buf = g_frame_buffer;

  printf("[CAM] Captured %d bytes\n", (int)g_frame_len);

  return 0;
}

/****************************************************************************
 * Name: rust_camera_wrapper_return_frame
 *
 * Description:
 *   Return the frame buffer after processing.
 *   Must be called after each successful capture.
 ****************************************************************************/

void rust_camera_wrapper_return_frame(void)
{
  /* Nothing to do */
}

/****************************************************************************
 * Name: rust_camera_wrapper_is_initialized
 *
 * Description:
 *   Check if camera is initialized.
 *
 * Returns:
 *   1 if initialized, 0 otherwise
 ****************************************************************************/

int rust_camera_wrapper_is_initialized(void)
{
  return g_camera_initialized ? 1 : 0;
}

/****************************************************************************
 * Name: rust_camera_wrapper_get_sensor
 *
 * Description:
 *   Get camera sensor settings.
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_camera_wrapper_get_sensor(int8_t *brightness, int8_t *contrast,
                                    int8_t *saturation)
{
  if (!g_camera_initialized)
    {
      return -ENODEV;
    }

  if (!brightness || !contrast || !saturation)
    {
      return -EINVAL;
    }

  *brightness = 0;
  *contrast = 0;
  *saturation = 0;

  return 0;
}

/****************************************************************************
 * Name: rust_camera_wrapper_set_sensor
 *
 * Description:
 *   Set camera sensor settings.
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_camera_wrapper_set_sensor(int8_t brightness, int8_t contrast,
                                    int8_t saturation, int hmirror, int vflip)
{
  if (!g_camera_initialized)
    {
      return -ENODEV;
    }

  (void)brightness;
  (void)contrast;
  (void)saturation;
  (void)hmirror;
  (void)vflip;

  return 0;
}
