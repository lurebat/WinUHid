#pragma once

#include "WinUHidDevs.h"

typedef struct _WINUHID_PS5_GAMEPAD* PWINUHID_PS5_GAMEPAD;

#include <pshpack1.h>

//
// https://controllers.fandom.com/wiki/Sony_DualSense
//
typedef struct _WINUHID_PS5_INPUT_REPORT
{
	UCHAR ReportId; // Do not modify

	UCHAR LeftStickX; // 0x80 is centered
	UCHAR LeftStickY; // 0x80 is centered
	UCHAR RightStickX; // 0x80 is centered
	UCHAR RightStickY; // 0x80 is centered

	UCHAR LeftTrigger;
	UCHAR RightTrigger;
	UCHAR SequenceNumber; // Calculated by WinUHid

	UCHAR Hat : 4; // Use WinUHidPS5SetHatState()
	UCHAR ButtonSquare : 1;
	UCHAR ButtonCross : 1;
	UCHAR ButtonCircle : 1;
	UCHAR ButtonTriangle : 1;
	UCHAR ButtonL1 : 1;
	UCHAR ButtonR1 : 1;
	UCHAR ButtonL2 : 1;
	UCHAR ButtonR2 : 1;
	UCHAR ButtonShare : 1;
	UCHAR ButtonOptions : 1;
	UCHAR ButtonL3 : 1;
	UCHAR ButtonR3 : 1;
	UCHAR ButtonHome : 1;
	UCHAR ButtonTouchpad : 1;
	UCHAR ButtonMute : 1;
	UCHAR Reserved : 1;
	UCHAR ButtonLeftFunction : 1;
	UCHAR ButtonRightFunction : 1;
	UCHAR ButtonLeftPaddle : 1;
	UCHAR ButtonRightPaddle : 1;
	UCHAR Reserved2[5];

	USHORT GyroX;
	USHORT GyroY;
	USHORT GyroZ;
	USHORT AccelX;
	USHORT AccelY;
	USHORT AccelZ;
	UINT SensorTimestamp; // Calculated by WinUHid
	UCHAR Temperature;

	//
	// Use WinUHidPS5SetTouchReport() to set these fields
	//
	struct {
		struct {
			UCHAR ContactSeq;
			UCHAR XLowPart;
			UCHAR XHighPart : 4;
			UCHAR YLowPart : 4;
			UCHAR YHighPart;
		} TouchPoints[2];
		UCHAR Timestamp;
	} TouchReport;

	//
	// Adaptive trigger status
	//
	UCHAR TriggerRightStopLocation : 4;
	UCHAR TriggerRightStatus : 4;
	UCHAR TriggerLeftStopLocation : 4;
	UCHAR TriggerLeftStatus : 4;
	UINT HostTimestamp;
	UCHAR TriggerRightEffect : 4;
	UCHAR TriggerLeftEffect : 4;
	UINT DeviceTimestamp;

	//
	// Use WinUHidPS5SetBatteryState() to set these fields
	//
	UCHAR BatteryPercent : 4;
	UCHAR BatteryState : 4;

	//
	// There are more fields beyond this that are not implemented by WinUHid
	//
	UCHAR Reserved3[10];
} WINUHID_PS5_INPUT_REPORT, *PWINUHID_PS5_INPUT_REPORT;
typedef CONST WINUHID_PS5_INPUT_REPORT *PCWINUHID_PS5_INPUT_REPORT;

typedef struct _WINUHID_PS5_TRIGGER_EFFECT {
	UCHAR Type;
	UCHAR Data[10];
} WINUHID_PS5_TRIGGER_EFFECT, *PWINUHID_PS5_TRIGGER_EFFECT;
typedef CONST WINUHID_PS5_TRIGGER_EFFECT *PCWINUHID_PS5_TRIGGER_EFFECT;

#include <poppack.h>

//
// Optional callback to be invoked when rumble motor state changes
//
typedef VOID WINUHID_PS5_RUMBLE_CB(PVOID CallbackContext, UCHAR LeftMotor, UCHAR RightMotor);
typedef WINUHID_PS5_RUMBLE_CB *PWINUHID_PS5_RUMBLE_CB;

//
// Optional callback to be invoked when adaptive trigger effects change
//
// NOTE: LeftTriggerEffect or RightTriggerEffect may be null if no effect is enabled for the respective trigger.
//
typedef VOID WINUHID_PS5_TRIGGER_EFFECT_CB(PVOID CallbackContext, PCWINUHID_PS5_TRIGGER_EFFECT LeftTriggerEffect, PCWINUHID_PS5_TRIGGER_EFFECT RightTriggerEffect);
typedef WINUHID_PS5_TRIGGER_EFFECT_CB *PWINUHID_PS5_TRIGGER_EFFECT_CB;

//
// Optional callback to be invoked when lightbar LED state changes
//
typedef VOID WINUHID_PS5_LIGHTBAR_LED_CB(PVOID CallbackContext, UCHAR LedRed, UCHAR LedGreen, UCHAR LedBlue);
typedef WINUHID_PS5_LIGHTBAR_LED_CB* PWINUHID_PS5_LIGHTBAR_LED_CB;

