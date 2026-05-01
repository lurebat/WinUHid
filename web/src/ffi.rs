//! FFI bindings for `WinUHid.dll` and `WinUHidDevs.dll`.
//!
//! Both libraries are loaded *dynamically* with [`libloading`] so this
//! crate compiles even when the WinUHid native build hasn't been done
//! yet. Every entry point used by the web app is mirrored here, with
//! the C signature documented above each binding.

#![allow(
    non_snake_case,
    non_camel_case_types,
    dead_code,
    clippy::upper_case_acronyms
)]

use std::ffi::{c_void, OsStr};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use libloading::{Library, Symbol};

// ---------------------------------------------------------------------------
// Win32 type aliases
// ---------------------------------------------------------------------------

pub type BOOL = i32;
pub type BYTE = u8;
pub type CHAR = i8;
pub type UCHAR = u8;
pub type SHORT = i16;
pub type USHORT = u16;
pub type INT = i32;
pub type UINT = u32;
pub type LONG = i32;
pub type ULONG = u32;
pub type DWORD = u32;
pub type LPCVOID = *const c_void;
pub type PVOID = *mut c_void;
pub type PCWSTR = *const u16;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

// ---------------------------------------------------------------------------
// WinUHid.h — generic SDK
// ---------------------------------------------------------------------------

pub const WINUHID_EVENT_NONE: u32 = 0x0;
pub const WINUHID_EVENT_GET_FEATURE: u32 = 0x1;
pub const WINUHID_EVENT_SET_FEATURE: u32 = 0x2;
pub const WINUHID_EVENT_WRITE_REPORT: u32 = 0x4;
pub const WINUHID_EVENT_READ_REPORT: u32 = 0x8;

pub type WINUHID_EVENT_TYPE = u32;
pub type WINUHID_REQUEST_ID = u32;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct WINUHID_DEVICE_CONFIG {
    pub SupportedEvents: WINUHID_EVENT_TYPE,
    pub VendorID: USHORT,
    pub ProductID: USHORT,
    pub VersionNumber: USHORT,
    pub ReportDescriptorLength: USHORT,
    pub ReportDescriptor: LPCVOID,
    pub ContainerId: GUID,
    pub InstanceID: PCWSTR,
    pub HardwareIDs: PCWSTR,
    pub ReadReportPeriodUs: UINT,
}

/// Layout of the *header* of `WINUHID_EVENT`; the trailing bytes are
/// either a `Write { length, data[] }` or a `Read { length }` sub-record.
#[repr(C, packed)]
pub struct WINUHID_EVENT_HEADER {
    pub Type: WINUHID_EVENT_TYPE,
    pub RequestId: WINUHID_REQUEST_ID,
    pub ReportId: UCHAR,
}

pub type PWINUHID_DEVICE = *mut c_void;
pub type PCWINUHID_EVENT = *const c_void;

pub type WinUHidEventCallback =
    unsafe extern "system" fn(callback_ctx: PVOID, device: PWINUHID_DEVICE, event: PCWINUHID_EVENT);

// ---------------------------------------------------------------------------
// WinUHidDevs.h — shared preset preamble
// ---------------------------------------------------------------------------

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct WINUHID_PRESET_DEVICE_INFO {
    pub VendorID: USHORT,
    pub ProductID: USHORT,
    pub VersionNumber: USHORT,
    pub ContainerId: GUID,
    pub InstanceID: PCWSTR,
    pub HardwareIDs: PCWSTR,
}

// ---------------------------------------------------------------------------
// Mouse preset
// ---------------------------------------------------------------------------

pub type PWINUHID_MOUSE_DEVICE = *mut c_void;

pub const WUHM_BUTTON_LEFT: u8 = 0x01;
pub const WUHM_BUTTON_RIGHT: u8 = 0x02;
pub const WUHM_BUTTON_MIDDLE: u8 = 0x03;
pub const WUHM_BUTTON_X1: u8 = 0x04;
pub const WUHM_BUTTON_X2: u8 = 0x05;

// ---------------------------------------------------------------------------
// PS4 preset
// ---------------------------------------------------------------------------

