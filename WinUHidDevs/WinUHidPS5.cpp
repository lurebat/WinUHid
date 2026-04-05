#include "pch.h"
#include "WinUHidPS5.h"

#include <algorithm>

#include <wrl/wrappers/corewrappers.h>
using namespace Microsoft::WRL;

//
// This device emulates a wired Sony DualSense gamepad
//

const BYTE k_PS5ReportDescriptor[] =
{
	0x05, 0x01,       // Usage Page (Generic Desktop Ctrls)
	0x09, 0x05,       // Usage (Game Pad)
	0xA1, 0x01,       // Collection (Application)
	0x85, 0x01,       //   Report ID (1)
	0x09, 0x30,       //   Usage (X)
	0x09, 0x31,       //   Usage (Y)
	0x09, 0x32,       //   Usage (Z)
	0x09, 0x35,       //   Usage (Rz)
	0x09, 0x33,       //   Usage (Rx)
	0x09, 0x34,       //   Usage (Ry)
	0x15, 0x00,       //   Logical Minimum (0)
	0x26, 0xFF, 0x00, //   Logical Maximum (255)
	0x75, 0x08,       //   Report Size (8)
	0x95, 0x06,       //   Report Count (6)
	0x81, 0x02,       //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
	0x06, 0x00, 0xFF, //   Usage Page (Vendor Defined 0xFF00)
	0x09, 0x20,       //   Usage (0x20)
	0x95, 0x01,       //   Report Count (1)
	0x81, 0x02,       //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
	0x05, 0x01,       //   Usage Page (Generic Desktop Ctrls)
	0x09, 0x39,       //   Usage (Hat switch)
	0x15, 0x00,       //   Logical Minimum (0)
	0x25, 0x07,       //   Logical Maximum (7)
	0x35, 0x00,       //   Physical Minimum (0)
	0x46, 0x3B, 0x01, //   Physical Maximum (315)
	0x65, 0x14,       //   Unit (System: English Rotation, Length: Centimeter)
	0x75, 0x04,       //   Report Size (4)
	0x95, 0x01,       //   Report Count (1)
	0x81, 0x42,       //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,Null State)
	0x65, 0x00,       //   Unit (None)
	0x05, 0x09,       //   Usage Page (Button)
	0x19, 0x01,       //   Usage Minimum (0x01)
	0x29, 0x0F,       //   Usage Maximum (0x0F)
	0x15, 0x00,       //   Logical Minimum (0)
	0x25, 0x01,       //   Logical Maximum (1)
	0x75, 0x01,       //   Report Size (1)
	0x95, 0x0F,       //   Report Count (15)
	0x81, 0x02,       //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
	0x06, 0x00, 0xFF, //   Usage Page (Vendor Defined 0xFF00)
	0x09, 0x21,       //   Usage (0x21)
	0x95, 0x0D,       //   Report Count (13)
	0x81, 0x02,       //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
	0x06, 0x00, 0xFF, //   Usage Page (Vendor Defined 0xFF00)
	0x09, 0x22,       //   Usage (0x22)
	0x15, 0x00,       //   Logical Minimum (0)
	0x26, 0xFF, 0x00, //   Logical Maximum (255)
	0x75, 0x08,       //   Report Size (8)
	0x95, 0x34,       //   Report Count (52)
	0x81, 0x02,       //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
	0x85, 0x02,       //   Report ID (2)
	0x09, 0x23,       //   Usage (0x23)
	0x95, 0x2F,       //   Report Count (47)
	0x91, 0x02,       //   Output (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x05,       //   Report ID (5)
	0x09, 0x33,       //   Usage (0x33)
	0x95, 0x28,       //   Report Count (40)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x08,       //   Report ID (8)
	0x09, 0x34,       //   Usage (0x34)
	0x95, 0x2F,       //   Report Count (47)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x09,       //   Report ID (9)
	0x09, 0x24,       //   Usage (0x24)
	0x95, 0x13,       //   Report Count (19)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x0A,       //   Report ID (10)
	0x09, 0x25,       //   Usage (0x25)
	0x95, 0x1A,       //   Report Count (26)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x20,       //   Report ID (32)
	0x09, 0x26,       //   Usage (0x26)
	0x95, 0x3F,       //   Report Count (63)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x21,       //   Report ID (33)
	0x09, 0x27,       //   Usage (0x27)
	0x95, 0x04,       //   Report Count (4)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x22,       //   Report ID (34)
	0x09, 0x40,       //   Usage (0x40)
	0x95, 0x3F,       //   Report Count (63)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x80,       //   Report ID (-128)
	0x09, 0x28,       //   Usage (0x28)
	0x95, 0x3F,       //   Report Count (63)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x81,       //   Report ID (-127)
	0x09, 0x29,       //   Usage (0x29)
	0x95, 0x3F,       //   Report Count (63)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x82,       //   Report ID (-126)
	0x09, 0x2A,       //   Usage (0x2A)
	0x95, 0x09,       //   Report Count (9)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x83,       //   Report ID (-125)
	0x09, 0x2B,       //   Usage (0x2B)
	0x95, 0x3F,       //   Report Count (63)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x84,       //   Report ID (-124)
	0x09, 0x2C,       //   Usage (0x2C)
	0x95, 0x3F,       //   Report Count (63)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0x85,       //   Report ID (-123)
	0x09, 0x2D,       //   Usage (0x2D)
	0x95, 0x02,       //   Report Count (2)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0xA0,       //   Report ID (-96)
	0x09, 0x2E,       //   Usage (0x2E)
	0x95, 0x01,       //   Report Count (1)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0xE0,       //   Report ID (-32)
	0x09, 0x2F,       //   Usage (0x2F)
	0x95, 0x3F,       //   Report Count (63)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0xF0,       //   Report ID (-16)
	0x09, 0x30,       //   Usage (0x30)
	0x95, 0x3F,       //   Report Count (63)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0xF1,       //   Report ID (-15)
	0x09, 0x31,       //   Usage (0x31)
	0x95, 0x3F,       //   Report Count (63)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0xF2,       //   Report ID (-14)
	0x09, 0x32,       //   Usage (0x32)
	0x95, 0x0F,       //   Report Count (15)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0xF4,       //   Report ID (-12)
	0x09, 0x35,       //   Usage (0x35)
	0x95, 0x3F,       //   Report Count (63)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0x85, 0xF5,       //   Report ID (-11)
	0x09, 0x36,       //   Usage (0x36)
	0x95, 0x03,       //   Report Count (3)
	0xB1, 0x02,       //   Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
	0xC0,             // End Collection
};

