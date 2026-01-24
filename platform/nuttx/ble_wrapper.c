/****************************************************************************
 * BLE Wrapper for Rust Integration
 *
 * This wrapper provides a simple C interface to NuttX BLE that can be called
 * from Rust FFI. It supports both NimBLE and NuttX native BLE backends.
 ****************************************************************************/

#include <nuttx/config.h>

#include <stdio.h>
#include <stdint.h>

/****************************************************************************
 * Debug helper for Rust FFI
 ****************************************************************************/

void rust_debug_print(const char *msg)
{
    if (msg) {
        printf("[RUST-DBG] %s\n", msg);
    }
}

/****************************************************************************
 * C wrapper entry point for debugging
 * This will be called by NuttX before the Rust code
 ****************************************************************************/

/* Forward declaration of the Rust entry point */
extern int rust_rustcam_main(int argc, char *argv[]);

int rustcam_main(int argc, char *argv[])
{
    printf("[C-DBG] rustcam_main C wrapper entered\n");
    printf("[C-DBG] About to call Rust entry point\n");

    int result = rust_rustcam_main(argc, argv);

    printf("[C-DBG] Rust returned: %d\n", result);
    return result;
}
#include <string.h>
#include <errno.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/ioctl.h>

#ifdef CONFIG_NIMBLE

#include <pthread.h>
#include <assert.h>

#include <net/if.h>

#include "nimble/ble.h"
#include "nimble/nimble_port.h"
#include "host/ble_hs.h"
#include "host/ble_gatt.h"
#include "host/util/util.h"
#include "services/gap/ble_svc_gap.h"
#include "services/gatt/ble_svc_gatt.h"
#include "os/os_mbuf.h"

/* State */
static volatile int g_ble_initialized = 0;
static volatile int g_ble_host_synced = 0;
static volatile int g_ble_advertising = 0;
static volatile int g_ble_connected = 0;
static volatile uint16_t g_conn_handle = 0;
static uint8_t g_own_addr_type = 0;
static pthread_t g_host_thread;
static pthread_t g_hci_thread;

/* External HCI socket handler */
extern void ble_hci_sock_ack_handler(void *param);
extern void ble_hci_sock_set_device(int dev);

/* Device name */
static char g_device_name[32] = "RustCam";

/* Pending advertising request */
static volatile int g_pending_adv = 0;

/* GATT command buffer - stores last received command */
static uint8_t g_gatt_command[64];
static volatile uint8_t g_gatt_command_len = 0;

/* GATT read response message */
static const char *g_gatt_read_msg = "Hello from RustCam!";

/* Custom GATT service UUIDs (matching unix.rs) */
/* Service UUID: 0x1234 */
/* Read characteristic UUID: 0x1235 */
/* Write characteristic UUID: 0x1236 */

static const ble_uuid16_t g_svc_uuid = BLE_UUID16_INIT(0x1234);
static const ble_uuid16_t g_chr_read_uuid = BLE_UUID16_INIT(0x1235);
static const ble_uuid16_t g_chr_write_uuid = BLE_UUID16_INIT(0x1236);

/* Value handle for read characteristic (filled in at registration) */
static uint16_t g_chr_read_handle;
static uint16_t g_chr_write_handle;

/* Forward declarations */
static void ble_on_sync(void);
static void ble_on_reset(int reason);
static int ble_gap_event(struct ble_gap_event *event, void *arg);
static void *ble_host_thread(void *arg);
static void *ble_hci_sock_thread(void *arg);
static void do_start_advertising(void);
static int gatt_chr_access(uint16_t conn_handle, uint16_t attr_handle,
                           struct ble_gatt_access_ctxt *ctxt, void *arg);

/****************************************************************************
 * GATT Service Definition
 * - Service UUID: 0x1234
 * - Read characteristic (0x1235): Returns "Hello from RustCam!"
 * - Write characteristic (0x1236): Receives commands
 ****************************************************************************/

static const struct ble_gatt_svc_def g_gatt_svcs[] = {
    {
        .type = BLE_GATT_SVC_TYPE_PRIMARY,
        .uuid = &g_svc_uuid.u,
        .characteristics = (struct ble_gatt_chr_def[]) {
            {
                /* Read characteristic */
                .uuid = &g_chr_read_uuid.u,
                .access_cb = gatt_chr_access,
                .flags = BLE_GATT_CHR_F_READ,
                .val_handle = &g_chr_read_handle,
            },
            {
                /* Write characteristic */
                .uuid = &g_chr_write_uuid.u,
                .access_cb = gatt_chr_access,
                .flags = BLE_GATT_CHR_F_WRITE | BLE_GATT_CHR_F_WRITE_NO_RSP,
                .val_handle = &g_chr_write_handle,
            },
            {
                0, /* No more characteristics */
            },
        },
    },
    {
        0, /* No more services */
    },
};

/****************************************************************************
 * Name: gatt_chr_access
 *
 * Description:
 *   GATT characteristic access callback. Handles read/write requests.
 *
 * Parameters:
 *   conn_handle - Connection handle
 *   attr_handle - Attribute handle
 *   ctxt - GATT access context
 *   arg - User argument (unused)
 *
 * Returns:
 *   0 on success, BLE_ATT_ERR_* on failure
 ****************************************************************************/

static int gatt_chr_access(uint16_t conn_handle, uint16_t attr_handle,
                           struct ble_gatt_access_ctxt *ctxt, void *arg)
{
    const ble_uuid_t *uuid = ctxt->chr->uuid;
    int rc;