pub type PWINUHID_PS4_GAMEPAD = *mut c_void;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct WINUHID_PS4_GAMEPAD_INFO {
    pub BasicInfo: *const WINUHID_PRESET_DEVICE_INFO,
    pub MacAddress: [u8; 6],
}

/// The DualShock 4 input report. Layout matches `WinUHidPS4.h` exactly.
/// Bitfields are flattened into `_packed_*` bytes — we set them via the
/// SDK helpers (`WinUHidPS4SetHatState`, etc.) rather than constructing
/// them ourselves.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct WINUHID_PS4_INPUT_REPORT {
    pub ReportId: u8,
    pub LeftStickX: u8,
    pub LeftStickY: u8,
    pub RightStickX: u8,
    pub RightStickY: u8,
    pub _packed_hat_face: u8, // hat:4, square:1, cross:1, circle:1, triangle:1
    pub _packed_shoulder_stick: u8, // L1:1, R1:1, L2:1, R2:1, share:1, options:1, L3:1, R3:1
    pub _packed_meta: u8,     // home:1, touchpad:1, reserved:6
    pub LeftTrigger: u8,
    pub RightTrigger: u8,
    pub Timestamp: u16,
    pub BatteryLevel: u8,
    pub GyroX: u16,
    pub GyroY: u16,
    pub GyroZ: u16,
    pub AccelX: u16,
    pub AccelY: u16,
    pub AccelZ: u16,
    pub Reserved2: [u8; 5],
    pub BatteryLevelSpecial: u8,
    pub Status: [u8; 2],
    pub TouchReportCount: u8,
    pub TouchReports: [Ps4TouchReport; 3],
    pub Reserved3: [u8; 3],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Ps4TouchReport {
    pub Timestamp: u8,
    pub TouchPoints: [Ps4TouchPoint; 2],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Ps4TouchPoint {
    pub ContactSeq: u8,
    pub XLowPart: u8,
    pub _packed_x_high_y_low: u8,
    pub YHighPart: u8,
}

// PS4 face/shoulder/meta bit masks for direct manipulation.
pub const PS4_FACE_SQUARE: u8 = 1 << 4;
pub const PS4_FACE_CROSS: u8 = 1 << 5;
pub const PS4_FACE_CIRCLE: u8 = 1 << 6;
pub const PS4_FACE_TRIANGLE: u8 = 1 << 7;

pub const PS4_BTN_L1: u8 = 1 << 0;
pub const PS4_BTN_R1: u8 = 1 << 1;
pub const PS4_BTN_L2: u8 = 1 << 2;
pub const PS4_BTN_R2: u8 = 1 << 3;
pub const PS4_BTN_SHARE: u8 = 1 << 4;
pub const PS4_BTN_OPTIONS: u8 = 1 << 5;
pub const PS4_BTN_L3: u8 = 1 << 6;
pub const PS4_BTN_R3: u8 = 1 << 7;

pub const PS4_BTN_HOME: u8 = 1 << 0;
pub const PS4_BTN_TOUCHPAD: u8 = 1 << 1;

pub type PWINUHID_PS4_FF_CB =
    unsafe extern "system" fn(ctx: PVOID, left_motor: u8, right_motor: u8);
pub type PWINUHID_PS4_LED_CB =
    unsafe extern "system" fn(ctx: PVOID, led_red: u8, led_green: u8, led_blue: u8);

// ---------------------------------------------------------------------------
// PS5 preset
// ---------------------------------------------------------------------------

pub type PWINUHID_PS5_GAMEPAD = *mut c_void;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct WINUHID_PS5_GAMEPAD_INFO {
    pub BasicInfo: *const WINUHID_PRESET_DEVICE_INFO,
    pub MacAddress: [u8; 6],
    pub FirmwareInfo: *const u8,
    pub FirmwareInfoLength: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct WINUHID_PS5_TRIGGER_EFFECT {
    pub Kind: u8,
    pub Data: [u8; 10],
}

/// Opaque-ish wrapper for the DualSense input report. Sized to be a
/// safe upper bound (the C struct is ~64 bytes); we let the SDK fill
/// it via `WinUHidPS5InitializeInputReport` and a handful of setters,
/// then patch bitfields directly through `as_bytes_mut()` indices that
/// match `WinUHidPS5.h`.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct WINUHID_PS5_INPUT_REPORT(pub [u8; 64]);

impl Default for WINUHID_PS5_INPUT_REPORT {
    fn default() -> Self {
        Self([0u8; 64])
    }
}

// PS5 byte offsets within WINUHID_PS5_INPUT_REPORT (see WinUHidPS5.h).
pub const PS5_OFF_LEFT_STICK_X: usize = 1;
pub const PS5_OFF_LEFT_STICK_Y: usize = 2;
pub const PS5_OFF_RIGHT_STICK_X: usize = 3;
pub const PS5_OFF_RIGHT_STICK_Y: usize = 4;
pub const PS5_OFF_LEFT_TRIGGER: usize = 5;
pub const PS5_OFF_RIGHT_TRIGGER: usize = 6;
// Byte 7 = SequenceNumber (managed by SDK).
pub const PS5_OFF_HAT_FACE: usize = 8; // hat:4, square:1, cross:1, circle:1, triangle:1
pub const PS5_OFF_SHOULDER_STICK: usize = 9; // L1, R1, L2, R2, share, options, L3, R3
pub const PS5_OFF_META: usize = 10; // home, touchpad, mute, reserved, lf, rf, lp, rp

// Adaptive trigger status (input report, not output effects).
// These let the virtual controller report trigger feedback state
// back to the host, e.g. for UI rendering.
pub const PS5_OFF_TRIGGER_RIGHT_STATUS: usize = 42; // low 4 = stop location, high 4 = status
pub const PS5_OFF_TRIGGER_LEFT_STATUS: usize = 43; // low 4 = stop location, high 4 = status
pub const PS5_OFF_TRIGGER_EFFECT: usize = 48; // low 4 = right effect, high 4 = left effect

pub const PS5_FACE_SQUARE: u8 = 1 << 4;
pub const PS5_FACE_CROSS: u8 = 1 << 5;
pub const PS5_FACE_CIRCLE: u8 = 1 << 6;
pub const PS5_FACE_TRIANGLE: u8 = 1 << 7;

pub const PS5_BTN_L1: u8 = 1 << 0;
pub const PS5_BTN_R1: u8 = 1 << 1;
pub const PS5_BTN_L2: u8 = 1 << 2;
pub const PS5_BTN_R2: u8 = 1 << 3;
pub const PS5_BTN_SHARE: u8 = 1 << 4;
pub const PS5_BTN_OPTIONS: u8 = 1 << 5;
pub const PS5_BTN_L3: u8 = 1 << 6;
pub const PS5_BTN_R3: u8 = 1 << 7;

pub const PS5_BTN_HOME: u8 = 1 << 0;
pub const PS5_BTN_TOUCHPAD: u8 = 1 << 1;
pub const PS5_BTN_MUTE: u8 = 1 << 2;
pub const PS5_BTN_LEFT_FUNCTION: u8 = 1 << 4;
pub const PS5_BTN_RIGHT_FUNCTION: u8 = 1 << 5;
pub const PS5_BTN_LEFT_PADDLE: u8 = 1 << 6;
pub const PS5_BTN_RIGHT_PADDLE: u8 = 1 << 7;

pub type PWINUHID_PS5_RUMBLE_CB =
    unsafe extern "system" fn(ctx: PVOID, left_motor: u8, right_motor: u8);
pub type PWINUHID_PS5_LIGHTBAR_LED_CB =
    unsafe extern "system" fn(ctx: PVOID, led_red: u8, led_green: u8, led_blue: u8);
pub type PWINUHID_PS5_PLAYER_LED_CB = unsafe extern "system" fn(ctx: PVOID, led_value: u8);
pub type PWINUHID_PS5_TRIGGER_EFFECT_CB = unsafe extern "system" fn(
    ctx: PVOID,
    left: *const WINUHID_PS5_TRIGGER_EFFECT,
    right: *const WINUHID_PS5_TRIGGER_EFFECT,
);
pub type PWINUHID_PS5_MIC_LED_CB = unsafe extern "system" fn(ctx: PVOID, led_state: u8);

// ---------------------------------------------------------------------------
// Xbox One preset
// ---------------------------------------------------------------------------

pub type PWINUHID_XONE_GAMEPAD = *mut c_void;

/// Sized to comfortably hold the `WINUHID_XONE_INPUT_REPORT`. The
/// struct is bit-packed; we set fields by byte offset (see
/// `WinUHidXOne.h`).
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct WINUHID_XONE_INPUT_REPORT(pub [u8; 17]);

// Xbox One byte offsets.
//
// MSVC packs bitfields within their declared storage type. Because both
// triggers are `USHORT : 10`, MSVC allocates *two* USHORTs for them
// (10 + 10 > 16). Likewise the trailing UCHAR bitfields each force their
// own UCHAR storage unit.
//
// Resulting layout (matches the size assertion `[u8; 17]`):
//
//   off | field
//  -----+------
//    0  | LeftStickX (u16 LE)
//    2  | LeftStickY
//    4  | RightStickX
//    6  | RightStickY
//    8  | LeftTrigger  (low 10 bits of a u16)
//   10  | RightTrigger (low 10 bits of a u16)
//   12  | A,B,X,Y,LB,RB,Back,Menu  (1 bit each, low to high)
//   13  | LS,RS,(reserved 6)
//   14  | Hat (low 4 bits), reserved 4
//   15  | Home (bit 0), reserved 7
//   16  | BatteryLevel (u8)
pub const XONE_OFF_LX: usize = 0;
pub const XONE_OFF_LY: usize = 2;
pub const XONE_OFF_RX: usize = 4;
pub const XONE_OFF_RY: usize = 6;
pub const XONE_OFF_LT: usize = 8;
pub const XONE_OFF_RT: usize = 10;
pub const XONE_OFF_BTN1: usize = 12;
pub const XONE_OFF_BTN2: usize = 13;
pub const XONE_OFF_HAT: usize = 14;
pub const XONE_OFF_HOME: usize = 15;
pub const XONE_OFF_BATTERY: usize = 16;

pub const XONE_BTN_A: u8 = 1 << 0;
pub const XONE_BTN_B: u8 = 1 << 1;
pub const XONE_BTN_X: u8 = 1 << 2;
pub const XONE_BTN_Y: u8 = 1 << 3;
pub const XONE_BTN_LB: u8 = 1 << 4;
pub const XONE_BTN_RB: u8 = 1 << 5;
pub const XONE_BTN_BACK: u8 = 1 << 6;
pub const XONE_BTN_MENU: u8 = 1 << 7;

pub const XONE_BTN_LS: u8 = 1 << 0;
pub const XONE_BTN_RS: u8 = 1 << 1;

pub const XONE_BTN_HOME: u8 = 1 << 0;

pub type PWINUHID_XONE_FF_CB = unsafe extern "system" fn(
    ctx: PVOID,
    left_motor: u8,
    right_motor: u8,
    left_trigger_motor: u8,
    right_trigger_motor: u8,
);

// ---------------------------------------------------------------------------
// Loaded entry points
// ---------------------------------------------------------------------------

/// Holds open `Library` handles plus typed function pointers.
pub struct Sdk {
    _winuhid: Library,
    _winuhid_devs: Option<Library>,
    pub core: Core,
    pub devs: Option<Devs>,
}

#[allow(clippy::type_complexity)]
pub struct Core {
    pub WinUHidGetDriverInterfaceVersion: unsafe extern "system" fn() -> DWORD,
    pub WinUHidCreateDevice:
        unsafe extern "system" fn(*const WINUHID_DEVICE_CONFIG) -> PWINUHID_DEVICE,
    pub WinUHidSubmitInputReport:
        unsafe extern "system" fn(PWINUHID_DEVICE, LPCVOID, DWORD) -> BOOL,
    pub WinUHidStartDevice:
        unsafe extern "system" fn(PWINUHID_DEVICE, Option<WinUHidEventCallback>, PVOID) -> BOOL,
    pub WinUHidPollEvent: unsafe extern "system" fn(PWINUHID_DEVICE, DWORD) -> PCWINUHID_EVENT,
    pub WinUHidCompleteWriteEvent:
        unsafe extern "system" fn(PWINUHID_DEVICE, PCWINUHID_EVENT, BOOL),
    pub WinUHidCompleteReadEvent:
        unsafe extern "system" fn(PWINUHID_DEVICE, PCWINUHID_EVENT, LPCVOID, DWORD),
    pub WinUHidStopDevice: unsafe extern "system" fn(PWINUHID_DEVICE),
    pub WinUHidDestroyDevice: unsafe extern "system" fn(PWINUHID_DEVICE),
}

#[allow(clippy::type_complexity)]
pub struct Devs {
    // Mouse
    pub WinUHidMouseCreate:
        unsafe extern "system" fn(*const WINUHID_PRESET_DEVICE_INFO) -> PWINUHID_MOUSE_DEVICE,
    pub WinUHidMouseReportMotion:
        unsafe extern "system" fn(PWINUHID_MOUSE_DEVICE, SHORT, SHORT) -> BOOL,
    pub WinUHidMouseReportButton:
        unsafe extern "system" fn(PWINUHID_MOUSE_DEVICE, UCHAR, BOOL) -> BOOL,
    pub WinUHidMouseReportScroll:
        unsafe extern "system" fn(PWINUHID_MOUSE_DEVICE, SHORT, BOOL) -> BOOL,
    pub WinUHidMouseDestroy: unsafe extern "system" fn(PWINUHID_MOUSE_DEVICE),

    // PS4
    pub WinUHidPS4Create: unsafe extern "system" fn(
        *const WINUHID_PS4_GAMEPAD_INFO,
        Option<PWINUHID_PS4_FF_CB>,
        Option<PWINUHID_PS4_LED_CB>,
        PVOID,
    ) -> PWINUHID_PS4_GAMEPAD,
    pub WinUHidPS4InitializeInputReport: unsafe extern "system" fn(*mut WINUHID_PS4_INPUT_REPORT),
    pub WinUHidPS4SetHatState: unsafe extern "system" fn(*mut WINUHID_PS4_INPUT_REPORT, INT, INT),
    pub WinUHidPS4SetBatteryState:
        unsafe extern "system" fn(*mut WINUHID_PS4_INPUT_REPORT, BOOL, UCHAR),
    pub WinUHidPS4SetTouchState:
        unsafe extern "system" fn(*mut WINUHID_PS4_INPUT_REPORT, UCHAR, BOOL, USHORT, USHORT),
    pub WinUHidPS4SetAccelState:
        unsafe extern "system" fn(*mut WINUHID_PS4_INPUT_REPORT, f32, f32, f32),
    pub WinUHidPS4SetGyroState:
        unsafe extern "system" fn(*mut WINUHID_PS4_INPUT_REPORT, f32, f32, f32),
    pub WinUHidPS4ReportInput:
        unsafe extern "system" fn(PWINUHID_PS4_GAMEPAD, *const WINUHID_PS4_INPUT_REPORT) -> BOOL,
    pub WinUHidPS4Destroy: unsafe extern "system" fn(PWINUHID_PS4_GAMEPAD),

    // PS5
    pub WinUHidPS5Create: unsafe extern "system" fn(
        *const WINUHID_PS5_GAMEPAD_INFO,
        Option<PWINUHID_PS5_RUMBLE_CB>,
        Option<PWINUHID_PS5_LIGHTBAR_LED_CB>,
        Option<PWINUHID_PS5_PLAYER_LED_CB>,
        Option<PWINUHID_PS5_TRIGGER_EFFECT_CB>,
        Option<PWINUHID_PS5_MIC_LED_CB>,
        PVOID,
    ) -> PWINUHID_PS5_GAMEPAD,
    pub WinUHidPS5InitializeInputReport: unsafe extern "system" fn(*mut WINUHID_PS5_INPUT_REPORT),
    pub WinUHidPS5SetHatState: unsafe extern "system" fn(*mut WINUHID_PS5_INPUT_REPORT, INT, INT),
    pub WinUHidPS5SetBatteryState:
        unsafe extern "system" fn(*mut WINUHID_PS5_INPUT_REPORT, BOOL, UCHAR),
    pub WinUHidPS5SetTouchState:
        unsafe extern "system" fn(*mut WINUHID_PS5_INPUT_REPORT, UCHAR, BOOL, USHORT, USHORT),
    pub WinUHidPS5SetAccelState:
        unsafe extern "system" fn(*mut WINUHID_PS5_INPUT_REPORT, f32, f32, f32),
    pub WinUHidPS5SetGyroState:
        unsafe extern "system" fn(*mut WINUHID_PS5_INPUT_REPORT, f32, f32, f32),
    pub WinUHidPS5ReportInput:
        unsafe extern "system" fn(PWINUHID_PS5_GAMEPAD, *const WINUHID_PS5_INPUT_REPORT) -> BOOL,
    pub WinUHidPS5Destroy: unsafe extern "system" fn(PWINUHID_PS5_GAMEPAD),

    // Xbox One
    pub WinUHidXOneCreate: unsafe extern "system" fn(
        *const WINUHID_PRESET_DEVICE_INFO,
        Option<PWINUHID_XONE_FF_CB>,
        PVOID,
    ) -> PWINUHID_XONE_GAMEPAD,
    pub WinUHidXOneInitializeInputReport: unsafe extern "system" fn(*mut WINUHID_XONE_INPUT_REPORT),
    pub WinUHidXOneSetHatState: unsafe extern "system" fn(*mut WINUHID_XONE_INPUT_REPORT, INT, INT),
    pub WinUHidXOneReportInput:
        unsafe extern "system" fn(PWINUHID_XONE_GAMEPAD, *const WINUHID_XONE_INPUT_REPORT) -> BOOL,
    pub WinUHidXOneDestroy: unsafe extern "system" fn(PWINUHID_XONE_GAMEPAD),
}

impl Sdk {
    /// Try to load the WinUHid SDK from the given directories (in order).
    /// `WinUHid.dll` is required; `WinUHidDevs.dll` is optional and only
    /// gates the preset-device features in the UI.
    pub fn load(search_dirs: &[PathBuf]) -> Result<Self> {
        let core_path = locate_dll("WinUHid.dll", search_dirs)
            .context("could not locate WinUHid.dll - set WINUHID_DLL_DIR or pass --dll-dir")?;

        // SAFETY: We point libloading at a vetted path string.
        let winuhid = unsafe { Library::new(&core_path) }
            .with_context(|| format!("failed to load {}", core_path.display()))?;

        let core = unsafe { Core::resolve(&winuhid) }
            .with_context(|| format!("missing exports in {}", core_path.display()))?;

        let (devs_lib, devs) = match locate_dll("WinUHidDevs.dll", search_dirs) {
            Some(path) => match unsafe { Library::new(&path) } {
                Ok(lib) => match unsafe { Devs::resolve(&lib) } {
                    Ok(d) => (Some(lib), Some(d)),
                    Err(e) => {
                        tracing::warn!("WinUHidDevs.dll exports incomplete: {e:#}");
                        (None, None)
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "failed to load WinUHidDevs.dll from {}: {e}",
                        path.display()
                    );
                    (None, None)
                }
            },
            None => {
                tracing::info!(
                    "WinUHidDevs.dll not found - preset devices will be unavailable in the UI"
                );
                (None, None)
            }
        };

        Ok(Self {
            _winuhid: winuhid,
            _winuhid_devs: devs_lib,
            core,
            devs,
        })
    }
}

impl Core {
    unsafe fn resolve(lib: &Library) -> Result<Self> {
        unsafe {
            Ok(Self {
                WinUHidGetDriverInterfaceVersion: *raw(lib, b"WinUHidGetDriverInterfaceVersion\0")?,
                WinUHidCreateDevice: *raw(lib, b"WinUHidCreateDevice\0")?,
                WinUHidSubmitInputReport: *raw(lib, b"WinUHidSubmitInputReport\0")?,
                WinUHidStartDevice: *raw(lib, b"WinUHidStartDevice\0")?,
                WinUHidPollEvent: *raw(lib, b"WinUHidPollEvent\0")?,
                WinUHidCompleteWriteEvent: *raw(lib, b"WinUHidCompleteWriteEvent\0")?,
                WinUHidCompleteReadEvent: *raw(lib, b"WinUHidCompleteReadEvent\0")?,
                WinUHidStopDevice: *raw(lib, b"WinUHidStopDevice\0")?,
                WinUHidDestroyDevice: *raw(lib, b"WinUHidDestroyDevice\0")?,
            })
        }
    }
}

impl Devs {
    unsafe fn resolve(lib: &Library) -> Result<Self> {
        unsafe {
            Ok(Self {
                WinUHidMouseCreate: *raw(lib, b"WinUHidMouseCreate\0")?,
                WinUHidMouseReportMotion: *raw(lib, b"WinUHidMouseReportMotion\0")?,
                WinUHidMouseReportButton: *raw(lib, b"WinUHidMouseReportButton\0")?,
                WinUHidMouseReportScroll: *raw(lib, b"WinUHidMouseReportScroll\0")?,
                WinUHidMouseDestroy: *raw(lib, b"WinUHidMouseDestroy\0")?,

                WinUHidPS4Create: *raw(lib, b"WinUHidPS4Create\0")?,
                WinUHidPS4InitializeInputReport: *raw(lib, b"WinUHidPS4InitializeInputReport\0")?,
                WinUHidPS4SetHatState: *raw(lib, b"WinUHidPS4SetHatState\0")?,
                WinUHidPS4SetBatteryState: *raw(lib, b"WinUHidPS4SetBatteryState\0")?,
                WinUHidPS4SetTouchState: *raw(lib, b"WinUHidPS4SetTouchState\0")?,
                WinUHidPS4SetAccelState: *raw(lib, b"WinUHidPS4SetAccelState\0")?,
                WinUHidPS4SetGyroState: *raw(lib, b"WinUHidPS4SetGyroState\0")?,
                WinUHidPS4ReportInput: *raw(lib, b"WinUHidPS4ReportInput\0")?,
                WinUHidPS4Destroy: *raw(lib, b"WinUHidPS4Destroy\0")?,

                WinUHidPS5Create: *raw(lib, b"WinUHidPS5Create\0")?,
                WinUHidPS5InitializeInputReport: *raw(lib, b"WinUHidPS5InitializeInputReport\0")?,
                WinUHidPS5SetHatState: *raw(lib, b"WinUHidPS5SetHatState\0")?,
                WinUHidPS5SetBatteryState: *raw(lib, b"WinUHidPS5SetBatteryState\0")?,
                WinUHidPS5SetTouchState: *raw(lib, b"WinUHidPS5SetTouchState\0")?,
                WinUHidPS5SetAccelState: *raw(lib, b"WinUHidPS5SetAccelState\0")?,
                WinUHidPS5SetGyroState: *raw(lib, b"WinUHidPS5SetGyroState\0")?,
                WinUHidPS5ReportInput: *raw(lib, b"WinUHidPS5ReportInput\0")?,
                WinUHidPS5Destroy: *raw(lib, b"WinUHidPS5Destroy\0")?,

                WinUHidXOneCreate: *raw(lib, b"WinUHidXOneCreate\0")?,
                WinUHidXOneInitializeInputReport: *raw(lib, b"WinUHidXOneInitializeInputReport\0")?,
                WinUHidXOneSetHatState: *raw(lib, b"WinUHidXOneSetHatState\0")?,
                WinUHidXOneReportInput: *raw(lib, b"WinUHidXOneReportInput\0")?,
                WinUHidXOneDestroy: *raw(lib, b"WinUHidXOneDestroy\0")?,
            })
        }
    }
}

unsafe fn raw<'lib, T: Copy>(lib: &'lib Library, name: &[u8]) -> Result<Symbol<'lib, T>> {
    unsafe { lib.get::<T>(name) }.with_context(|| {
        format!(
            "missing export {}",
            std::str::from_utf8(&name[..name.len() - 1]).unwrap_or("<invalid utf8>")
        )
    })
}