const WINUHID_DEVICE_CONFIG k_PS5Config =
{
	(WINUHID_EVENT_TYPE)(WINUHID_EVENT_READ_REPORT | WINUHID_EVENT_WRITE_REPORT | WINUHID_EVENT_GET_FEATURE | WINUHID_EVENT_SET_FEATURE),
	0x054c, // Sony
	0x0ce6, // DualSense
	0,
	sizeof(k_PS5ReportDescriptor),
	k_PS5ReportDescriptor,
	{},
	NULL,
	NULL,
	10000, // 10 ms input throttling interval
};

#include <pshpack1.h>

typedef struct _PS5_LED_STATE {
	UCHAR Red;
	UCHAR Green;
	UCHAR Blue;
} PS5_LED_STATE, *PPS5_LED_STATE;

typedef struct _PS5_RUMBLE_STATE {
	UCHAR Right;
	UCHAR Left;
} PS5_RUMBLE_STATE, *PPS5_RUMBLE_STATE;

typedef struct PS5_OUTPUT_REPORT {
	UCHAR CompatibleVibration : 1;
	UCHAR RumbleNotHaptics : 1;
	UCHAR RightTriggerEffectValid : 1;
	UCHAR LeftTriggerEffectValid : 1;
	UCHAR Reserved : 4;
	UCHAR MicMuteLedValid : 1;
	UCHAR PowerSaveValid : 1;
	UCHAR LightBarControlValid : 1;
	UCHAR ReleaseLeds : 1;
	UCHAR PlayerIndicatorValid : 1;
	UCHAR Reserved2 : 3;

	PS5_RUMBLE_STATE MotorState;

	UCHAR Reserved3[4];
	UCHAR MuteButtonLed;

	UCHAR PowerSaveControl;
	WINUHID_PS5_TRIGGER_EFFECT RightTriggerEffect;
	WINUHID_PS5_TRIGGER_EFFECT LeftTriggerEffect;
	UCHAR Reserved4[6];

	UCHAR LedBrightnessValid : 1;
	UCHAR LightBarSetupValid : 1;
	UCHAR CompatibleVibration2 : 1;
	UCHAR Reserved5 : 5;
	UCHAR Reserved6[2];
	UCHAR LightBarSetup;
	UCHAR LedBrightness;
	UCHAR PlayerLeds;
	PS5_LED_STATE LedState;
} PS5_OUTPUT_REPORT, *PPS5_OUTPUT_REPORT;