    (void)conn_handle;
    (void)attr_handle;
    (void)arg;

    /* Read characteristic (0x1235) */
    if (ble_uuid_cmp(uuid, &g_chr_read_uuid.u) == 0) {
        if (ctxt->op == BLE_GATT_ACCESS_OP_READ_CHR) {
            rc = os_mbuf_append(ctxt->om, g_gatt_read_msg,
                                strlen(g_gatt_read_msg));
            if (rc != 0) {
                return BLE_ATT_ERR_INSUFFICIENT_RES;
            }
            printf("[GATT] Read request: returning '%s'\n", g_gatt_read_msg);
            return 0;
        }
        return BLE_ATT_ERR_UNLIKELY;
    }

    /* Write characteristic (0x1236) */
    if (ble_uuid_cmp(uuid, &g_chr_write_uuid.u) == 0) {
        if (ctxt->op == BLE_GATT_ACCESS_OP_WRITE_CHR) {
            uint16_t len = OS_MBUF_PKTLEN(ctxt->om);
            if (len > sizeof(g_gatt_command) - 1) {
                len = sizeof(g_gatt_command) - 1;
            }

            rc = ble_hs_mbuf_to_flat(ctxt->om, g_gatt_command, len, NULL);
            if (rc != 0) {
                return BLE_ATT_ERR_UNLIKELY;
            }

            g_gatt_command[len] = '\0';
            g_gatt_command_len = len;

            printf("[GATT] Write request: received '%s' (%d bytes)\n",
                   g_gatt_command, len);
            return 0;
        }
        return BLE_ATT_ERR_UNLIKELY;
    }

    return BLE_ATT_ERR_UNLIKELY;
}

/****************************************************************************
 * Name: rust_ble_wrapper_gatt_get_command
 *
 * Description:
 *   Get the last command received via GATT write.
 *
 * Parameters:
 *   buf     - Buffer to store the command
 *   buf_len - Size of the buffer
 *
 * Returns:
 *   Length of command copied, or 0 if no command available
 ****************************************************************************/

int rust_ble_wrapper_gatt_get_command(uint8_t *buf, int buf_len)
{
    int len = g_gatt_command_len;
    if (len == 0 || buf == NULL || buf_len <= 0) {
        return 0;
    }

    if (len > buf_len - 1) {
        len = buf_len - 1;
    }

    memcpy(buf, g_gatt_command, len);
    buf[len] = '\0';

    /* Clear command after reading */
    g_gatt_command_len = 0;

    return len;
}

/****************************************************************************
 * Name: rust_ble_wrapper_gatt_has_command
 *
 * Description:
 *   Check if there is a pending GATT command.
 *
 * Returns:
 *   1 if command is available, 0 otherwise
 ****************************************************************************/