fn locate_dll(name: &str, search_dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in search_dirs {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // Fall back to the OS loader (PATH, app dir, system32, ...).
    if Path::new(name).file_name() == Some(OsStr::new(name)) {
        return Some(PathBuf::from(name));
    }
    None
}

// ---------------------------------------------------------------------------
// Helpers for reading a `PCWINUHID_EVENT` returned from the SDK.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ParsedEvent {
    pub kind: WINUHID_EVENT_TYPE,
    pub request_id: WINUHID_REQUEST_ID,
    pub report_id: u8,
    /// For write events this is the data the OS is delivering. For read
    /// events this is empty (the OS is asking *us* for data).
    pub data: Vec<u8>,
    /// For read events this is the requested buffer length; 0 means
    /// "any input report".
    pub read_length: u32,
}

/// Parse a `PCWINUHID_EVENT` produced by `WinUHidPollEvent` /
/// `WINUHID_EVENT_CALLBACK`.
///
/// # Safety
/// `event` must point to a valid `WINUHID_EVENT` for the duration of
/// the call.
pub unsafe fn parse_event(event: PCWINUHID_EVENT) -> ParsedEvent {
    let header = unsafe { &*(event as *const WINUHID_EVENT_HEADER) };
    // Copy out potentially-unaligned packed fields.
    let kind = unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(header.Type)) };
    let request_id = unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(header.RequestId)) };
    let report_id = unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(header.ReportId)) };

    let body_ptr = unsafe { (event as *const u8).add(std::mem::size_of::<WINUHID_EVENT_HEADER>()) };

    if kind == WINUHID_EVENT_SET_FEATURE || kind == WINUHID_EVENT_WRITE_REPORT {
        let data_len = unsafe { std::ptr::read_unaligned(body_ptr as *const u32) };
        let data_ptr = unsafe { body_ptr.add(std::mem::size_of::<u32>()) };
        let data = unsafe { std::slice::from_raw_parts(data_ptr, data_len as usize).to_vec() };
        ParsedEvent {
            kind,
            request_id,
            report_id,
            data,
            read_length: 0,
        }
    } else {
        let data_len = unsafe { std::ptr::read_unaligned(body_ptr as *const u32) };
        ParsedEvent {
            kind,
            request_id,
            report_id,
            data: Vec::new(),
            read_length: data_len,
        }
    }
}