#include <poppack.h>

typedef struct _WINUHID_PS5_GAMEPAD {
	PWINUHID_DEVICE Device;
	BOOL Stopping;
	SRWLOCK Lock;

	PWINUHID_PS5_RUMBLE_CB RumbleCallback;
	PS5_RUMBLE_STATE LastMotorState;
	PWINUHID_PS5_LIGHTBAR_LED_CB LightBarCallback;
	PS5_LED_STATE LastLedState;
	PWINUHID_PS5_TRIGGER_EFFECT_CB TriggerEffectCallback;
	WINUHID_PS5_TRIGGER_EFFECT LastRightTriggerEffect, LastLeftTriggerEffect;
	PWINUHID_PS5_PLAYER_LED_CB PlayerLedCallback;
	UCHAR LastPlayerLedState;
	PWINUHID_PS5_MIC_LED_CB MicLedCallback;
	UCHAR LastMicLedState;
	PVOID CallbackContext;

	LARGE_INTEGER QpcFrequency;
	LARGE_INTEGER LastInputReportTime;
	WINUHID_PS5_INPUT_REPORT LastInputReport;
	UINT Timestamp;
	UCHAR SequenceNumber;

	UCHAR MacAddress[6];
	UCHAR FirmwareInfoReport[64];
} WINUHID_PS5_GAMEPAD, *PWINUHID_PS5_GAMEPAD;

#define PS5_FEATURE_REPORT_CALIBRATION		0x05
#define PS5_FEATURE_REPORT_PAIRING_INFO		0x09
#define PS5_FEATURE_REPORT_FIRMWARE_INFO	0x20

static const UCHAR k_DefaultFirmwareInfo[] =
{
	0x20, 0x4d, 0x61, 0x72, 0x20, 0x31, 0x35, 0x20,
	0x32, 0x30, 0x32, 0x35, 0x31, 0x30, 0x3a, 0x30,
	0x30, 0x3a, 0x30, 0x30, 0x04, 0x00, 0x14, 0x00,
	0x2c, 0x04, 0x00, 0x00, 0x0a, 0x00, 0x0c, 0x01,
	0x51, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
	0x00, 0x00, 0x00, 0x00, 0x58, 0x04, 0x00, 0x00,
	0x2a, 0x00, 0x01, 0x00, 0x09, 0x00, 0x02, 0x00,
	0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
};

#define PS5_OUTPUT_REPORT_EFFECTS           0x02

void PrepareInputReportForSubmission(PWINUHID_PS5_GAMEPAD Gamepad)
{
	LARGE_INTEGER now;
	QueryPerformanceCounter(&now);

	//
	// Compute the time between reports to determine the timestamp increment
	//
	LARGE_INTEGER deltaNs;
	deltaNs.QuadPart = now.QuadPart - Gamepad->LastInputReportTime.QuadPart;
	deltaNs.QuadPart *= 1000000000ULL;
	deltaNs.QuadPart /= Gamepad->QpcFrequency.QuadPart;
	Gamepad->Timestamp += (UINT)(deltaNs.QuadPart / 333); // Timestamp is in 0.333us units
	Gamepad->LastInputReportTime = now;

	//
	// Send the input report with the updated timestamp and sequence number
	//
	Gamepad->LastInputReport.SequenceNumber = Gamepad->SequenceNumber++;
	Gamepad->LastInputReport.SensorTimestamp = Gamepad->Timestamp;
}