//
// Optional callback to be invoked when player index LED state changes
//
// Standard values for each player are (masking 0x1F):
// Player 1 -> 0x04
// Player 2 -> 0x0A
// Player 3 -> 0x15
// Player 4 -> 0x1B
// Player 5 -> 0x1F
//
// However, nothing prevents other values from being sent.
//
typedef VOID WINUHID_PS5_PLAYER_LED_CB(PVOID CallbackContext, UCHAR LedValue);
typedef WINUHID_PS5_PLAYER_LED_CB* PWINUHID_PS5_PLAYER_LED_CB;

//
// Optional callback to be invoked when mic mute LED state changes
//
// Values: 0 = off, 1 = on (solid), 2 = pulse (blink)
//
typedef VOID WINUHID_PS5_MIC_LED_CB(PVOID CallbackContext, UCHAR LedState);
typedef WINUHID_PS5_MIC_LED_CB* PWINUHID_PS5_MIC_LED_CB;

typedef struct _WINUHID_PS5_GAMEPAD_INFO {
	//
	// Basic HID and PnP device information (optional)
	//
	PCWINUHID_PRESET_DEVICE_INFO BasicInfo;

	//
	// Unique identifier for this PS5 gamepad
	//
	UCHAR MacAddress[6];

	//
	// Optional raw firmware info report (feature report 0x20).
	// If non-NULL, this 64-byte blob is served verbatim for
	// GET_FEATURE 0x20 requests. First byte must be the report
	// ID (0x20). If NULL, a built-in default is used.
	//
	CONST UCHAR *FirmwareInfo;
	UCHAR FirmwareInfoLength;
} WINUHID_PS5_GAMEPAD_INFO, * PWINUHID_PS5_GAMEPAD_INFO;
typedef CONST WINUHID_PS5_GAMEPAD_INFO* PCWINUHID_PS5_GAMEPAD_INFO;

//
// Creates a new PS5 gamepad.
//
// NOTE: By default this emulates a standard DualSense gamepad. If you want to use buttons
// that are only present on DualSense Edge, you should override the ProductID to 0x0df2 to
// ensure applications will know to look for them.
//
// To destroy the device, call WinUHidPS5Destroy().
//
// On failure, the function will return NULL. Call GetLastError() to the error code.
//
WINUHID_API PWINUHID_PS5_GAMEPAD WinUHidPS5Create(PCWINUHID_PS5_GAMEPAD_INFO Info,
	PWINUHID_PS5_RUMBLE_CB RumbleCallback, PWINUHID_PS5_LIGHTBAR_LED_CB LightBarLedCallback,
	PWINUHID_PS5_PLAYER_LED_CB PlayerLedCallback, PWINUHID_PS5_TRIGGER_EFFECT_CB TriggerEffectCallback,
	PWINUHID_PS5_MIC_LED_CB MicLedCallback, PVOID CallbackContext);

//
// Initializes the input report with neutral data.
//
WINUHID_API VOID WinUHidPS5InitializeInputReport(PWINUHID_PS5_INPUT_REPORT Report);

//
// Sets the hat state in the input report.
//
// Each axis of the hat can have one of 3 values: -1, 0, 1
//
// Negative values are left/up. Positive values are right/down. 0 is neutral.
//
WINUHID_API VOID WinUHidPS5SetHatState(PWINUHID_PS5_INPUT_REPORT Report, INT HatX, INT HatY);

//
// Sets the battery state in the input report.
//
WINUHID_API VOID WinUHidPS5SetBatteryState(PWINUHID_PS5_INPUT_REPORT Report, BOOL Wired, UCHAR Percentage);

//
// Sets the touch state in the input report.
//
// The PS5 controller supports 2 simultaneous touches (TouchIndex 0 and 1).
// The touchpad is 1920x1080, so TouchX/Y must be within those dimensions.
//
WINUHID_API VOID WinUHidPS5SetTouchState(PWINUHID_PS5_INPUT_REPORT Report, UCHAR TouchIndex, BOOL TouchDown, USHORT TouchX, USHORT TouchY);

//
// Sets the accelerometer state in the input report.
//
// The values provided should be in meters per second squared.
//
WINUHID_API VOID WinUHidPS5SetAccelState(PWINUHID_PS5_INPUT_REPORT Report, float AccelX, float AccelY, float AccelZ);

//
// Sets the gyroscope state in the input report.
//
// The values provided should be in radians per second.
//
WINUHID_API VOID WinUHidPS5SetGyroState(PWINUHID_PS5_INPUT_REPORT Report, float GyroX, float GyroY, float GyroZ);

//
// Submits an input report to the device.
//
// On failure, the function will return FALSE. Call GetLastError() to the error code.
//
WINUHID_API BOOL WinUHidPS5ReportInput(PWINUHID_PS5_GAMEPAD Gamepad, PCWINUHID_PS5_INPUT_REPORT Report);

//
// Destroys the gamepad.
//
// This function never fails as long as the provided argument is valid.
//
WINUHID_API VOID WinUHidPS5Destroy(PWINUHID_PS5_GAMEPAD Gamepad);