pub fn is_read(kind: WINUHID_EVENT_TYPE) -> bool {
    kind == WINUHID_EVENT_GET_FEATURE || kind == WINUHID_EVENT_READ_REPORT
}

pub fn event_label(kind: WINUHID_EVENT_TYPE) -> &'static str {
    match kind {
        WINUHID_EVENT_GET_FEATURE => "GET_FEATURE",
        WINUHID_EVENT_SET_FEATURE => "SET_FEATURE",
        WINUHID_EVENT_WRITE_REPORT => "WRITE_REPORT",
        WINUHID_EVENT_READ_REPORT => "READ_REPORT",
        _ => "UNKNOWN",
    }
}

/// Last Win32 error, formatted.
pub fn last_win32_error(label: &str) -> anyhow::Error {
    #[cfg(windows)]
    {
        let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        anyhow!("{label} failed: GetLastError = 0x{code:08X}")
    }
    #[cfg(not(windows))]
    {
        anyhow!("{label} failed (non-Windows build)")
    }
}

pub fn last_win32_error_code() -> u32 {
    #[cfg(windows)]
    {
        unsafe { windows_sys::Win32::Foundation::GetLastError() }
    }
    #[cfg(not(windows))]
    {
        0
    }
}

pub fn clear_last_win32_error() {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::Foundation::SetLastError(0);
    }
}