VOID WinUHidPS5Callback(PVOID CallbackContext, PWINUHID_DEVICE Device, PCWINUHID_EVENT Event)
{
	auto gamepad = (PWINUHID_PS5_GAMEPAD)CallbackContext;

	if (Event->Type == WINUHID_EVENT_GET_FEATURE) {
		switch (Event->ReportId)
		{
		case PS5_FEATURE_REPORT_CALIBRATION:
		{
			//
			// This neutral calibration is from inputino
			//
			static const UCHAR data[] =
			{
				0x02,
				0x00, 0x00, // gyro_pitch_bias
				0x00, 0x00, // gyro_yaw_bias
				0x00, 0x00, // gyro_roll_bias
				0x10, 0x27, // gyro_pitch_plus
				0xF0, 0xD8, // gyro_pitch_minus
				0x10, 0x27, // gyro_yaw_plus
				0xF0, 0xD8,  // gyro_yaw_minus
				0x10, 0x27, // gyro_roll_plus
				0xF0, 0xD8, // gyro_roll_minus
				0xF4, 0x01, // gyro_speed_plus
				0xF4, 0x01, // gyro_speed_minus
				0x10, 0x27, // acc_x_plus
				0xF0, 0xD8, // acc_x_minus
				0x10, 0x27, // acc_y_plus
				0xF0, 0xD8, // acc_y_minus
				0x10, 0x27, // acc_z_plus
				0xF0, 0xD8, // acc_z_minus
			};

			WinUHidCompleteReadEvent(Device, Event, &data, sizeof(data));
			break;
		}

		case PS5_FEATURE_REPORT_FIRMWARE_INFO:
		{
			WinUHidCompleteReadEvent(Device, Event,
				gamepad->FirmwareInfoReport, sizeof(gamepad->FirmwareInfoReport));
			break;
		}

		case PS5_FEATURE_REPORT_PAIRING_INFO:
		{
			UCHAR data[] =
			{
				0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
			};

			//
			// The MAC address is reported in reverse order
			//
			for (int i = 0; i < sizeof(gamepad->MacAddress); i++) {
				data[i + 1] = gamepad->MacAddress[sizeof(gamepad->MacAddress) - (i + 1)];
			}

			WinUHidCompleteReadEvent(Device, Event, data, sizeof(data));
			break;
		}

		default:
		{
			//
			// Return zero-filled data for unimplemented feature reports.
			// Returning NULL causes WebHID clients (Chrome) to timeout/retry
			// on each unimplemented report during enumeration, leading to
			// multi-second freezes.
			//
			UCHAR zeroData[64] = {};
			zeroData[0] = Event->ReportId;
			WinUHidCompleteReadEvent(Device, Event, zeroData, sizeof(zeroData));
			break;
		}
		}
	}
	else if (Event->Type == WINUHID_EVENT_SET_FEATURE) {
		//
		// Just succeed all set feature events
		//
		WinUHidCompleteWriteEvent(Device, Event, TRUE);
	}
	else if (Event->Type == WINUHID_EVENT_READ_REPORT) {
		//
		// Resubmit the latest input report with updated timestamps
		//
		AcquireSRWLockExclusive(&gamepad->Lock);
		PrepareInputReportForSubmission(gamepad);
		ReleaseSRWLockExclusive(&gamepad->Lock);
		WinUHidCompleteReadEvent(Device, Event, &gamepad->LastInputReport, sizeof(gamepad->LastInputReport));
	}
	else {
		//
		// There's only one defined output report
		//
		if (Event->ReportId != PS5_OUTPUT_REPORT_EFFECTS || Event->Write.DataLength < 1 + sizeof(PS5_OUTPUT_REPORT)) {
			WinUHidCompleteWriteEvent(Device, Event, FALSE);
			return;
		}

		auto outputReport = (PPS5_OUTPUT_REPORT)&Event->Write.Data[1];

		if (gamepad->LightBarCallback && outputReport->LightBarControlValid && !RtlEqualMemory(&outputReport->LedState, &gamepad->LastLedState, sizeof(gamepad->LastLedState))) {
			gamepad->LightBarCallback(gamepad->CallbackContext, outputReport->LedState.Red, outputReport->LedState.Green, outputReport->LedState.Blue);
			gamepad->LastLedState = outputReport->LedState;
		}

		if (gamepad->PlayerLedCallback && outputReport->PlayerIndicatorValid && outputReport->PlayerLeds != gamepad->LastPlayerLedState) {
			gamepad->PlayerLedCallback(gamepad->CallbackContext, outputReport->PlayerLeds);
			gamepad->LastPlayerLedState = outputReport->PlayerLeds;
		}

		if (gamepad->RumbleCallback && (outputReport->CompatibleVibration || outputReport->CompatibleVibration2) && !RtlEqualMemory(&outputReport->MotorState, &gamepad->LastMotorState, sizeof(gamepad->LastMotorState))) {
			//
			// When using legacy rumble support, double the amplitude to more closely match real hardware behavior
			//
			gamepad->RumbleCallback(gamepad->CallbackContext,
				outputReport->MotorState.Left << (outputReport->CompatibleVibration2 ? 0 : 1),
				outputReport->MotorState.Right << (outputReport->CompatibleVibration2 ? 0 : 1));
			gamepad->LastMotorState = outputReport->MotorState;
		}

		if (gamepad->TriggerEffectCallback &&
			((outputReport->RightTriggerEffectValid && !RtlEqualMemory(&outputReport->RightTriggerEffect, &gamepad->LastRightTriggerEffect, sizeof(gamepad->LastRightTriggerEffect))) ||
			 (outputReport->LeftTriggerEffectValid && !RtlEqualMemory(&outputReport->LeftTriggerEffect, &gamepad->LastLeftTriggerEffect, sizeof(gamepad->LastLeftTriggerEffect))))) {
			gamepad->TriggerEffectCallback(gamepad->CallbackContext,
				outputReport->LeftTriggerEffectValid ? &outputReport->LeftTriggerEffect : NULL,
				outputReport->RightTriggerEffectValid ? &outputReport->RightTriggerEffect : NULL);

			if (outputReport->RightTriggerEffectValid) {
				gamepad->LastRightTriggerEffect = outputReport->RightTriggerEffect;
			}
			if (outputReport->LeftTriggerEffectValid) {
				gamepad->LastLeftTriggerEffect = outputReport->LeftTriggerEffect;
			}
		}

		if (gamepad->MicLedCallback && outputReport->MicMuteLedValid && outputReport->MuteButtonLed != gamepad->LastMicLedState) {
			gamepad->MicLedCallback(gamepad->CallbackContext, outputReport->MuteButtonLed);
			gamepad->LastMicLedState = outputReport->MuteButtonLed;
		}

		WinUHidCompleteWriteEvent(Device, Event, TRUE);
	}
}