int rust_ble_wrapper_gatt_has_command(void)
{
    return g_gatt_command_len > 0 ? 1 : 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_gatt_set_read_msg
 *
 * Description:
 *   Set the message returned by GATT read operations.
 *
 * Parameters:
 *   msg - The message to return on read (NULL to reset to default)
 *
 * Returns:
 *   0 on success
 ****************************************************************************/

static char g_gatt_read_msg_buf[64] = "Hello from RustCam!";

int rust_ble_wrapper_gatt_set_read_msg(const char *msg)
{
    if (msg == NULL || msg[0] == '\0') {
        strncpy(g_gatt_read_msg_buf, "Hello from RustCam!",
                sizeof(g_gatt_read_msg_buf) - 1);
    } else {
        strncpy(g_gatt_read_msg_buf, msg, sizeof(g_gatt_read_msg_buf) - 1);
    }
    g_gatt_read_msg_buf[sizeof(g_gatt_read_msg_buf) - 1] = '\0';
    g_gatt_read_msg = g_gatt_read_msg_buf;
    return 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_init
 *
 * Description:
 *   Initialize BLE subsystem. Must be called before other BLE functions.
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_ble_wrapper_init(void)
{
    int rc;
    int sock;
    struct ifreq ifr;

    if (g_ble_initialized) {
        return -EALREADY;
    }

    printf("[BLE] Initializing NimBLE...\n");

    /* Bring up bnep0 interface before initializing NimBLE */
    sock = socket(AF_INET, SOCK_DGRAM, 0);
    if (sock >= 0) {
        memset(&ifr, 0, sizeof(ifr));
        strncpy(ifr.ifr_name, "bnep0", IFNAMSIZ);
        rc = ioctl(sock, SIOCGIFFLAGS, &ifr);
        if (rc >= 0) {
            ifr.ifr_flags |= IFF_UP;
            rc = ioctl(sock, SIOCSIFFLAGS, &ifr);
            if (rc < 0) {
                printf("[BLE] Warning: Failed to bring up bnep0: %d\n", errno);
            } else {
                printf("[BLE] Brought up bnep0 interface\n");
            }
        }
        close(sock);
    }

    /* Set HCI socket device to 0 (default Bluetooth interface) */
    ble_hci_sock_set_device(0);

    /* Initialize NimBLE (returns void) */
    nimble_port_init();

    /* Configure callbacks */
    ble_hs_cfg.sync_cb = ble_on_sync;
    ble_hs_cfg.reset_cb = ble_on_reset;

    /* Initialize GAP and GATT services */
    ble_svc_gap_init();
    ble_svc_gatt_init();

    /* Register our custom GATT services */
    rc = ble_gatts_count_cfg(g_gatt_svcs);
    if (rc != 0) {
        printf("[BLE] Failed to count GATT services: %d\n", rc);
        return -rc;
    }

    rc = ble_gatts_add_svcs(g_gatt_svcs);
    if (rc != 0) {
        printf("[BLE] Failed to add GATT services: %d\n", rc);
        return -rc;
    }

    printf("[BLE] Custom GATT service registered (UUID: 0x1234)\n");
    printf("[BLE]   - Read char UUID: 0x1235\n");
    printf("[BLE]   - Write char UUID: 0x1236\n");

    /* Set device name */
    rc = ble_svc_gap_device_name_set(g_device_name);
    if (rc != 0) {
        printf("[BLE] Failed to set device name: %d\n", rc);
    }

    /* Start the HCI socket thread first (handles communication with controller) */
    pthread_attr_t attr;
    pthread_attr_init(&attr);
    pthread_attr_setstacksize(&attr, 4096);
    rc = pthread_create(&g_hci_thread, &attr, ble_hci_sock_thread, NULL);
    if (rc != 0) {
        printf("[BLE] Failed to create HCI socket thread: %d\n", rc);
        return -rc;
    }
    printf("[BLE] HCI socket thread started\n");

    /* Start the host thread */
    rc = pthread_create(&g_host_thread, &attr, ble_host_thread, NULL);
    if (rc != 0) {
        printf("[BLE] Failed to create host thread: %d\n", rc);
        return -rc;
    }
    printf("[BLE] Host thread started\n");

    g_ble_initialized = 1;
    printf("[BLE] Initialized successfully\n");

    return 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_deinit
 *
 * Description:
 *   Deinitialize BLE subsystem.
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_ble_wrapper_deinit(void)
{
    if (!g_ble_initialized) {
        return -ENODEV;
    }

    if (g_ble_advertising) {
        ble_gap_adv_stop();
        g_ble_advertising = 0;
    }

    /* Note: nimble_port doesn't have a stop/deinit function */
    g_ble_initialized = 0;
    g_ble_host_synced = 0;

    printf("[BLE] Deinitialized\n");
    return 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_start_advertising
 *
 * Description:
 *   Start BLE advertising with the given device name.
 *
 * Parameters:
 *   name - Device name to advertise (max 29 chars)
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_ble_wrapper_start_advertising(const char *name)
{
    if (!g_ble_initialized) {
        return -ENODEV;
    }

    /* Update device name if provided */
    if (name != NULL && name[0] != '\0') {
        strncpy(g_device_name, name, sizeof(g_device_name) - 1);
        g_device_name[sizeof(g_device_name) - 1] = '\0';
        ble_svc_gap_device_name_set(g_device_name);
    }

    /* If host is already synced, start advertising directly */
    if (g_ble_host_synced) {
        do_start_advertising();
    } else {
        /* Request advertising to start when host syncs */
        g_pending_adv = 1;
        printf("[BLE] Waiting for host sync before advertising...\n");
    }

    return 0;
}

/****************************************************************************
 * Name: do_start_advertising
 *
 * Description:
 *   Actually start advertising (called when host is synced).
 ****************************************************************************/

static void do_start_advertising(void)
{
    struct ble_gap_adv_params adv_params;
    uint8_t ad[BLE_HS_ADV_MAX_SZ];
    uint8_t ad_len = 0;
    uint8_t ad_flags = BLE_HS_ADV_F_DISC_GEN | BLE_HS_ADV_F_BREDR_UNSUP;
    int rc;

    /* Build advertising data manually (more reliable) */
    ad[ad_len++] = 2;  /* Length */
    ad[ad_len++] = BLE_HS_ADV_TYPE_FLAGS;
    ad[ad_len++] = ad_flags;

    ad[ad_len++] = strlen(g_device_name) + 1;  /* Length */
    ad[ad_len++] = BLE_HS_ADV_TYPE_COMP_NAME;
    memcpy(&ad[ad_len], g_device_name, strlen(g_device_name));
    ad_len += strlen(g_device_name);

    rc = ble_gap_adv_set_data(ad, ad_len);
    if (rc != 0) {
        printf("[BLE] Failed to set adv data: %d\n", rc);
        return;
    }

    /* Start advertising */
    memset(&adv_params, 0, sizeof(adv_params));
    adv_params.conn_mode = BLE_GAP_CONN_MODE_UND;
    adv_params.disc_mode = BLE_GAP_DISC_MODE_GEN;

    rc = ble_gap_adv_start(g_own_addr_type, NULL, BLE_HS_FOREVER,
                           &adv_params, ble_gap_event, NULL);
    if (rc != 0) {
        printf("[BLE] Failed to start advertising: %d\n", rc);
        return;
    }

    g_ble_advertising = 1;
    printf("[BLE] Advertising as '%s'\n", g_device_name);
}

/****************************************************************************
 * Name: rust_ble_wrapper_stop_advertising
 *
 * Description:
 *   Stop BLE advertising.
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_ble_wrapper_stop_advertising(void)
{
    if (!g_ble_advertising) {
        return 0;
    }

    ble_gap_adv_stop();
    g_ble_advertising = 0;
    g_pending_adv = 0;

    printf("[BLE] Advertising stopped\n");
    return 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_is_connected
 *
 * Description:
 *   Check if a device is connected.
 *
 * Returns:
 *   1 if connected, 0 otherwise
 ****************************************************************************/

int rust_ble_wrapper_is_connected(void)
{
    return g_ble_connected;
}

/****************************************************************************
 * Name: rust_ble_wrapper_run
 *
 * Description:
 *   Run the BLE host task. This blocks until BLE is stopped.
 *   Should be called from a separate thread.
 ****************************************************************************/

void rust_ble_wrapper_run(void)
{
    printf("[BLE] Starting host task\n");
    nimble_port_run();
    printf("[BLE] Host task stopped\n");
}

/****************************************************************************
 * Private Functions
 ****************************************************************************/

static void *ble_host_thread(void *arg)
{
    (void)arg;
    printf("[BLE] Host thread running\n");
    nimble_port_run();
    printf("[BLE] Host thread exited\n");
    return NULL;
}

static void *ble_hci_sock_thread(void *arg)
{
    (void)arg;
    printf("[BLE] HCI socket thread running\n");
    ble_hci_sock_ack_handler(NULL);
    printf("[BLE] HCI socket thread exited\n");
    return NULL;
}

static void ble_on_sync(void)
{
    int rc;
    ble_addr_t addr;

    printf("[BLE] Host synced\n");

    /* Generate a non-resolvable private address */
    rc = ble_hs_id_gen_rnd(1, &addr);
    if (rc != 0) {
        printf("[BLE] Failed to generate random address: %d\n", rc);
    } else {
        printf("[BLE] Random Address: %02X:%02X:%02X:%02X:%02X:%02X\n",
               addr.val[5], addr.val[4], addr.val[3],
               addr.val[2], addr.val[1], addr.val[0]);

        rc = ble_hs_id_set_rnd(addr.val);
        if (rc != 0) {
            printf("[BLE] Failed to set random address: %d\n", rc);
        }
    }

    rc = ble_hs_util_ensure_addr(0);
    if (rc != 0) {
        printf("[BLE] Failed to ensure address: %d\n", rc);
        return;
    }

    rc = ble_hs_id_infer_auto(0, &g_own_addr_type);
    if (rc != 0) {
        printf("[BLE] Failed to infer address type: %d\n", rc);
        return;
    }

    g_ble_host_synced = 1;

    /* Start pending advertising if requested */
    if (g_pending_adv) {
        g_pending_adv = 0;
        do_start_advertising();
    }
}

static void ble_on_reset(int reason)
{
    printf("[BLE] Host reset, reason=%d\n", reason);
    g_ble_host_synced = 0;
}

static int ble_gap_event(struct ble_gap_event *event, void *arg)
{
    (void)arg;

    switch (event->type) {
        case BLE_GAP_EVENT_CONNECT:
            if (event->connect.status == 0) {
                g_conn_handle = event->connect.conn_handle;
                g_ble_connected = 1;
                printf("[BLE] Connected, handle=%d\n", g_conn_handle);
            } else {
                printf("[BLE] Connection failed, status=%d\n",
                       event->connect.status);
                /* Resume advertising */
                if (g_ble_advertising) {
                    do_start_advertising();
                }
            }
            break;

        case BLE_GAP_EVENT_DISCONNECT:
            g_ble_connected = 0;
            printf("[BLE] Disconnected, reason=%d\n",
                   event->disconnect.reason);
            /* Resume advertising */
            if (g_ble_advertising) {
                do_start_advertising();
            }
            break;

        case BLE_GAP_EVENT_ADV_COMPLETE:
            printf("[BLE] Advertising complete\n");
            break;

        case BLE_GAP_EVENT_MTU:
            printf("[BLE] MTU updated to %d\n", event->mtu.value);
            break;

        default:
            break;
    }

    return 0;
}

#elif defined(CONFIG_WIRELESS_BLUETOOTH)

/****************************************************************************
 * NuttX Native Bluetooth Implementation
 * Uses IOCTL interface via bt_ioctl.h and bt_gatt.h for GATT
 ****************************************************************************/

#include <sys/socket.h>
#include <netpacket/bluetooth.h>
#include <nuttx/wireless/bluetooth/bt_ioctl.h>
#include <nuttx/wireless/bluetooth/bt_core.h>
#include <nuttx/wireless/bluetooth/bt_hci.h>
#include <nuttx/wireless/bluetooth/bt_uuid.h>
#include <nuttx/wireless/bluetooth/bt_gatt.h>
#include <net/if.h>

/* State */
static volatile int g_ble_initialized = 0;
static volatile int g_ble_advertising = 0;
static volatile int g_gatt_registered = 0;
static int g_bt_sockfd = -1;
static char g_device_name[32] = "RustCam";

/* BLE interface name - ESP32 BLE typically uses bnep0 */
#define BT_IFNAME "bnep0"

/* GATT command buffer - stores last received command */
static uint8_t g_gatt_command[64];
static volatile uint8_t g_gatt_command_len = 0;

/* GATT read response message */
static char g_gatt_read_msg_buf[64] = "Hello from RustCam!";

/****************************************************************************
 * GATT Service Definitions for Native Bluetooth
 *
 * Combined GATT table with:
 * 1. GAP Service (UUID 0x1800) - Required for BLE
 * 2. RustCam Service (UUID 0x1234) - Custom service with read/write chars
 *
 * Handle allocation:
 *   GAP Service:
 *   0x0001 - GAP Primary Service
 *   0x0002 - Device Name Characteristic declaration
 *   0x0003 - Device Name value
 *   0x0004 - Appearance Characteristic declaration
 *   0x0005 - Appearance value
 *
 *   RustCam Service:
 *   0x0010 - Primary Service (0x1234)
 *   0x0011 - Read Characteristic declaration
 *   0x0012 - Read Characteristic value (0x1235)
 *   0x0013 - Write Characteristic declaration
 *   0x0014 - Write Characteristic value (0x1236)
 ****************************************************************************/

/* GAP Service handle definitions */
#define GAP_SVC_HANDLE         0x0001
#define GAP_NAME_CHR_HANDLE    0x0002
#define GAP_NAME_VAL_HANDLE    0x0003
#define GAP_APPEAR_CHR_HANDLE  0x0004
#define GAP_APPEAR_VAL_HANDLE  0x0005

/* RustCam Service handle definitions */
#define RUSTCAM_SVC_HANDLE     0x0010
#define READ_CHR_HANDLE        0x0011
#define READ_VAL_HANDLE        0x0012
#define WRITE_CHR_HANDLE       0x0013
#define WRITE_VAL_HANDLE       0x0014

/* GAP Service UUID: 0x1800 */
static struct bt_uuid_s g_gap_svc_uuid =
{
  .type = BT_UUID_16,
  .u    = { .u16 = BT_UUID_GAP },
};

/* GAP Device Name UUID: 0x2A00 */
static struct bt_uuid_s g_gap_name_uuid =
{
  .type = BT_UUID_16,
  .u    = { .u16 = BT_UUID_GAP_DEVICE_NAME },
};

/* GAP Appearance UUID: 0x2A01 */
static struct bt_uuid_s g_gap_appear_uuid =
{
  .type = BT_UUID_16,
  .u    = { .u16 = BT_UUID_GAP_APPEARANCE },
};

/* GAP Device Name characteristic */
static struct bt_gatt_chrc_s g_gap_name_chrc =
{
  .properties   = BT_GATT_CHRC_READ,
  .value_handle = GAP_NAME_VAL_HANDLE,
  .uuid         = &g_gap_name_uuid,
};

/* GAP Appearance characteristic */
static struct bt_gatt_chrc_s g_gap_appear_chrc =
{
  .properties   = BT_GATT_CHRC_READ,
  .value_handle = GAP_APPEAR_VAL_HANDLE,
  .uuid         = &g_gap_appear_uuid,
};

/* Service UUID: 0x1234 */
static struct bt_uuid_s g_rustcam_svc_uuid =
{
  .type = BT_UUID_16,
  .u    = { .u16 = 0x1234 },
};

/* Read characteristic UUID: 0x1235 */
static struct bt_uuid_s g_read_chr_uuid =
{
  .type = BT_UUID_16,
  .u    = { .u16 = 0x1235 },
};

/* Write characteristic UUID: 0x1236 */
static struct bt_uuid_s g_write_chr_uuid =
{
  .type = BT_UUID_16,
  .u    = { .u16 = 0x1236 },
};

/* Read characteristic declaration */
static struct bt_gatt_chrc_s g_read_chrc =
{
  .properties   = BT_GATT_CHRC_READ,
  .value_handle = READ_VAL_HANDLE,
  .uuid         = &g_read_chr_uuid,
};

/* Write characteristic declaration */
static struct bt_gatt_chrc_s g_write_chrc =
{
  .properties   = BT_GATT_CHRC_WRITE | BT_GATT_CHRC_WRITE_WITHOUT_RESP,
  .value_handle = WRITE_VAL_HANDLE,
  .uuid         = &g_write_chr_uuid,
};

/* Forward declarations for GATT callbacks */
static int gap_read_name(FAR struct bt_conn_s *conn,
                         FAR const struct bt_gatt_attr_s *attr,
                         FAR void *buf, uint8_t len, uint16_t offset);

static int gap_read_appearance(FAR struct bt_conn_s *conn,
                               FAR const struct bt_gatt_attr_s *attr,
                               FAR void *buf, uint8_t len, uint16_t offset);

static int gatt_read_value(FAR struct bt_conn_s *conn,
                           FAR const struct bt_gatt_attr_s *attr,
                           FAR void *buf, uint8_t len, uint16_t offset);

static int gatt_write_value(FAR struct bt_conn_s *conn,
                            FAR const struct bt_gatt_attr_s *attr,
                            FAR const void *buf, uint8_t len, uint16_t offset);

/* Combined GATT attribute table (GAP + RustCam) */
static const struct bt_gatt_attr_s g_combined_attrs[] =
{
  /* ===== GAP Service (0x1800) ===== */
  BT_GATT_PRIMARY_SERVICE(GAP_SVC_HANDLE, &g_gap_svc_uuid),

  /* Device Name Characteristic */
  BT_GATT_CHARACTERISTIC(GAP_NAME_CHR_HANDLE, &g_gap_name_chrc),
  BT_GATT_DESCRIPTOR(GAP_NAME_VAL_HANDLE, &g_gap_name_uuid, BT_GATT_PERM_READ,
                     gap_read_name, NULL, NULL),

  /* Appearance Characteristic */
  BT_GATT_CHARACTERISTIC(GAP_APPEAR_CHR_HANDLE, &g_gap_appear_chrc),
  BT_GATT_DESCRIPTOR(GAP_APPEAR_VAL_HANDLE, &g_gap_appear_uuid, BT_GATT_PERM_READ,
                     gap_read_appearance, NULL, NULL),

  /* ===== RustCam Service (0x1234) ===== */
  BT_GATT_PRIMARY_SERVICE(RUSTCAM_SVC_HANDLE, &g_rustcam_svc_uuid),

  /* Read Characteristic declaration */
  BT_GATT_CHARACTERISTIC(READ_CHR_HANDLE, &g_read_chrc),

  /* Read Characteristic value (0x1235) */
  BT_GATT_DESCRIPTOR(READ_VAL_HANDLE, &g_read_chr_uuid, BT_GATT_PERM_READ,
                     gatt_read_value, NULL, NULL),

  /* Write Characteristic declaration */
  BT_GATT_CHARACTERISTIC(WRITE_CHR_HANDLE, &g_write_chrc),

  /* Write Characteristic value (0x1236) */
  BT_GATT_DESCRIPTOR(WRITE_VAL_HANDLE, &g_write_chr_uuid, BT_GATT_PERM_WRITE,
                     NULL, gatt_write_value, NULL),
};

/****************************************************************************
 * GAP Service Callbacks
 ****************************************************************************/

static int gap_read_name(FAR struct bt_conn_s *conn,
                         FAR const struct bt_gatt_attr_s *attr,
                         FAR void *buf, uint8_t len, uint16_t offset)
{
  (void)conn;
  (void)attr;

  return bt_gatt_attr_read(conn, attr, buf, len, offset,
                           g_device_name, strlen(g_device_name));
}

static int gap_read_appearance(FAR struct bt_conn_s *conn,
                               FAR const struct bt_gatt_attr_s *attr,
                               FAR void *buf, uint8_t len, uint16_t offset)
{
  uint16_t appearance = 0;  /* Generic device */
  (void)conn;
  (void)attr;

  return bt_gatt_attr_read(conn, attr, buf, len, offset,
                           &appearance, sizeof(appearance));
}

/****************************************************************************
 * Name: gatt_read_value
 *
 * Description:
 *   GATT read callback for the read characteristic (0x1235).
 *   Returns the current read message.
 ****************************************************************************/

static int gatt_read_value(FAR struct bt_conn_s *conn,
                           FAR const struct bt_gatt_attr_s *attr,
                           FAR void *buf, uint8_t len, uint16_t offset)
{
  (void)conn;
  (void)attr;

  printf("[GATT] Read request: returning '%s'\n", g_gatt_read_msg_buf);

  return bt_gatt_attr_read(conn, attr, buf, len, offset,
                           g_gatt_read_msg_buf, strlen(g_gatt_read_msg_buf));
}

/****************************************************************************
 * Name: gatt_write_value
 *
 * Description:
 *   GATT write callback for the write characteristic (0x1236).
 *   Stores the received command in the buffer.
 ****************************************************************************/

static int gatt_write_value(FAR struct bt_conn_s *conn,
                            FAR const struct bt_gatt_attr_s *attr,
                            FAR const void *buf, uint8_t len, uint16_t offset)
{
  (void)conn;
  (void)attr;
  (void)offset;

  if (len > sizeof(g_gatt_command) - 1)
    {
      len = sizeof(g_gatt_command) - 1;
    }

  memcpy(g_gatt_command, buf, len);
  g_gatt_command[len] = '\0';
  g_gatt_command_len = len;

  printf("[GATT] Write request: received '%s' (%d bytes)\n",
         g_gatt_command, len);

  return len;
}

/* Forward declaration */
int rust_ble_wrapper_stop_advertising(void);

/****************************************************************************
 * Name: rust_ble_wrapper_init
 *
 * Description:
 *   Initialize BLE subsystem using NuttX native Bluetooth.
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_ble_wrapper_init(void)
{
    if (g_ble_initialized) {
        return -EALREADY;
    }

    printf("[BLE] Initializing NuttX native Bluetooth...\n");

    /* Create Bluetooth socket */
    g_bt_sockfd = socket(PF_BLUETOOTH, SOCK_RAW, BTPROTO_L2CAP);
    if (g_bt_sockfd < 0) {
        int err = errno;
        printf("[BLE] Failed to create socket: %d (%s)\n", err, strerror(err));
        return -err;
    }

    /* Register combined GATT attributes (GAP + RustCam) */
    if (!g_gatt_registered) {
        bt_gatt_register(g_combined_attrs,
                         sizeof(g_combined_attrs) / sizeof(g_combined_attrs[0]));
        g_gatt_registered = 1;
        printf("[BLE] GATT services registered:\n");
        printf("[BLE]   - GAP Service (0x1800)\n");
        printf("[BLE]   - RustCam Service (0x1234)\n");
        printf("[BLE]     - Read characteristic: 0x1235\n");
        printf("[BLE]     - Write characteristic: 0x1236\n");
    }

    g_ble_initialized = 1;
    printf("[BLE] Initialized successfully (socket fd=%d)\n", g_bt_sockfd);

    return 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_deinit
 *
 * Description:
 *   Deinitialize BLE subsystem.
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_ble_wrapper_deinit(void)
{
    if (!g_ble_initialized) {
        return -ENODEV;
    }

    if (g_ble_advertising) {
        rust_ble_wrapper_stop_advertising();
    }

    if (g_bt_sockfd >= 0) {
        close(g_bt_sockfd);
        g_bt_sockfd = -1;
    }

    g_ble_initialized = 0;
    printf("[BLE] Deinitialized\n");
    return 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_start_advertising
 *
 * Description:
 *   Start BLE advertising with the given device name using NuttX IOCTL.
 *
 * Parameters:
 *   name - Device name to advertise (max 29 chars)
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_ble_wrapper_start_advertising(const char *name)
{
    struct btreq_s btreq;
    struct bt_eir_s ad[3];  /* Flags + Name + terminator */
    int ret;
    size_t name_len;

    if (!g_ble_initialized) {
        return -ENODEV;
    }

    if (g_ble_advertising) {
        printf("[BLE] Already advertising\n");
        return 0;
    }

    /* Update device name if provided */
    if (name != NULL && name[0] != '\0') {
        strncpy(g_device_name, name, sizeof(g_device_name) - 1);
        g_device_name[sizeof(g_device_name) - 1] = '\0';
    }

    name_len = strlen(g_device_name);

    printf("[BLE] Starting advertising as '%s'...\n", g_device_name);

    /* Build advertising data - terminated with len=0 entry */
    memset(ad, 0, sizeof(ad));

    /* AD structure 0: Flags */
    ad[0].len = 2;
    ad[0].type = BT_EIR_FLAGS;
    ad[0].data[0] = BT_LE_AD_GENERAL | BT_LE_AD_NO_BREDR;

    /* AD structure 1: Complete Local Name */
    ad[1].len = name_len + 1;
    ad[1].type = BT_EIR_NAME_COMPLETE;
    memcpy(ad[1].data, g_device_name, name_len);

    /* AD structure 2: Terminator (len=0 already set by memset) */

    /* Setup btreq structure */
    memset(&btreq, 0, sizeof(btreq));
    strlcpy(btreq.btr_name, BT_IFNAME, sizeof(btreq.btr_name));
    btreq.btr_advtype = BT_LE_ADV_IND;  /* Connectable undirected advertising */
    btreq.btr_advad = ad;

    /* Start advertising via IOCTL */
    ret = ioctl(g_bt_sockfd, SIOCBTADVSTART, (unsigned long)((uintptr_t)&btreq));
    if (ret < 0) {
        int err = errno;
        printf("[BLE] Failed to start advertising: %d (%s)\n", err, strerror(err));
        return -err;
    }

    g_ble_advertising = 1;
    printf("[BLE] Advertising started as '%s'\n", g_device_name);

    return 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_stop_advertising
 *
 * Description:
 *   Stop BLE advertising.
 *
 * Returns:
 *   0 on success, negative errno on failure
 ****************************************************************************/

int rust_ble_wrapper_stop_advertising(void)
{
    struct btreq_s btreq;
    int ret;

    if (!g_ble_initialized) {
        return -ENODEV;
    }

    if (!g_ble_advertising) {
        return 0;
    }

    memset(&btreq, 0, sizeof(btreq));
    strlcpy(btreq.btr_name, BT_IFNAME, sizeof(btreq.btr_name));

    ret = ioctl(g_bt_sockfd, SIOCBTADVSTOP, (unsigned long)((uintptr_t)&btreq));
    if (ret < 0) {
        int err = errno;
        printf("[BLE] Failed to stop advertising: %d (%s)\n", err, strerror(err));
        return -err;
    }

    g_ble_advertising = 0;
    printf("[BLE] Advertising stopped\n");
    return 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_is_connected
 *
 * Description:
 *   Check if a device is connected.
 *   Note: Connection tracking not yet implemented for native BLE.
 *
 * Returns:
 *   1 if connected, 0 otherwise
 ****************************************************************************/

int rust_ble_wrapper_is_connected(void)
{
    /* TODO: Implement connection state tracking for native BLE */
    return 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_run
 *
 * Description:
 *   Run the BLE host task. For native BLE, this is a no-op since the
 *   kernel handles everything.
 ****************************************************************************/

void rust_ble_wrapper_run(void)
{
    /* Native BLE doesn't need a separate host thread */
    printf("[BLE] Native BLE - no host thread needed\n");
}

/****************************************************************************
 * GATT Functions for Native Bluetooth
 ****************************************************************************/

/****************************************************************************
 * Name: rust_ble_wrapper_gatt_get_command
 *
 * Description:
 *   Get the last command received via GATT write.
 *
 * Parameters:
 *   buf     - Buffer to store the command
 *   buf_len - Size of the buffer
 *
 * Returns:
 *   Length of command copied, or 0 if no command available
 ****************************************************************************/

int rust_ble_wrapper_gatt_get_command(uint8_t *buf, int buf_len)
{
    int len = g_gatt_command_len;
    if (len == 0 || buf == NULL || buf_len <= 0) {
        return 0;
    }

    if (len > buf_len - 1) {
        len = buf_len - 1;
    }

    memcpy(buf, g_gatt_command, len);
    buf[len] = '\0';

    /* Clear command after reading */
    g_gatt_command_len = 0;

    return len;
}

/****************************************************************************
 * Name: rust_ble_wrapper_gatt_has_command
 *
 * Description:
 *   Check if there is a pending GATT command.
 *
 * Returns:
 *   1 if command is available, 0 otherwise
 ****************************************************************************/

int rust_ble_wrapper_gatt_has_command(void)
{
    return g_gatt_command_len > 0 ? 1 : 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_gatt_set_read_msg
 *
 * Description:
 *   Set the message returned by GATT read operations.
 *
 * Parameters:
 *   msg - The message to return on read (NULL to reset to default)
 *
 * Returns:
 *   0 on success
 ****************************************************************************/

int rust_ble_wrapper_gatt_set_read_msg(const char *msg)
{
    if (msg == NULL || msg[0] == '\0') {
        strncpy(g_gatt_read_msg_buf, "Hello from RustCam!",
                sizeof(g_gatt_read_msg_buf) - 1);
    } else {
        strncpy(g_gatt_read_msg_buf, msg, sizeof(g_gatt_read_msg_buf) - 1);
    }
    g_gatt_read_msg_buf[sizeof(g_gatt_read_msg_buf) - 1] = '\0';
    return 0;
}

/****************************************************************************
 * Name: rust_ble_wrapper_debug_print_status
 *
 * Description:
 *   Print debug status information for troubleshooting GATT issues.
 *   This helps diagnose why GATT operations might not be working.
 ****************************************************************************/

void rust_ble_wrapper_debug_print_status(void)
{
    struct btreq_s btreq;
    int ret;

    printf("\n========== BLE DEBUG STATUS ==========\n");
    printf("[BLE] Initialized: %s\n", g_ble_initialized ? "YES" : "NO");
    printf("[BLE] Advertising: %s\n", g_ble_advertising ? "YES" : "NO");
    printf("[BLE] GATT Registered: %s\n", g_gatt_registered ? "YES" : "NO");
    printf("[BLE] Socket FD: %d\n", g_bt_sockfd);
    printf("[BLE] Device Name: %s\n", g_device_name);
    printf("[BLE] Read Message: %s\n", g_gatt_read_msg_buf);
    printf("[BLE] Pending Command: %s (len=%d)\n",
           g_gatt_command_len > 0 ? (char*)g_gatt_command : "(none)",
           g_gatt_command_len);

    /* Query BT device info via IOCTL */
    if (g_bt_sockfd >= 0) {
        memset(&btreq, 0, sizeof(btreq));
        strlcpy(btreq.btr_name, BT_IFNAME, sizeof(btreq.btr_name));

        ret = ioctl(g_bt_sockfd, SIOCGBTINFO, (unsigned long)((uintptr_t)&btreq));
        if (ret >= 0) {
            printf("\n[HCI] Device Info (from IOCTL):\n");
            printf("  - BD Address: %02X:%02X:%02X:%02X:%02X:%02X\n",
                   btreq.btr_bdaddr.val[5], btreq.btr_bdaddr.val[4],
                   btreq.btr_bdaddr.val[3], btreq.btr_bdaddr.val[2],
                   btreq.btr_bdaddr.val[1], btreq.btr_bdaddr.val[0]);
            printf("  - Flags: 0x%04X\n", btreq.btr_flags);
            printf("  - Free CMD buffers: %d\n", btreq.btr_num_cmd);
            printf("  - Free ACL buffers: %d (max: %d)\n",
                   btreq.btr_num_acl, btreq.btr_max_acl);
            printf("  - ACL MTU: %d\n", btreq.btr_acl_mtu);
        } else {
            printf("\n[HCI] Failed to get device info: %s\n", strerror(errno));
        }
    }

    /* Print GATT table info */
    printf("\n[GATT] Combined Attribute Table:\n");
    printf("  - GAP Service (0x1800) at handles 0x%04X-0x%04X\n",
           GAP_SVC_HANDLE, GAP_APPEAR_VAL_HANDLE);
    printf("    - Device Name (0x2A00): handle 0x%04X\n", GAP_NAME_VAL_HANDLE);
    printf("    - Appearance (0x2A01): handle 0x%04X\n", GAP_APPEAR_VAL_HANDLE);
    printf("  - RustCam Service (0x1234) at handles 0x%04X-0x%04X\n",
           RUSTCAM_SVC_HANDLE, WRITE_VAL_HANDLE);
    printf("    - Read Char (0x1235): handle 0x%04X\n", READ_VAL_HANDLE);
    printf("    - Write Char (0x1236): handle 0x%04X\n", WRITE_VAL_HANDLE);
    printf("  - Total attributes: %zu\n",
           sizeof(g_combined_attrs) / sizeof(g_combined_attrs[0]));

    printf("\n[DEBUG] To test GATT from another device:\n");
    printf("  1. Scan: hcitool lescan\n");
    printf("  2. Connect: gatttool -b <addr> -I\n");
    printf("  3. In gatttool: connect\n");
    printf("  4. Discover: primary\n");
    printf("  5. Read: char-read-hnd 0x0012\n");
    printf("  6. Write: char-write-cmd 0x0014 48656c6c6f\n");
    printf("======================================\n\n");
}

#else /* Neither NimBLE nor native Bluetooth */

/* Stub implementations when no BLE backend is enabled */

int rust_ble_wrapper_init(void)
{
    printf("[BLE] No BLE backend available (need CONFIG_NIMBLE or CONFIG_WIRELESS_BLUETOOTH)\n");
    return -ENOTSUP;
}

int rust_ble_wrapper_deinit(void)
{
    return -ENOTSUP;
}

int rust_ble_wrapper_start_advertising(const char *name)
{
    (void)name;
    return -ENOTSUP;
}

int rust_ble_wrapper_stop_advertising(void)
{
    return -ENOTSUP;
}

int rust_ble_wrapper_is_connected(void)
{
    return 0;
}

void rust_ble_wrapper_run(void)
{
}

int rust_ble_wrapper_gatt_get_command(uint8_t *buf, int buf_len)
{
    (void)buf;
    (void)buf_len;
    return 0;
}

int rust_ble_wrapper_gatt_has_command(void)
{
    return 0;
}

int rust_ble_wrapper_gatt_set_read_msg(const char *msg)
{
    (void)msg;
    return 0;
}

#endif /* CONFIG_NIMBLE / CONFIG_WIRELESS_BLUETOOTH */
