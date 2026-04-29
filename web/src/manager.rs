//! Device manager — owns the SDK, tracks live virtual devices, and
//! plumbs FFI callbacks into tokio broadcast channels.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::{anyhow, bail, Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::ffi;

/// Maximum number of buffered events per device's broadcast channel
/// before laggy subscribers start receiving `RecvError::Lagged`.
const EVENT_CHANNEL_CAPACITY: usize = 256;

// ---------------------------------------------------------------------------
// Public message schema (also used over the WebSocket).
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceEvent {
    /// The OS sent a HID write/set-feature/get-feature/read-report event.
    HidEvent {
        ts_ms: u64,
        kind: String,
        report_id: u8,
        /// Hex-encoded data (write-direction only).
        data_hex: String,
    },
    /// A preset device received force-feedback motor state from the OS.
    Rumble {
        ts_ms: u64,
        left: u8,
        right: u8,
        /// Optional impulse triggers (Xbox One).
        left_trigger: Option<u8>,
        right_trigger: Option<u8>,
    },
    /// A preset device received an LED state change from the OS.
    Led {
        ts_ms: u64,
        red: u8,
        green: u8,
        blue: u8,
    },
    /// PS5 player-index LED.
    PlayerLed { ts_ms: u64, value: u8 },
    /// PS5 mic mute LED state (0=off, 1=on, 2=pulse).
    MicLed { ts_ms: u64, state: u8 },
    /// PS5 adaptive-trigger effect change.
    TriggerEffect {
        ts_ms: u64,
        left_kind: Option<u8>,
        left_data_hex: Option<String>,
        right_kind: Option<u8>,
        right_data_hex: Option<String>,
    },
    /// Snapshot of the current input report we are pushing to the OS.
    InputSnapshot { ts_ms: u64, hex: String },
    /// Diagnostic / error message.
    Diag {
        ts_ms: u64,
        level: String,
        msg: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Generic,
    Mouse,
    Ps4,
    Ps5,
    XOne,
}

#[derive(Serialize, Debug, Clone)]
pub struct DeviceSummary {
    pub id: Uuid,
    pub kind: DeviceKind,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub created_ts_ms: u64,
}

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

pub struct Manager {
    pub sdk: Arc<ffi::Sdk>,
    devices: Mutex<HashMap<Uuid, Arc<Device>>>,
}

impl Manager {
    pub fn new(sdk: Arc<ffi::Sdk>) -> Self {
        Self {
            sdk,
            devices: Mutex::new(HashMap::new()),
        }
    }

    pub fn driver_version(&self) -> u32 {
        unsafe { (self.sdk.core.WinUHidGetDriverInterfaceVersion)() }
    }

    pub fn devs_available(&self) -> bool {
        self.sdk.devs.is_some()
    }

    pub fn list(&self) -> Vec<DeviceSummary> {
        let g = self.devices.lock();
        let mut v: Vec<_> = g.values().map(|d| d.summary()).collect();
        v.sort_by_key(|s| s.created_ts_ms);
        v
    }

    pub fn get(&self, id: Uuid) -> Option<Arc<Device>> {
        self.devices.lock().get(&id).cloned()
    }

    pub fn destroy(&self, id: Uuid) -> Result<()> {
        let dev = self
            .devices
            .lock()
            .remove(&id)
            .ok_or_else(|| anyhow!("no such device"))?;
        // The Device's Drop impl handles the SDK calls.
        drop(dev);
        Ok(())
    }

    pub fn destroy_all(&self) {
        let mut g = self.devices.lock();
        g.clear();
    }

    fn insert(&self, dev: Device) -> Arc<Device> {
        let arc = Arc::new(dev);
        self.devices.lock().insert(arc.id, arc.clone());
        arc
    }

    // ---- Generic device ----------------------------------------------------

    pub fn create_generic(&self, params: GenericDeviceParams) -> Result<DeviceSummary> {
        let report_descriptor = hex::decode(params.report_descriptor_hex.trim())
            .context("report_descriptor_hex is not valid hex")?;
        if report_descriptor.is_empty() {
            bail!("report_descriptor_hex must not be empty");
        }
        if report_descriptor.len() > u16::MAX as usize {
            bail!("report_descriptor too large");
        }

        // The callback context lives as long as the device.
        let ctx: Box<GenericCtx> = Box::new(GenericCtx {
            sdk: self.sdk.clone(),
            tx: broadcast::channel(EVENT_CHANNEL_CAPACITY).0,
            current_inputs: Mutex::new(HashMap::new()),
            last_report_id: Mutex::new(None),
            handle: Mutex::new(std::ptr::null_mut()),
        });

        let mut cfg = ffi::WINUHID_DEVICE_CONFIG {
            SupportedEvents: ffi::WINUHID_EVENT_GET_FEATURE
                | ffi::WINUHID_EVENT_SET_FEATURE
                | ffi::WINUHID_EVENT_WRITE_REPORT,
            VendorID: params.vendor_id,
            ProductID: params.product_id,
            VersionNumber: params.version,
            ReportDescriptorLength: report_descriptor.len() as u16,
            ReportDescriptor: report_descriptor.as_ptr() as ffi::LPCVOID,
            ContainerId: ffi::GUID::default(),
            InstanceID: std::ptr::null(),
            HardwareIDs: std::ptr::null(),
            ReadReportPeriodUs: 0,
        };
        if params.enable_read_events {
            cfg.SupportedEvents |= ffi::WINUHID_EVENT_READ_REPORT;
            cfg.ReadReportPeriodUs = params.read_report_period_us.unwrap_or(8_000);
        }

        let handle = unsafe { (self.sdk.core.WinUHidCreateDevice)(&cfg) };
        if handle.is_null() {
            return Err(ffi::last_win32_error("WinUHidCreateDevice"));
        }
        *ctx.handle.lock() = handle;

        // Box::into_raw transfers ownership to the FFI side.
        let ctx_ptr: *mut GenericCtx = Box::into_raw(ctx);

        let ok = unsafe {
            (self.sdk.core.WinUHidStartDevice)(
                handle,
                Some(generic_event_callback),
                ctx_ptr as *mut c_void,
            )
        };
        if ok == 0 {
            // Reclaim and free.
            unsafe {
                (self.sdk.core.WinUHidDestroyDevice)(handle);
                drop(Box::from_raw(ctx_ptr));
            }
            return Err(ffi::last_win32_error("WinUHidStartDevice"));
        }

        let id = Uuid::new_v4();
        let device = Device {
            id,
            kind: DeviceKind::Generic,
            name: params.name.unwrap_or_else(|| "Generic HID".to_string()),
            vendor_id: params.vendor_id,
            product_id: params.product_id,
            created_ts_ms: now_ms(),
            tx: unsafe { (*ctx_ptr).tx.clone() },
            sdk: self.sdk.clone(),
            inner: DeviceInner::Generic {
                handle: SendPtr(handle),
                ctx_ptr: SendPtrMut(ctx_ptr),
                report_descriptor,
            },
        };
        let summary = device.summary();
        self.insert(device);
        Ok(summary)
    }

    pub fn submit_generic_input(&self, id: Uuid, report_hex: &str) -> Result<()> {
        let dev = self.get(id).ok_or_else(|| anyhow!("no such device"))?;
        let report = hex::decode(report_hex.trim()).context("invalid hex")?;
        if report.is_empty() {
            bail!("input report must not be empty");
        }
        match &dev.inner {
            DeviceInner::Generic {
                handle, ctx_ptr, ..
            } => {
                let ok = unsafe {
                    (self.sdk.core.WinUHidSubmitInputReport)(
                        handle.0,
                        report.as_ptr() as ffi::LPCVOID,
                        report.len() as u32,
                    )
                };
                if ok == 0 {
                    return Err(ffi::last_win32_error("WinUHidSubmitInputReport"));
                }
                // Only after a successful submit do we cache the report
                // so the read-event callback can replay it. Numbered
                // reports use the first byte as the key; un-numbered
                // reports go under key 0.
                let report_id = report.first().copied().unwrap_or(0);
                unsafe {
                    let ctx = &*ctx_ptr.0;
                    ctx.current_inputs.lock().insert(report_id, report.clone());
                    *ctx.last_report_id.lock() = Some(report_id);
                }
                let _ = dev.tx.send(DeviceEvent::InputSnapshot {
                    ts_ms: now_ms(),
                    hex: hex::encode(&report),
                });
                Ok(())
            }
            _ => bail!("device is not a generic HID device"),
        }
    }

    // ---- Mouse preset ------------------------------------------------------

    pub fn create_mouse(&self, name: Option<String>) -> Result<DeviceSummary> {
        let devs = self
            .sdk
            .devs
            .as_ref()
            .context("WinUHidDevs.dll not loaded")?;
        let handle = unsafe { (devs.WinUHidMouseCreate)(std::ptr::null()) };
        if handle.is_null() {
            return Err(ffi::last_win32_error("WinUHidMouseCreate"));
        }
        let id = Uuid::new_v4();
        let (tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let device = Device {
            id,
            kind: DeviceKind::Mouse,
            name: name.unwrap_or_else(|| "Mouse".to_string()),
            // Microsoft Precision Mouse VID/PID per WinUHidMouse.cpp (typical default).
            vendor_id: 0x045E,
            product_id: 0x0823,
            created_ts_ms: now_ms(),
            tx,
            sdk: self.sdk.clone(),
            inner: DeviceInner::Mouse {
                handle: SendPtr(handle),
            },
        };
        let summary = device.summary();
        self.insert(device);
        Ok(summary)
    }

    pub fn mouse_motion(&self, id: Uuid, dx: i16, dy: i16) -> Result<()> {
        let dev = self.get(id).ok_or_else(|| anyhow!("no such device"))?;
        let devs = self
            .sdk
            .devs
            .as_ref()
            .context("WinUHidDevs.dll not loaded")?;
        if let DeviceInner::Mouse { handle } = &dev.inner {
            if unsafe { (devs.WinUHidMouseReportMotion)(handle.0, dx, dy) } == 0 {
                return Err(ffi::last_win32_error("WinUHidMouseReportMotion"));
            }
            Ok(())
        } else {
            bail!("device is not a mouse")
        }
    }

    pub fn mouse_button(&self, id: Uuid, button: u8, down: bool) -> Result<()> {
        let dev = self.get(id).ok_or_else(|| anyhow!("no such device"))?;
        let devs = self
            .sdk
            .devs
            .as_ref()
            .context("WinUHidDevs.dll not loaded")?;
        if let DeviceInner::Mouse { handle } = &dev.inner {
            let bool_val: ffi::BOOL = if down { 1 } else { 0 };
            if unsafe { (devs.WinUHidMouseReportButton)(handle.0, button, bool_val) } == 0 {
                return Err(ffi::last_win32_error("WinUHidMouseReportButton"));
            }
            Ok(())
        } else {
            bail!("device is not a mouse")
        }
    }

    pub fn mouse_scroll(&self, id: Uuid, value: i16, horizontal: bool) -> Result<()> {
        let dev = self.get(id).ok_or_else(|| anyhow!("no such device"))?;
        let devs = self
            .sdk
            .devs
            .as_ref()
            .context("WinUHidDevs.dll not loaded")?;
        if let DeviceInner::Mouse { handle } = &dev.inner {
            let bool_val: ffi::BOOL = if horizontal { 1 } else { 0 };
            if unsafe { (devs.WinUHidMouseReportScroll)(handle.0, value, bool_val) } == 0 {
                return Err(ffi::last_win32_error("WinUHidMouseReportScroll"));
            }
            Ok(())
        } else {
            bail!("device is not a mouse")
        }
    }

    // ---- PS4 preset --------------------------------------------------------

    pub fn create_ps4(&self, name: Option<String>) -> Result<DeviceSummary> {
        let devs = self
            .sdk
            .devs
            .as_ref()
            .context("WinUHidDevs.dll not loaded")?;
        let (tx, _) = broadcast::channel::<DeviceEvent>(EVENT_CHANNEL_CAPACITY);
        let ctx: Box<PresetFeedbackCtx> = Box::new(PresetFeedbackCtx { tx: tx.clone() });
        let info = ffi::WINUHID_PS4_GAMEPAD_INFO {
            BasicInfo: std::ptr::null(),
            MacAddress: random_mac(),
        };
        let ctx_ptr = Box::into_raw(ctx);
        let handle = unsafe {
            (devs.WinUHidPS4Create)(
                &info,
                Some(ps4_rumble_callback),
                Some(ps4_led_callback),
                ctx_ptr as *mut c_void,
            )
        };
        if handle.is_null() {
            unsafe { drop(Box::from_raw(ctx_ptr)) };
            return Err(ffi::last_win32_error("WinUHidPS4Create"));
        }

        let id = Uuid::new_v4();
        let device = Device {
            id,
            kind: DeviceKind::Ps4,
            name: name.unwrap_or_else(|| "DualShock 4".to_string()),
            vendor_id: 0x054C,
            product_id: 0x09CC,
            created_ts_ms: now_ms(),
            tx,
            sdk: self.sdk.clone(),
            inner: DeviceInner::Ps4 {
                handle: SendPtr(handle),
                ctx_ptr: SendPtrMut(ctx_ptr),
                report: Mutex::new(default_ps4_report(devs)),
            },
        };
        let summary = device.summary();
        self.insert(device);
        Ok(summary)
    }

    pub fn submit_ps4_state(&self, id: Uuid, state: &Ps4State) -> Result<()> {
        let dev = self.get(id).ok_or_else(|| anyhow!("no such device"))?;
        let devs = self
            .sdk
            .devs
            .as_ref()
            .context("WinUHidDevs.dll not loaded")?;
        if let DeviceInner::Ps4 { handle, report, .. } = &dev.inner {
            let mut r = report.lock();
            // Hat (SDK helper handles encoding) — done first so that the
            // pure-Rust byte packer can preserve the low nibble it wrote.
            unsafe {
                (devs.WinUHidPS4SetHatState)(&mut *r, state.hat_x as i32, state.hat_y as i32);
            }
            apply_ps4_state_bytes(&mut r, state);
            unsafe {
                (devs.WinUHidPS4SetTouchState)(
                    &mut *r,
                    0,
                    b(state.touchpad_active),
                    state.touchpad_x.min(1919),
                    state.touchpad_y.min(942),
                );
                (devs.WinUHidPS4SetTouchState)(
                    &mut *r,
                    1,
                    b(state.touchpad2_active),
                    state.touchpad2_x.min(1919),
                    state.touchpad2_y.min(942),
                );
                (devs.WinUHidPS4SetAccelState)(
                    &mut *r,
                    state.accel_x,
                    state.accel_y,
                    state.accel_z,
                );
                (devs.WinUHidPS4SetGyroState)(&mut *r, state.gyro_x, state.gyro_y, state.gyro_z);
            }
            unsafe {
                (devs.WinUHidPS4ReportInput)(handle.0, &*r as *const _);
            }
            let _ = dev.tx.send(DeviceEvent::InputSnapshot {
                ts_ms: now_ms(),
                hex: hex::encode(struct_as_bytes(&*r)),
            });
            Ok(())
        } else {
            bail!("device is not a PS4 gamepad");
        }
    }

    // ---- PS5 preset --------------------------------------------------------

    pub fn create_ps5(&self, name: Option<String>) -> Result<DeviceSummary> {
        let devs = self
            .sdk
            .devs
            .as_ref()
            .context("WinUHidDevs.dll not loaded")?;
        let (tx, _) = broadcast::channel::<DeviceEvent>(EVENT_CHANNEL_CAPACITY);
        let ctx: Box<PresetFeedbackCtx> = Box::new(PresetFeedbackCtx { tx: tx.clone() });
        let info = ffi::WINUHID_PS5_GAMEPAD_INFO {
            BasicInfo: std::ptr::null(),
            MacAddress: random_mac(),
            FirmwareInfo: std::ptr::null(),
            FirmwareInfoLength: 0,
        };
        let ctx_ptr = Box::into_raw(ctx);
        let handle = unsafe {
            (devs.WinUHidPS5Create)(
                &info,
                Some(ps5_rumble_callback),
                Some(ps5_lightbar_callback),
                Some(ps5_player_led_callback),
                Some(ps5_trigger_effect_callback),
                Some(ps5_mic_led_callback),
                ctx_ptr as *mut c_void,
            )
        };
        if handle.is_null() {
            unsafe { drop(Box::from_raw(ctx_ptr)) };
            return Err(ffi::last_win32_error("WinUHidPS5Create"));
        }
        let mut report = ffi::WINUHID_PS5_INPUT_REPORT::default();
        unsafe { (devs.WinUHidPS5InitializeInputReport)(&mut report) };

        let id = Uuid::new_v4();
        let device = Device {
            id,
            kind: DeviceKind::Ps5,
            name: name.unwrap_or_else(|| "DualSense".to_string()),
            vendor_id: 0x054C,
            product_id: 0x0CE6,
            created_ts_ms: now_ms(),
            tx,
            sdk: self.sdk.clone(),
            inner: DeviceInner::Ps5 {
                handle: SendPtr(handle),
                ctx_ptr: SendPtrMut(ctx_ptr),
                report: Mutex::new(report),
            },
        };
        let summary = device.summary();
        self.insert(device);
        Ok(summary)
    }

    pub fn submit_ps5_state(&self, id: Uuid, state: &Ps5State) -> Result<()> {
        let dev = self.get(id).ok_or_else(|| anyhow!("no such device"))?;
        let devs = self
            .sdk
            .devs
            .as_ref()
            .context("WinUHidDevs.dll not loaded")?;
        if let DeviceInner::Ps5 { handle, report, .. } = &dev.inner {
            let mut r = report.lock();
            // Hat first (the SDK helper takes a &mut WINUHID_PS5_INPUT_REPORT
            // and writes into the hat nibble of the same byte we'll
            // OR-merge below).
            unsafe {
                (devs.WinUHidPS5SetHatState)(&mut *r, state.hat_x as i32, state.hat_y as i32);
            }
            apply_ps5_state_bytes(&mut r, state);
            unsafe {
                (devs.WinUHidPS5SetTouchState)(
                    &mut *r,
                    0,
                    b(state.touchpad_active),
                    state.touchpad_x.min(1919),
                    state.touchpad_y.min(1079),
                );
                (devs.WinUHidPS5SetTouchState)(
                    &mut *r,
                    1,
                    b(state.touchpad2_active),
                    state.touchpad2_x.min(1919),
                    state.touchpad2_y.min(1079),
                );
                (devs.WinUHidPS5SetAccelState)(
                    &mut *r,
                    state.accel_x,
                    state.accel_y,
                    state.accel_z,
                );
                (devs.WinUHidPS5SetGyroState)(&mut *r, state.gyro_x, state.gyro_y, state.gyro_z);
            }
            unsafe {
                (devs.WinUHidPS5ReportInput)(handle.0, &*r as *const _);
            }
            let _ = dev.tx.send(DeviceEvent::InputSnapshot {
                ts_ms: now_ms(),
                hex: hex::encode(&r.0),
            });
            Ok(())
        } else {
            bail!("device is not a PS5 gamepad");
        }
    }

    // ---- Xbox One preset ---------------------------------------------------

    pub fn create_xone(&self, name: Option<String>) -> Result<DeviceSummary> {
        let devs = self
            .sdk
            .devs
            .as_ref()
            .context("WinUHidDevs.dll not loaded")?;
        let (tx, _) = broadcast::channel::<DeviceEvent>(EVENT_CHANNEL_CAPACITY);
        let ctx: Box<PresetFeedbackCtx> = Box::new(PresetFeedbackCtx { tx: tx.clone() });
        let ctx_ptr = Box::into_raw(ctx);
        let handle = unsafe {
            (devs.WinUHidXOneCreate)(
                std::ptr::null(),
                Some(xone_ff_callback),
                ctx_ptr as *mut c_void,
            )
        };
        if handle.is_null() {
            unsafe { drop(Box::from_raw(ctx_ptr)) };
            return Err(ffi::last_win32_error("WinUHidXOneCreate"));
        }
        let mut report = ffi::WINUHID_XONE_INPUT_REPORT::default();
        unsafe { (devs.WinUHidXOneInitializeInputReport)(&mut report) };

        let id = Uuid::new_v4();
        let device = Device {
            id,
            kind: DeviceKind::XOne,
            name: name.unwrap_or_else(|| "Xbox One Controller".to_string()),
            vendor_id: 0x045E,
            product_id: 0x02E0,
            created_ts_ms: now_ms(),
            tx,
            sdk: self.sdk.clone(),
            inner: DeviceInner::XOne {
                handle: SendPtr(handle),
                ctx_ptr: SendPtrMut(ctx_ptr),
                report: Mutex::new(report),
            },
        };
        let summary = device.summary();
        self.insert(device);
        Ok(summary)
    }

    pub fn submit_xone_state(&self, id: Uuid, state: &XOneState) -> Result<()> {
        let dev = self.get(id).ok_or_else(|| anyhow!("no such device"))?;
        let devs = self
            .sdk
            .devs
            .as_ref()
            .context("WinUHidDevs.dll not loaded")?;
        if let DeviceInner::XOne { handle, report, .. } = &dev.inner {
            let mut r = report.lock();
            unsafe {
                (devs.WinUHidXOneSetHatState)(&mut *r, state.hat_x as i32, state.hat_y as i32);
            }
            apply_xone_state_bytes(&mut r, state);
            unsafe {
                (devs.WinUHidXOneReportInput)(handle.0, &*r as *const _);
            }
            let _ = dev.tx.send(DeviceEvent::InputSnapshot {
                ts_ms: now_ms(),
                hex: hex::encode(&r.0),
            });
            Ok(())
        } else {
            bail!("device is not an Xbox One gamepad");
        }
    }
}

// ---------------------------------------------------------------------------
// Per-device data
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone)]
pub struct GenericDeviceParams {
    pub name: Option<String>,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    pub report_descriptor_hex: String,
    /// If true, the OS-driven READ_REPORT events are surfaced to the UI.
    /// Required for devices that the OS polls aggressively.
    #[serde(default)]
    pub enable_read_events: bool,
    pub read_report_period_us: Option<u32>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Ps4State {
    pub left_stick_x: u8,
    pub left_stick_y: u8,
    pub right_stick_x: u8,
    pub right_stick_y: u8,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub hat_x: i8,
    pub hat_y: i8,
    pub btn_square: bool,
    pub btn_cross: bool,
    pub btn_circle: bool,
    pub btn_triangle: bool,
    pub btn_l1: bool,
    pub btn_r1: bool,
    pub btn_l2: bool,
    pub btn_r2: bool,
    pub btn_share: bool,
    pub btn_options: bool,
    pub btn_l3: bool,
    pub btn_r3: bool,
    pub btn_home: bool,
    pub btn_touchpad: bool,
    #[serde(default)]
    pub touchpad_active: bool,
    #[serde(default)]
    pub touchpad_x: u16,
    #[serde(default)]
    pub touchpad_y: u16,
    #[serde(default)]
    pub touchpad2_active: bool,
    #[serde(default)]
    pub touchpad2_x: u16,
    #[serde(default)]
    pub touchpad2_y: u16,
    #[serde(default)]
    pub accel_x: f32,
    #[serde(default)]
    pub accel_y: f32,
    #[serde(default)]
    pub accel_z: f32,
    #[serde(default)]
    pub gyro_x: f32,
    #[serde(default)]
    pub gyro_y: f32,
    #[serde(default)]
    pub gyro_z: f32,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Ps5State {
    pub left_stick_x: u8,
    pub left_stick_y: u8,
    pub right_stick_x: u8,
    pub right_stick_y: u8,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub hat_x: i8,
    pub hat_y: i8,
    pub btn_square: bool,
    pub btn_cross: bool,
    pub btn_circle: bool,
    pub btn_triangle: bool,
    pub btn_l1: bool,
    pub btn_r1: bool,
    pub btn_l2: bool,
    pub btn_r2: bool,
    pub btn_share: bool,
    pub btn_options: bool,
    pub btn_l3: bool,
    pub btn_r3: bool,
    pub btn_home: bool,
    pub btn_touchpad: bool,
    pub btn_mute: bool,
    #[serde(default)]
    pub touchpad_active: bool,
    #[serde(default)]
    pub touchpad_x: u16,
    #[serde(default)]
    pub touchpad_y: u16,
    #[serde(default)]
    pub touchpad2_active: bool,
    #[serde(default)]
    pub touchpad2_x: u16,
    #[serde(default)]
    pub touchpad2_y: u16,
    #[serde(default)]
    pub accel_x: f32,
    #[serde(default)]
    pub accel_y: f32,
    #[serde(default)]
    pub accel_z: f32,
    #[serde(default)]
    pub gyro_x: f32,
    #[serde(default)]
    pub gyro_y: f32,
    #[serde(default)]
    pub gyro_z: f32,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct XOneState {
    pub left_stick_x: u16,
    pub left_stick_y: u16,
    pub right_stick_x: u16,
    pub right_stick_y: u16,
    pub left_trigger: u16,  // 0..1023
    pub right_trigger: u16, // 0..1023
    pub hat_x: i8,
    pub hat_y: i8,
    pub btn_a: bool,
    pub btn_b: bool,
    pub btn_x: bool,
    pub btn_y: bool,
    pub btn_lb: bool,
    pub btn_rb: bool,
    pub btn_back: bool,
    pub btn_menu: bool,
    pub btn_ls: bool,
    pub btn_rs: bool,
    pub btn_home: bool,
    pub battery_level: u8,
}

pub struct Device {
    pub id: Uuid,
    pub kind: DeviceKind,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub created_ts_ms: u64,
    pub tx: broadcast::Sender<DeviceEvent>,
    /// Held so `Drop` can call SDK destroy functions without relying on
    /// process-wide globals.
    sdk: Arc<ffi::Sdk>,
    inner: DeviceInner,
}

impl Device {
    pub fn summary(&self) -> DeviceSummary {
        DeviceSummary {
            id: self.id,
            kind: self.kind,
            name: self.name.clone(),
            vendor_id: self.vendor_id,
            product_id: self.product_id,
            created_ts_ms: self.created_ts_ms,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DeviceEvent> {
        self.tx.subscribe()
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // The native SDK guarantees that `WinUHid*Destroy` waits for any
        // worker / callback threads it spawned to exit before returning,
        // so it is safe to free our boxed callback contexts immediately
        // afterwards.
        match &self.inner {
            DeviceInner::Generic {
                handle, ctx_ptr, ..
            } => unsafe {
                (self.sdk.core.WinUHidStopDevice)(handle.0);
                (self.sdk.core.WinUHidDestroyDevice)(handle.0);
                drop(Box::from_raw(ctx_ptr.0));
            },
            DeviceInner::Mouse { handle } => {
                if let Some(devs) = self.sdk.devs.as_ref() {
                    unsafe { (devs.WinUHidMouseDestroy)(handle.0) };
                }
            }
            DeviceInner::Ps4 {
                handle, ctx_ptr, ..
            } => {
                if let Some(devs) = self.sdk.devs.as_ref() {
                    unsafe { (devs.WinUHidPS4Destroy)(handle.0) };
                }
                unsafe { drop(Box::from_raw(ctx_ptr.0)) };
            }
            DeviceInner::Ps5 {
                handle, ctx_ptr, ..
            } => {
                if let Some(devs) = self.sdk.devs.as_ref() {
                    unsafe { (devs.WinUHidPS5Destroy)(handle.0) };
                }
                unsafe { drop(Box::from_raw(ctx_ptr.0)) };
            }
            DeviceInner::XOne {
                handle, ctx_ptr, ..
            } => {
                if let Some(devs) = self.sdk.devs.as_ref() {
                    unsafe { (devs.WinUHidXOneDestroy)(handle.0) };
                }
                unsafe { drop(Box::from_raw(ctx_ptr.0)) };
            }
        }
    }
}

enum DeviceInner {
    Generic {
        handle: SendPtr<c_void>,
        ctx_ptr: SendPtrMut<GenericCtx>,
        #[allow(dead_code)]
        report_descriptor: Vec<u8>,
    },
    Mouse {
        handle: SendPtr<c_void>,
    },
    Ps4 {
        handle: SendPtr<c_void>,
        ctx_ptr: SendPtrMut<PresetFeedbackCtx>,
        report: Mutex<ffi::WINUHID_PS4_INPUT_REPORT>,
    },
    Ps5 {
        handle: SendPtr<c_void>,
        ctx_ptr: SendPtrMut<PresetFeedbackCtx>,
        report: Mutex<ffi::WINUHID_PS5_INPUT_REPORT>,
    },
    XOne {
        handle: SendPtr<c_void>,
        ctx_ptr: SendPtrMut<PresetFeedbackCtx>,
        report: Mutex<ffi::WINUHID_XONE_INPUT_REPORT>,
    },
}

// ---------------------------------------------------------------------------
// FFI callback bookkeeping
// ---------------------------------------------------------------------------

/// Per-generic-device callback context. The SDK stores a pointer to one
/// of these and gives it back to us in the event callback.
struct GenericCtx {
    sdk: Arc<ffi::Sdk>,
    tx: broadcast::Sender<DeviceEvent>,
    /// Last user-submitted report per report ID (key 0 = un-numbered).
    current_inputs: Mutex<HashMap<u8, Vec<u8>>>,
    /// Most recent report id we successfully submitted (for "any" reads).
    last_report_id: Mutex<Option<u8>>,
    handle: Mutex<ffi::PWINUHID_DEVICE>,
}

struct PresetFeedbackCtx {
    tx: broadcast::Sender<DeviceEvent>,
}

unsafe extern "system" fn generic_event_callback(
    callback_ctx: ffi::PVOID,
    _device: ffi::PWINUHID_DEVICE,
    event: ffi::PCWINUHID_EVENT,
) {
    let ctx = unsafe { &*(callback_ctx as *const GenericCtx) };
    let parsed = unsafe { ffi::parse_event(event) };
    let handle = *ctx.handle.lock();

    // Complete the event first — the SDK callback runs on a
    // time-critical thread and we don't want hex encoding / channel
    // sends sitting between us and `WinUHidCompleteWriteEvent`.
    if ffi::is_read(parsed.kind) {
        let snapshot = pick_read_snapshot(&ctx.current_inputs, &ctx.last_report_id, &parsed);
        let (ptr, len) = match snapshot.as_ref() {
            Some(buf) => (buf.as_ptr() as ffi::LPCVOID, buf.len() as u32),
            None => (std::ptr::null(), 0u32),
        };
        unsafe { (ctx.sdk.core.WinUHidCompleteReadEvent)(handle, event, ptr, len) };
    } else {
        unsafe { (ctx.sdk.core.WinUHidCompleteWriteEvent)(handle, event, 1) };
    }

    // Now (best-effort) forward the event to UI subscribers.
    let send_data_hex = if parsed.data.is_empty() {
        String::new()
    } else {
        hex::encode(&parsed.data)
    };
    let _ = ctx.tx.send(DeviceEvent::HidEvent {
        ts_ms: now_ms(),
        kind: ffi::event_label(parsed.kind).to_string(),
        report_id: parsed.report_id,
        data_hex: send_data_hex,
    });
}

/// Decide what bytes to send back for a `READ_REPORT` / `GET_FEATURE`.
///
/// 1. If the OS asked for a specific report id we have submitted, return
///    that exact report.
/// 2. If we have nothing for that id but the OS did request a specific
///    `read_length`, synthesize a zero-filled report of that size (with
///    byte 0 set to the report id for numbered reports).
/// 3. If the OS sent `report_id == 0 && read_length == 0` (the
///    "any input report" flavour), fall back to the most recently
///    submitted report we know of.
/// 4. As a last resort, return `None` so the SDK fails the request.
fn pick_read_snapshot(
    current_inputs: &Mutex<HashMap<u8, Vec<u8>>>,
    last_report_id: &Mutex<Option<u8>>,
    ev: &ffi::ParsedEvent,
) -> Option<Vec<u8>> {
    let map = current_inputs.lock();

    if ev.report_id != 0 {
        if let Some(buf) = map.get(&ev.report_id) {
            return Some(buf.clone());
        }
        if ev.read_length > 0 {
            let mut synth = vec![0u8; ev.read_length as usize];
            synth[0] = ev.report_id;
            return Some(synth);
        }
        return None;
    }

    // report_id == 0
    if let Some(last) = *last_report_id.lock() {
        if let Some(buf) = map.get(&last) {
            return Some(buf.clone());
        }
    }
    if ev.read_length > 0 {
        return Some(vec![0u8; ev.read_length as usize]);
    }
    None
}

unsafe extern "system" fn ps4_rumble_callback(callback_ctx: ffi::PVOID, left: u8, right: u8) {
    let ctx = unsafe { &*(callback_ctx as *const PresetFeedbackCtx) };
    let _ = ctx.tx.send(DeviceEvent::Rumble {
        ts_ms: now_ms(),
        left,
        right,
        left_trigger: None,
        right_trigger: None,
    });
}

unsafe extern "system" fn ps4_led_callback(callback_ctx: ffi::PVOID, red: u8, green: u8, blue: u8) {
    let ctx = unsafe { &*(callback_ctx as *const PresetFeedbackCtx) };
    let _ = ctx.tx.send(DeviceEvent::Led {
        ts_ms: now_ms(),
        red,
        green,
        blue,
    });
}

unsafe extern "system" fn ps5_rumble_callback(callback_ctx: ffi::PVOID, left: u8, right: u8) {
    let ctx = unsafe { &*(callback_ctx as *const PresetFeedbackCtx) };
    let _ = ctx.tx.send(DeviceEvent::Rumble {
        ts_ms: now_ms(),
        left,
        right,
        left_trigger: None,
        right_trigger: None,
    });
}

unsafe extern "system" fn ps5_lightbar_callback(
    callback_ctx: ffi::PVOID,
    red: u8,
    green: u8,
    blue: u8,
) {
    let ctx = unsafe { &*(callback_ctx as *const PresetFeedbackCtx) };
    let _ = ctx.tx.send(DeviceEvent::Led {
        ts_ms: now_ms(),
        red,
        green,
        blue,
    });
}

unsafe extern "system" fn ps5_player_led_callback(callback_ctx: ffi::PVOID, value: u8) {
    let ctx = unsafe { &*(callback_ctx as *const PresetFeedbackCtx) };
    let _ = ctx.tx.send(DeviceEvent::PlayerLed {
        ts_ms: now_ms(),
        value,
    });
}

unsafe extern "system" fn ps5_mic_led_callback(callback_ctx: ffi::PVOID, led_state: u8) {
    let ctx = unsafe { &*(callback_ctx as *const PresetFeedbackCtx) };
    let _ = ctx.tx.send(DeviceEvent::MicLed {
        ts_ms: now_ms(),
        state: led_state,
    });
}

unsafe extern "system" fn ps5_trigger_effect_callback(
    callback_ctx: ffi::PVOID,
    left: *const ffi::WINUHID_PS5_TRIGGER_EFFECT,
    right: *const ffi::WINUHID_PS5_TRIGGER_EFFECT,
) {
    let ctx = unsafe { &*(callback_ctx as *const PresetFeedbackCtx) };
    let (lk, ld) = unsafe { unpack_trigger_effect(left) };
    let (rk, rd) = unsafe { unpack_trigger_effect(right) };
    let _ = ctx.tx.send(DeviceEvent::TriggerEffect {
        ts_ms: now_ms(),
        left_kind: lk,
        left_data_hex: ld,
        right_kind: rk,
        right_data_hex: rd,
    });
}

unsafe fn unpack_trigger_effect(
    p: *const ffi::WINUHID_PS5_TRIGGER_EFFECT,
) -> (Option<u8>, Option<String>) {
    if p.is_null() {
        (None, None)
    } else {
        let kind = unsafe { std::ptr::read_unaligned(std::ptr::addr_of!((*p).Kind)) };
        let data = unsafe { std::ptr::read_unaligned(std::ptr::addr_of!((*p).Data)) };
        (Some(kind), Some(hex::encode(data)))
    }
}

unsafe extern "system" fn xone_ff_callback(
    callback_ctx: ffi::PVOID,
    left: u8,
    right: u8,
    left_trigger: u8,
    right_trigger: u8,
) {
    let ctx = unsafe { &*(callback_ctx as *const PresetFeedbackCtx) };
    let _ = ctx.tx.send(DeviceEvent::Rumble {
        ts_ms: now_ms(),
        left,
        right,
        left_trigger: Some(left_trigger),
        right_trigger: Some(right_trigger),
    });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wrapper around an opaque FFI handle that is documented (by the SDK)
/// as being safe to use from any thread once created. We mark it
/// `Send + Sync` for the specific handle/context types listed below
/// and **only** those — the wrapper is private to this module.
struct SendPtr<T>(*mut T);
struct SendPtrMut<T>(*mut T);

// Generic device handle (`PWINUHID_DEVICE`) — thread-safe per
// `WinUHid.h:130`+. The `c_void` here matches `PWINUHID_DEVICE`.
unsafe impl Send for SendPtr<c_void> {}
unsafe impl Sync for SendPtr<c_void> {}

// Per-device context boxes (we allocate, the SDK only echoes them back).
unsafe impl Send for SendPtrMut<GenericCtx> {}
unsafe impl Sync for SendPtrMut<GenericCtx> {}
unsafe impl Send for SendPtrMut<PresetFeedbackCtx> {}
unsafe impl Sync for SendPtrMut<PresetFeedbackCtx> {}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn random_mac() -> [u8; 6] {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0xC0FFEE);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let bytes = n.to_le_bytes();
    [0x02, 0xCA, 0xFE, bytes[0], bytes[1], bytes[2]]
}

fn default_ps4_report(devs: &ffi::Devs) -> ffi::WINUHID_PS4_INPUT_REPORT {
    let mut r = ffi::WINUHID_PS4_INPUT_REPORT {
        ReportId: 1,
        LeftStickX: 0x80,
        LeftStickY: 0x80,
        RightStickX: 0x80,
        RightStickY: 0x80,
        _packed_hat_face: 0x08, // hat = neutral
        _packed_shoulder_stick: 0,
        _packed_meta: 0,
        LeftTrigger: 0,
        RightTrigger: 0,
        Timestamp: 0,
        BatteryLevel: 0,
        GyroX: 0,
        GyroY: 0,
        GyroZ: 0,
        AccelX: 0,
        AccelY: 0,
        AccelZ: 0,
        Reserved2: [0; 5],
        BatteryLevelSpecial: 0,
        Status: [0; 2],
        TouchReportCount: 0,
        TouchReports: [ffi::Ps4TouchReport {
            Timestamp: 0,
            TouchPoints: [ffi::Ps4TouchPoint {
                ContactSeq: 0,
                XLowPart: 0,
                _packed_x_high_y_low: 0,
                YHighPart: 0,
            }; 2],
        }; 3],
        Reserved3: [0; 3],
    };
    unsafe { (devs.WinUHidPS4InitializeInputReport)(&mut r) };
    r
}

fn struct_as_bytes<T>(s: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts(s as *const T as *const u8, std::mem::size_of::<T>()) }
}

fn b(x: bool) -> ffi::BOOL {
    if x {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Pure byte-mutation helpers for preset gamepad input reports.
//
// These factor out the "mash bits into the right slots" logic from
// `submit_*_state` so it can be exercised by unit tests without going
// anywhere near the WinUHid SDK. Each helper writes everything *except*
// the directional hat — the hat is encoded by an SDK helper
// (`WinUHid{PS4,PS5,XOne}SetHatState`) and that call stays in the
// `submit_*_state` paths.
// ---------------------------------------------------------------------------

pub(crate) fn apply_ps4_state_bytes(r: &mut ffi::WINUHID_PS4_INPUT_REPORT, state: &Ps4State) {
    r.LeftStickX = state.left_stick_x;
    r.LeftStickY = state.left_stick_y;
    r.RightStickX = state.right_stick_x;
    r.RightStickY = state.right_stick_y;
    r.LeftTrigger = state.left_trigger;
    r.RightTrigger = state.right_trigger;

    let face = (if state.btn_square {
        ffi::PS4_FACE_SQUARE
    } else {
        0
    }) | (if state.btn_cross {
        ffi::PS4_FACE_CROSS
    } else {
        0
    }) | (if state.btn_circle {
        ffi::PS4_FACE_CIRCLE
    } else {
        0
    }) | (if state.btn_triangle {
        ffi::PS4_FACE_TRIANGLE
    } else {
        0
    });
    // Preserve hat (low nibble) which the SDK helper already wrote.
    r._packed_hat_face = (r._packed_hat_face & 0x0F) | face;
    r._packed_shoulder_stick = (if state.btn_l1 { ffi::PS4_BTN_L1 } else { 0 })
        | (if state.btn_r1 { ffi::PS4_BTN_R1 } else { 0 })
        | (if state.btn_l2 { ffi::PS4_BTN_L2 } else { 0 })
        | (if state.btn_r2 { ffi::PS4_BTN_R2 } else { 0 })
        | (if state.btn_share {
            ffi::PS4_BTN_SHARE
        } else {
            0
        })
        | (if state.btn_options {
            ffi::PS4_BTN_OPTIONS
        } else {
            0
        })
        | (if state.btn_l3 { ffi::PS4_BTN_L3 } else { 0 })
        | (if state.btn_r3 { ffi::PS4_BTN_R3 } else { 0 });
    r._packed_meta = (r._packed_meta & 0xFC)
        | (if state.btn_home { ffi::PS4_BTN_HOME } else { 0 })
        | (if state.btn_touchpad {
            ffi::PS4_BTN_TOUCHPAD
        } else {
            0
        });
}

pub(crate) fn apply_ps5_state_bytes(r: &mut ffi::WINUHID_PS5_INPUT_REPORT, state: &Ps5State) {
    let face = (if state.btn_square {
        ffi::PS5_FACE_SQUARE
    } else {
        0
    }) | (if state.btn_cross {
        ffi::PS5_FACE_CROSS
    } else {
        0
    }) | (if state.btn_circle {
        ffi::PS5_FACE_CIRCLE
    } else {
        0
    }) | (if state.btn_triangle {
        ffi::PS5_FACE_TRIANGLE
    } else {
        0
    });
    let shoulder = (if state.btn_l1 { ffi::PS5_BTN_L1 } else { 0 })
        | (if state.btn_r1 { ffi::PS5_BTN_R1 } else { 0 })
        | (if state.btn_l2 { ffi::PS5_BTN_L2 } else { 0 })
        | (if state.btn_r2 { ffi::PS5_BTN_R2 } else { 0 })
        | (if state.btn_share {
            ffi::PS5_BTN_SHARE
        } else {
            0
        })
        | (if state.btn_options {
            ffi::PS5_BTN_OPTIONS
        } else {
            0
        })
        | (if state.btn_l3 { ffi::PS5_BTN_L3 } else { 0 })
        | (if state.btn_r3 { ffi::PS5_BTN_R3 } else { 0 });
    let meta = (if state.btn_home { ffi::PS5_BTN_HOME } else { 0 })
        | (if state.btn_touchpad {
            ffi::PS5_BTN_TOUCHPAD
        } else {
            0
        })
        | (if state.btn_mute { ffi::PS5_BTN_MUTE } else { 0 });

    let bytes = &mut r.0;
    bytes[ffi::PS5_OFF_LEFT_STICK_X] = state.left_stick_x;
    bytes[ffi::PS5_OFF_LEFT_STICK_Y] = state.left_stick_y;
    bytes[ffi::PS5_OFF_RIGHT_STICK_X] = state.right_stick_x;
    bytes[ffi::PS5_OFF_RIGHT_STICK_Y] = state.right_stick_y;
    bytes[ffi::PS5_OFF_LEFT_TRIGGER] = state.left_trigger;
    bytes[ffi::PS5_OFF_RIGHT_TRIGGER] = state.right_trigger;
    // Preserve hat (low nibble) which the SDK helper already wrote.
    bytes[ffi::PS5_OFF_HAT_FACE] = (bytes[ffi::PS5_OFF_HAT_FACE] & 0x0F) | face;
    bytes[ffi::PS5_OFF_SHOULDER_STICK] = shoulder;
    bytes[ffi::PS5_OFF_META] = meta;
}

pub(crate) fn apply_xone_state_bytes(r: &mut ffi::WINUHID_XONE_INPUT_REPORT, state: &XOneState) {
    let bytes = &mut r.0;
    bytes[ffi::XONE_OFF_LX..ffi::XONE_OFF_LX + 2]
        .copy_from_slice(&state.left_stick_x.to_le_bytes());
    bytes[ffi::XONE_OFF_LY..ffi::XONE_OFF_LY + 2]
        .copy_from_slice(&state.left_stick_y.to_le_bytes());
    bytes[ffi::XONE_OFF_RX..ffi::XONE_OFF_RX + 2]
        .copy_from_slice(&state.right_stick_x.to_le_bytes());
    bytes[ffi::XONE_OFF_RY..ffi::XONE_OFF_RY + 2]
        .copy_from_slice(&state.right_stick_y.to_le_bytes());
    // Triggers are 10-bit values stored in the low bits of two
    // separate u16 storage units. The high 6 bits in each storage unit
    // are reserved and stay zero.
    let lt = state.left_trigger.min(1023);
    let rt = state.right_trigger.min(1023);
    bytes[ffi::XONE_OFF_LT..ffi::XONE_OFF_LT + 2].copy_from_slice(&lt.to_le_bytes());
    bytes[ffi::XONE_OFF_RT..ffi::XONE_OFF_RT + 2].copy_from_slice(&rt.to_le_bytes());
    bytes[ffi::XONE_OFF_BTN1] = (if state.btn_a { ffi::XONE_BTN_A } else { 0 })
        | (if state.btn_b { ffi::XONE_BTN_B } else { 0 })
        | (if state.btn_x { ffi::XONE_BTN_X } else { 0 })
        | (if state.btn_y { ffi::XONE_BTN_Y } else { 0 })
        | (if state.btn_lb { ffi::XONE_BTN_LB } else { 0 })
        | (if state.btn_rb { ffi::XONE_BTN_RB } else { 0 })
        | (if state.btn_back {
            ffi::XONE_BTN_BACK
        } else {
            0
        })
        | (if state.btn_menu {
            ffi::XONE_BTN_MENU
        } else {
            0
        });
    bytes[ffi::XONE_OFF_BTN2] = (if state.btn_ls { ffi::XONE_BTN_LS } else { 0 })
        | (if state.btn_rs { ffi::XONE_BTN_RS } else { 0 });
    bytes[ffi::XONE_OFF_HOME] = if state.btn_home {
        ffi::XONE_BTN_HOME
    } else {
        0
    };
    bytes[ffi::XONE_OFF_BATTERY] = state.battery_level;
}

// ---------------------------------------------------------------------------
// (Previously held a `LATEST_SDK` global so `Drop` could reach the SDK
// function table. We now thread `Arc<Sdk>` through `Device` itself, so
// the global is gone.)

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // -- helpers ------------------------------------------------------------

    fn empty_ps4_report() -> ffi::WINUHID_PS4_INPUT_REPORT {
        // Identical to `default_ps4_report` minus the SDK call. The
        // tests don't care about the report id / stick centring, only
        // that the bytes the helper writes match.
        ffi::WINUHID_PS4_INPUT_REPORT {
            ReportId: 1,
            LeftStickX: 0,
            LeftStickY: 0,
            RightStickX: 0,
            RightStickY: 0,
            // Pre-seed the hat low nibble so we can check it survives
            // a call to `apply_ps4_state_bytes`.
            _packed_hat_face: 0x08,
            _packed_shoulder_stick: 0,
            _packed_meta: 0,
            LeftTrigger: 0,
            RightTrigger: 0,
            Timestamp: 0,
            BatteryLevel: 0,
            GyroX: 0,
            GyroY: 0,
            GyroZ: 0,
            AccelX: 0,
            AccelY: 0,
            AccelZ: 0,
            Reserved2: [0; 5],
            BatteryLevelSpecial: 0,
            Status: [0; 2],
            TouchReportCount: 0,
            TouchReports: [ffi::Ps4TouchReport {
                Timestamp: 0,
                TouchPoints: [ffi::Ps4TouchPoint {
                    ContactSeq: 0,
                    XLowPart: 0,
                    _packed_x_high_y_low: 0,
                    YHighPart: 0,
                }; 2],
            }; 3],
            Reserved3: [0; 3],
        }
    }

    fn distinctive_ps4_state() -> Ps4State {
        Ps4State {
            left_stick_x: 0x11,
            left_stick_y: 0x22,
            right_stick_x: 0x33,
            right_stick_y: 0x44,
            left_trigger: 0x55,
            right_trigger: 0x66,
            hat_x: -1,
            hat_y: 1,
            btn_square: true,
            btn_l1: true,
            btn_share: true,
            btn_home: true,
            btn_touchpad: true,
            ..Ps4State::default()
        }
    }

    fn parsed_event(report_id: u8, read_length: u32) -> ffi::ParsedEvent {
        ffi::ParsedEvent {
            kind: ffi::WINUHID_EVENT_READ_REPORT,
            request_id: 0,
            report_id,
            data: Vec::new(),
            read_length,
        }
    }

    // -- PS4 ----------------------------------------------------------------

    #[test]
    fn ps4_byte_packing_sets_sticks_triggers_and_masks() {
        let mut r = empty_ps4_report();
        let original_hat = r._packed_hat_face & 0x0F;
        let state = distinctive_ps4_state();

        apply_ps4_state_bytes(&mut r, &state);

        // `_packed_*` are bit-packed fields on a `repr(C, packed)` struct;
        // the compiler refuses to take direct references to them. Copy
        // the values out into local `u8`s before asserting.
        let left_stick_x = r.LeftStickX;
        let right_stick_y = r.RightStickY;
        let left_trigger = r.LeftTrigger;
        let right_trigger = r.RightTrigger;
        let hat_face = r._packed_hat_face;
        let shoulder = r._packed_shoulder_stick;
        let meta = r._packed_meta;

        assert_eq!(left_stick_x, 0x11);
        assert_eq!(right_stick_y, 0x44);
        assert_eq!(left_trigger, 0x55);
        assert_eq!(right_trigger, 0x66);

        // High nibble = face mask (square only), low nibble = preserved hat.
        assert_eq!(hat_face & 0xF0, ffi::PS4_FACE_SQUARE);
        assert_eq!(hat_face & 0x0F, original_hat);

        assert_eq!(shoulder, ffi::PS4_BTN_L1 | ffi::PS4_BTN_SHARE);
        assert_eq!(meta, ffi::PS4_BTN_HOME | ffi::PS4_BTN_TOUCHPAD);
    }

    // -- PS5 ----------------------------------------------------------------

    #[test]
    fn ps5_byte_packing_sets_sticks_triggers_and_masks() {
        let mut r = ffi::WINUHID_PS5_INPUT_REPORT::default();
        // Pre-seed the hat low nibble; the helper must preserve it.
        r.0[ffi::PS5_OFF_HAT_FACE] = 0x08;

        let state = Ps5State {
            left_stick_x: 0x11,
            left_stick_y: 0x22,
            right_stick_x: 0x33,
            right_stick_y: 0x44,
            left_trigger: 0x55,
            right_trigger: 0x66,
            hat_x: 0,
            hat_y: 0,
            btn_cross: true,
            btn_triangle: true,
            btn_l2: true,
            btn_options: true,
            btn_home: true,
            btn_mute: true,
            ..Ps5State::default()
        };

        apply_ps5_state_bytes(&mut r, &state);

        assert_eq!(r.0[ffi::PS5_OFF_LEFT_STICK_X], 0x11);
        assert_eq!(r.0[ffi::PS5_OFF_LEFT_STICK_Y], 0x22);
        assert_eq!(r.0[ffi::PS5_OFF_RIGHT_STICK_X], 0x33);
        assert_eq!(r.0[ffi::PS5_OFF_RIGHT_STICK_Y], 0x44);
        assert_eq!(r.0[ffi::PS5_OFF_LEFT_TRIGGER], 0x55);
        assert_eq!(r.0[ffi::PS5_OFF_RIGHT_TRIGGER], 0x66);

        // High nibble = face mask, low nibble = preserved hat.
        assert_eq!(
            r.0[ffi::PS5_OFF_HAT_FACE] & 0xF0,
            ffi::PS5_FACE_CROSS | ffi::PS5_FACE_TRIANGLE
        );
        assert_eq!(r.0[ffi::PS5_OFF_HAT_FACE] & 0x0F, 0x08);

        assert_eq!(
            r.0[ffi::PS5_OFF_SHOULDER_STICK],
            ffi::PS5_BTN_L2 | ffi::PS5_BTN_OPTIONS
        );
        assert_eq!(
            r.0[ffi::PS5_OFF_META],
            ffi::PS5_BTN_HOME | ffi::PS5_BTN_MUTE
        );
    }

    // -- Xbox One -----------------------------------------------------------

    #[test]
    fn xone_byte_packing_round_trips_sticks() {
        let mut r = ffi::WINUHID_XONE_INPUT_REPORT::default();
        let state = XOneState {
            left_stick_x: 0x1234,
            left_stick_y: 0x5678,
            right_stick_x: 0x9ABC,
            right_stick_y: 0xDEF0,
            ..XOneState::default()
        };

        apply_xone_state_bytes(&mut r, &state);

        assert_eq!(
            &r.0[ffi::XONE_OFF_LX..ffi::XONE_OFF_LX + 2],
            &0x1234u16.to_le_bytes()
        );
        assert_eq!(
            &r.0[ffi::XONE_OFF_LY..ffi::XONE_OFF_LY + 2],
            &0x5678u16.to_le_bytes()
        );
        assert_eq!(
            &r.0[ffi::XONE_OFF_RX..ffi::XONE_OFF_RX + 2],
            &0x9ABCu16.to_le_bytes()
        );
        assert_eq!(
            &r.0[ffi::XONE_OFF_RY..ffi::XONE_OFF_RY + 2],
            &0xDEF0u16.to_le_bytes()
        );
    }

    #[test]
    fn xone_byte_packing_writes_triggers_as_le_u16() {
        let mut r = ffi::WINUHID_XONE_INPUT_REPORT::default();
        let state = XOneState {
            left_trigger: 0x123,
            right_trigger: 0x2A0,
            ..XOneState::default()
        };

        apply_xone_state_bytes(&mut r, &state);

        assert_eq!(&r.0[8..10], &0x123u16.to_le_bytes());
        assert_eq!(&r.0[10..12], &0x2A0u16.to_le_bytes());
    }

    #[test]
    fn xone_byte_packing_clamps_oversize_trigger_to_10_bit() {
        let mut r = ffi::WINUHID_XONE_INPUT_REPORT::default();
        let state = XOneState {
            left_trigger: 0xFFFF,
            right_trigger: 0xFFFF,
            ..XOneState::default()
        };

        apply_xone_state_bytes(&mut r, &state);

        assert_eq!(
            &r.0[ffi::XONE_OFF_LT..ffi::XONE_OFF_LT + 2],
            &0x3FFu16.to_le_bytes()
        );
        assert_eq!(
            &r.0[ffi::XONE_OFF_RT..ffi::XONE_OFF_RT + 2],
            &0x3FFu16.to_le_bytes()
        );
    }

    #[test]
    fn xone_byte_packing_sets_button_masks() {
        let mut r = ffi::WINUHID_XONE_INPUT_REPORT::default();
        let state = XOneState {
            btn_a: true,
            btn_b: true,
            btn_lb: true,
            btn_ls: true,
            btn_home: true,
            battery_level: 0x42,
            ..XOneState::default()
        };

        apply_xone_state_bytes(&mut r, &state);

        assert_eq!(
            r.0[ffi::XONE_OFF_BTN1],
            ffi::XONE_BTN_A | ffi::XONE_BTN_B | ffi::XONE_BTN_LB
        );
        assert_eq!(r.0[ffi::XONE_OFF_BTN2], ffi::XONE_BTN_LS);
        assert_eq!(r.0[ffi::XONE_OFF_HOME], ffi::XONE_BTN_HOME);
        assert_eq!(r.0[ffi::XONE_OFF_BATTERY], 0x42);
    }

    // -- pick_read_snapshot -------------------------------------------------

    #[test]
    fn pick_read_snapshot_returns_cached_bytes_for_specific_report_id() {
        let inputs = Mutex::new(HashMap::from([(0x07u8, vec![0x07, 0xAA, 0xBB, 0xCC])]));
        let last = Mutex::new(Some(0x07u8));
        let ev = parsed_event(0x07, 16);

        let snapshot = pick_read_snapshot(&inputs, &last, &ev);

        assert_eq!(snapshot, Some(vec![0x07, 0xAA, 0xBB, 0xCC]));
    }

    #[test]
    fn pick_read_snapshot_synthesizes_zero_buffer_when_specific_id_missing() {
        let inputs: Mutex<HashMap<u8, Vec<u8>>> = Mutex::new(HashMap::new());
        let last = Mutex::new(None);
        let ev = parsed_event(0x05, 4);

        let snapshot = pick_read_snapshot(&inputs, &last, &ev);

        assert_eq!(snapshot, Some(vec![0x05, 0x00, 0x00, 0x00]));
    }

    #[test]
    fn pick_read_snapshot_falls_back_to_last_report_id_when_id_zero() {
        let inputs = Mutex::new(HashMap::from([(0x03u8, vec![0x03, 0xDE, 0xAD])]));
        let last = Mutex::new(Some(0x03u8));
        let ev = parsed_event(0, 0);

        let snapshot = pick_read_snapshot(&inputs, &last, &ev);

        assert_eq!(snapshot, Some(vec![0x03, 0xDE, 0xAD]));
    }

    #[test]
    fn pick_read_snapshot_returns_none_when_nothing_known() {
        let inputs: Mutex<HashMap<u8, Vec<u8>>> = Mutex::new(HashMap::new());
        let last: Mutex<Option<u8>> = Mutex::new(None);
        let ev = parsed_event(0, 0);

        let snapshot = pick_read_snapshot(&inputs, &last, &ev);

        assert_eq!(snapshot, None);
    }

    // -- Default-state shape (touchpad + IMU) -------------------------------

    #[test]
    fn ps4_state_default_includes_zero_touchpad_and_imu() {
        let s = Ps4State::default();
        assert!(!s.touchpad_active);
        assert_eq!(s.touchpad_x, 0);
        assert_eq!(s.touchpad_y, 0);
        assert!(!s.touchpad2_active);
        assert_eq!(s.touchpad2_x, 0);
        assert_eq!(s.touchpad2_y, 0);
        assert_eq!(s.accel_x, 0.0);
        assert_eq!(s.accel_y, 0.0);
        assert_eq!(s.accel_z, 0.0);
        assert_eq!(s.gyro_x, 0.0);
        assert_eq!(s.gyro_y, 0.0);
        assert_eq!(s.gyro_z, 0.0);
    }

    #[test]
    fn ps5_state_default_includes_zero_touchpad_and_imu() {
        let s = Ps5State::default();
        assert!(!s.touchpad_active);
        assert_eq!(s.touchpad_x, 0);
        assert_eq!(s.touchpad_y, 0);
        assert!(!s.touchpad2_active);
        assert_eq!(s.touchpad2_x, 0);
        assert_eq!(s.touchpad2_y, 0);
        assert_eq!(s.accel_x, 0.0);
        assert_eq!(s.accel_y, 0.0);
        assert_eq!(s.accel_z, 0.0);
        assert_eq!(s.gyro_x, 0.0);
        assert_eq!(s.gyro_y, 0.0);
        assert_eq!(s.gyro_z, 0.0);
    }
}