WINUHID_API PWINUHID_PS5_GAMEPAD WinUHidPS5Create(PCWINUHID_PS5_GAMEPAD_INFO Info,
	PWINUHID_PS5_RUMBLE_CB RumbleCallback, PWINUHID_PS5_LIGHTBAR_LED_CB LightBarLedCallback,
	PWINUHID_PS5_PLAYER_LED_CB PlayerLedCallback, PWINUHID_PS5_TRIGGER_EFFECT_CB TriggerEffectCallback,
	PWINUHID_PS5_MIC_LED_CB MicLedCallback, PVOID CallbackContext)
{
	WINUHID_DEVICE_CONFIG config = k_PS5Config;
	PopulateDeviceInfo(&config, Info ? Info->BasicInfo : NULL);

	if (config.VendorID == 0) {
		SetLastError(ERROR_INVALID_PARAMETER);
		return NULL;
	}

	PWINUHID_PS5_GAMEPAD gamepad = (PWINUHID_PS5_GAMEPAD)HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, sizeof(*gamepad));
	if (!gamepad) {
		SetLastError(ERROR_OUTOFMEMORY);
		return NULL;
	}

	QueryPerformanceFrequency(&gamepad->QpcFrequency);
	QueryPerformanceCounter(&gamepad->LastInputReportTime);

	InitializeSRWLock(&gamepad->Lock);
	gamepad->RumbleCallback = RumbleCallback;
	gamepad->LightBarCallback = LightBarLedCallback;
	gamepad->PlayerLedCallback = PlayerLedCallback;
	gamepad->TriggerEffectCallback = TriggerEffectCallback;
	gamepad->MicLedCallback = MicLedCallback;
	gamepad->CallbackContext = CallbackContext;

	if (Info) {
		RtlCopyMemory(&gamepad->MacAddress[0], &Info->MacAddress[0], sizeof(gamepad->MacAddress));

		if (Info->FirmwareInfo && Info->FirmwareInfoLength == sizeof(gamepad->FirmwareInfoReport)) {
			RtlCopyMemory(gamepad->FirmwareInfoReport, Info->FirmwareInfo, sizeof(gamepad->FirmwareInfoReport));
		} else {
			RtlCopyMemory(gamepad->FirmwareInfoReport, k_DefaultFirmwareInfo, sizeof(k_DefaultFirmwareInfo));
		}
	} else {
		RtlCopyMemory(gamepad->FirmwareInfoReport, k_DefaultFirmwareInfo, sizeof(k_DefaultFirmwareInfo));
	}

	WinUHidPS5InitializeInputReport(&gamepad->LastInputReport);

	gamepad->Device = WinUHidCreateDevice(&config);
	if (!gamepad->Device) {
		WinUHidPS5Destroy(gamepad);
		return NULL;
	}

	if (!WinUHidStartDevice(gamepad->Device, WinUHidPS5Callback, gamepad)) {
		WinUHidPS5Destroy(gamepad);
		return NULL;
	}

	//
	// Send an neutral input report
	//
	WINUHID_PS5_INPUT_REPORT inputReport;
	WinUHidPS5InitializeInputReport(&inputReport);
	if (!WinUHidPS5ReportInput(gamepad, &inputReport)) {
		WinUHidPS5Destroy(gamepad);
		return NULL;
	}

	return gamepad;
}

WINUHID_API BOOL WinUHidPS5ReportInput(PWINUHID_PS5_GAMEPAD Gamepad, PCWINUHID_PS5_INPUT_REPORT Report)
{
	AcquireSRWLockExclusive(&Gamepad->Lock);
	Gamepad->LastInputReport = *Report;
	PrepareInputReportForSubmission(Gamepad);
	ReleaseSRWLockExclusive(&Gamepad->Lock);

	BOOL ret = WinUHidSubmitInputReport(Gamepad->Device, &Gamepad->LastInputReport, sizeof(Gamepad->LastInputReport));

	//
	// Since we handle WINUHID_EVENT_READ_REPORT, WinUHidSubmitInputReport may fail with ERROR_NOT_READY
	// if a HID client hasn't asked for another report. This is fine because our callback function will
	// report this input once the caller is ready for it.
	//
	return ret || GetLastError() == ERROR_NOT_READY;
}

WINUHID_API VOID WinUHidPS5InitializeInputReport(PWINUHID_PS5_INPUT_REPORT Report)
{
	RtlZeroMemory(Report, sizeof(*Report));

	Report->ReportId = 0x01;
	Report->LeftStickX = 0x80;
	Report->LeftStickY = 0x80;
	Report->RightStickX = 0x80;
	Report->RightStickY = 0x80;
	Report->Hat = 0x8;

	WinUHidPS5SetTouchState(Report, 0, FALSE, 0, 0);
	WinUHidPS5SetTouchState(Report, 1, FALSE, 0, 0);

	WinUHidPS5SetBatteryState(Report, TRUE, 100);

	//
	// Initialize accelerometer in neutral upright position
	//
	WinUHidPS5SetAccelState(Report, 0, 9.80665f, 0);
}

WINUHID_API VOID WinUHidPS5SetTouchState(PWINUHID_PS5_INPUT_REPORT Report, UCHAR TouchIndex, BOOL TouchDown, USHORT TouchX, USHORT TouchY)
{
	if (TouchDown) {
		Report->TouchReport.TouchPoints[TouchIndex].ContactSeq++;
		Report->TouchReport.TouchPoints[TouchIndex].ContactSeq &= ~0x80;
	}
	else {
		Report->TouchReport.TouchPoints[TouchIndex].ContactSeq |= 0x80;
	}
	Report->TouchReport.TouchPoints[TouchIndex].XLowPart = TouchX & 0xFF;
	Report->TouchReport.TouchPoints[TouchIndex].XHighPart = (TouchX >> 8) & 0xF;
	Report->TouchReport.TouchPoints[TouchIndex].YLowPart = TouchY & 0xF;
	Report->TouchReport.TouchPoints[TouchIndex].YHighPart = (TouchY >> 4) & 0xFF;
	Report->TouchReport.Timestamp++;
}

WINUHID_API VOID WinUHidPS5SetAccelState(PWINUHID_PS5_INPUT_REPORT Report, float AccelX, float AccelY, float AccelZ)
{
	static const float k_AccelSensitivity = 0.000980664976f;

	Report->AccelX = std::clamp((int)(AccelX / k_AccelSensitivity), SHRT_MIN, SHRT_MAX);
	Report->AccelY = std::clamp((int)(AccelY / k_AccelSensitivity), SHRT_MIN, SHRT_MAX);
	Report->AccelZ = std::clamp((int)(AccelZ / k_AccelSensitivity), SHRT_MIN, SHRT_MAX);
}

WINUHID_API VOID WinUHidPS5SetGyroState(PWINUHID_PS5_INPUT_REPORT Report, float GyroX, float GyroY, float GyroZ)
{
	static const float k_GyroSensitivity = 0.000872664619f;

	Report->GyroX = std::clamp((int)(GyroX / k_GyroSensitivity), SHRT_MIN, SHRT_MAX);
	Report->GyroY = std::clamp((int)(GyroY / k_GyroSensitivity), SHRT_MIN, SHRT_MAX);
	Report->GyroZ = std::clamp((int)(GyroZ / k_GyroSensitivity), SHRT_MIN, SHRT_MAX);
}

WINUHID_API VOID WinUHidPS5SetBatteryState(PWINUHID_PS5_INPUT_REPORT Report, BOOL Wired, UCHAR Percentage)
{
	if (Percentage == 100 && Wired) {
		Report->BatteryPercent = 0xA;
		Report->BatteryState = 0x02;
	}
	else {
		Report->BatteryPercent = Percentage / 10;
		Report->BatteryState = Wired ? 1 : 0;
	}
}

WINUHID_API VOID WinUHidPS5SetHatState(PWINUHID_PS5_INPUT_REPORT Report, INT HatX, INT HatY)
{
	if (HatX == 0 && HatY == 0) {
		Report->Hat = 0x8; // Neutral
	}
	else if (HatX < 0 && HatY < 0) {
		Report->Hat = 0x7; // Top-left
	}
	else if (HatX < 0 && HatY == 0) {
		Report->Hat = 0x6; // Left
	}
	else if (HatX < 0 && HatY > 0) {
		Report->Hat = 0x5; // Bottom-left
	}
	else if (HatX == 0 && HatY > 0) {
		Report->Hat = 0x4; // Bottom
	}
	else if (HatX > 0 && HatY > 0) {
		Report->Hat = 0x3; // Bottom-right
	}
	else if (HatX > 0 && HatY == 0) {
		Report->Hat = 0x2; // Right
	}
	else if (HatX > 0 && HatY < 0) {
		Report->Hat = 0x1; // Top-right
	}
	else {
		Report->Hat = 0x0; // Top
	}
}

WINUHID_API VOID WinUHidPS5Destroy(PWINUHID_PS5_GAMEPAD Gamepad)
{
	if (!Gamepad) {
		return;
	}

	Gamepad->Stopping = TRUE;

	if (Gamepad->Device) {
		WinUHidDestroyDevice(Gamepad->Device);
	}

	HeapFree(GetProcessHeap(), 0, Gamepad);
}