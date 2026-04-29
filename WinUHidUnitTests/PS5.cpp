#include "pch.h"
#include "Utilities.h"

TEST(PS5, CreateBasic) {
	auto gamepad = WinUHidPS5Create(NULL, NULL, NULL, NULL, NULL, NULL, NULL);
	ASSERT_TRUE(gamepad) << "Failed to create PS5 gamepad";

	SDLGamepadManager gm;
	ASSERT_EQ(gm.GetGamepadCount(), 1) << "Unable to detect PS5 gamepad with SDL";

	EXPECT_EQ(SDL_GetGamepadVendor(gm.GetGamepad(0)), 0x054c); // Sony
	EXPECT_EQ(SDL_GetGamepadProduct(gm.GetGamepad(0)), 0x0ce6); // DualSense
	EXPECT_EQ(SDL_GetGamepadProductVersion(gm.GetGamepad(0)), 0);

	WinUHidPS5Destroy(gamepad);
}

TEST(PS5, CreateAdvanced) {
	WINUHID_PRESET_DEVICE_INFO basicInfo = {
		0x054c,
		0x0df2, // DualSense Edge
		1, // HID version
		{},
		NULL,
		NULL
	};

	WINUHID_PS5_GAMEPAD_INFO PS5Info = {
		&basicInfo,
		{ 1, 2, 3, 4, 5, 6 } // MAC address
	};
	auto gamepad = WinUHidPS5Create(&PS5Info, NULL, NULL, NULL, NULL, NULL, NULL);
	ASSERT_TRUE(gamepad) << "Failed to create PS5 gamepad";

	SDLGamepadManager gm;
	ASSERT_EQ(gm.GetGamepadCount(), 1) << "Unable to detect PS5 gamepad with SDL";

	EXPECT_EQ(SDL_GetGamepadVendor(gm.GetGamepad(0)), 0x054c); // Sony
	EXPECT_EQ(SDL_GetGamepadProduct(gm.GetGamepad(0)), 0x0df2); // DualSense Edge
	EXPECT_EQ(SDL_GetGamepadProductVersion(gm.GetGamepad(0)), 1);
	EXPECT_EQ(std::string{ SDL_GetGamepadSerial(gm.GetGamepad(0)) }, "01-02-03-04-05-06");

	WinUHidPS5Destroy(gamepad);
}

TEST(PS5, ButtonMapping) {
	WINUHID_PRESET_DEVICE_INFO basicInfo = {
		0x054c,
		0x0df2, // DualSense Edge
		1, // HID version
		{},
		NULL,
		NULL
	};

	WINUHID_PS5_GAMEPAD_INFO PS5Info = {
		&basicInfo
	};

	auto gamepad = WinUHidPS5Create(&PS5Info, NULL, NULL, NULL, NULL, NULL, NULL);
	ASSERT_TRUE(gamepad) << "Failed to create PS5 gamepad";

	SDLGamepadManager gm;
	ASSERT_EQ(gm.GetGamepadCount(), 1) << "Unable to detect PS5 gamepad with SDL";

	WINUHID_PS5_INPUT_REPORT report;
	WinUHidPS5InitializeInputReport(&report);

	for (int x = -1; x <= 1; x++) {
		for (int y = -1; y <= 1; y++) {
			WinUHidPS5SetHatState(&report, x, y);
			ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
			gm.ExpectHatState(x, y);
		}
	}

	report.ButtonCross = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_SOUTH, !!report.ButtonCross);

	report.ButtonCircle = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_EAST, !!report.ButtonCircle);

	report.ButtonTriangle = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_NORTH, !!report.ButtonTriangle);

	report.ButtonSquare = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_WEST, !!report.ButtonSquare);

	report.ButtonShare = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_BACK, !!report.ButtonShare);

	report.ButtonOptions = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_START, !!report.ButtonOptions);

	report.ButtonHome = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_GUIDE, !!report.ButtonHome);

	report.ButtonL1 = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_LEFT_SHOULDER, !!report.ButtonL1);

	report.ButtonR1 = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER, !!report.ButtonR1);

	report.ButtonL3 = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_LEFT_STICK, !!report.ButtonL3);

	report.ButtonR3 = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_RIGHT_STICK, !!report.ButtonR3);

	report.ButtonTouchpad = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_TOUCHPAD, !!report.ButtonTouchpad);

	report.ButtonMute = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_MISC1, !!report.ButtonMute);

	//
	// These additional buttons require emulating a DualSense Edge
	//

	report.ButtonLeftFunction = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_LEFT_PADDLE2, !!report.ButtonLeftFunction);

	report.ButtonRightFunction = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_RIGHT_PADDLE2, !!report.ButtonRightFunction);

	report.ButtonLeftPaddle = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_LEFT_PADDLE1, !!report.ButtonLeftPaddle);

	report.ButtonRightPaddle = 1;
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectButtonState(SDL_GAMEPAD_BUTTON_RIGHT_PADDLE1, !!report.ButtonRightPaddle);

	WinUHidPS5Destroy(gamepad);
}

//
// The magic values here match SDL3's parsing logic
//
#define SDL_EXPECTED_STICK_VAL(x) (((int)(x) * 257) - 32768)
#define SDL_EXPECTED_TRIGGER_VAL(x) (((int)(x) * 0x7FFF) / 0xFF)
TEST(PS5, AxisMapping) {
	auto gamepad = WinUHidPS5Create(NULL, NULL, NULL, NULL, NULL, NULL, NULL);
	ASSERT_TRUE(gamepad) << "Failed to create PS5 gamepad";

	SDLGamepadManager gm;
	ASSERT_EQ(gm.GetGamepadCount(), 1) << "Unable to detect PS5 gamepad with SDL";

	WINUHID_PS5_INPUT_REPORT report;
	WinUHidPS5InitializeInputReport(&report);

	for (Uint16 value = 0; value <= 0xFF; value++) {
		report.LeftStickX = (Uint8)value;
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
		gm.ExpectAxisValue(SDL_GAMEPAD_AXIS_LEFTX, SDL_EXPECTED_STICK_VAL(value));
	}

	for (Uint16 value = 0; value <= 0xFF; value++) {
		report.LeftStickY = (Uint8)value;
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
		gm.ExpectAxisValue(SDL_GAMEPAD_AXIS_LEFTY, SDL_EXPECTED_STICK_VAL(value));
	}

	for (Uint16 value = 0; value <= 0xFF; value++) {
		report.RightStickX = (Uint8)value;
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
		gm.ExpectAxisValue(SDL_GAMEPAD_AXIS_RIGHTX, SDL_EXPECTED_STICK_VAL(value));
	}

	for (Uint16 value = 0; value <= 0xFF; value++) {
		report.RightStickY = (Uint8)value;
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
		gm.ExpectAxisValue(SDL_GAMEPAD_AXIS_RIGHTY, SDL_EXPECTED_STICK_VAL(value));
	}



	for (Uint16 value = 0; value <= 0xFF; value++) {
		if (value == 1) {
			//
			// FIXME: Figure out why SDL returns 0 for a raw trigger value of 1
			//
			continue;
		}

		report.LeftTrigger = (Uint8)value;
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
		gm.ExpectAxisValue(SDL_GAMEPAD_AXIS_LEFT_TRIGGER, SDL_EXPECTED_TRIGGER_VAL(value));
	}

	for (Uint16 value = 0; value <= 0xFF; value++) {
		if (value == 1) {
			//
			// FIXME: Figure out why SDL returns 0 for a raw trigger value of 1
			//
			continue;
		}

		report.RightTrigger = (Uint8)value;
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
		gm.ExpectAxisValue(SDL_GAMEPAD_AXIS_RIGHT_TRIGGER, SDL_EXPECTED_TRIGGER_VAL(value));
	}

	WinUHidPS5Destroy(gamepad);
}

TEST(PS5, SensorMapping) {
	auto gamepad = WinUHidPS5Create(NULL, NULL, NULL, NULL, NULL, NULL, NULL);
	ASSERT_TRUE(gamepad) << "Failed to create PS5 gamepad";

	SDLGamepadManager gm;
	ASSERT_EQ(gm.GetGamepadCount(), 1) << "Unable to detect PS5 gamepad with SDL";

	//
	// Sensors must be enabled before they can be read
	//
	ASSERT_TRUE(SDL_SetGamepadSensorEnabled(gm.GetGamepad(0), SDL_SENSOR_GYRO, true));
	ASSERT_TRUE(SDL_SetGamepadSensorEnabled(gm.GetGamepad(0), SDL_SENSOR_ACCEL, true));

	WINUHID_PS5_INPUT_REPORT report;
	WinUHidPS5InitializeInputReport(&report);

	for (float value = -28; value <= 28; value += 0.25) {
		WinUHidPS5SetGyroState(&report, value, 0, 0);
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);

		float expected[3] = { value, 0, 0 };
		gm.ExpectSensorData(SDL_SENSOR_GYRO, expected);
	}

	for (float value = -28; value <= 28; value += 0.25) {
		WinUHidPS5SetGyroState(&report, 0, value, 0);
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);

		float expected[3] = { 0, value, 0 };
		gm.ExpectSensorData(SDL_SENSOR_GYRO, expected);
	}

	for (float value = -28; value <= 28; value += 0.25) {
		WinUHidPS5SetGyroState(&report, 0, 0, value);
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);

		float expected[3] = { 0, 0, value };
		gm.ExpectSensorData(SDL_SENSOR_GYRO, expected);
	}

	for (float value = -32; value <= 32; value += 0.25) {
		WinUHidPS5SetAccelState(&report, value, 0, 0);
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);

		float expected[3] = { value, 0, 0 };
		gm.ExpectSensorData(SDL_SENSOR_ACCEL, expected);
	}

	for (float value = -32; value <= 32; value += 0.25) {
		WinUHidPS5SetAccelState(&report, 0, value, 0);
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);

		float expected[3] = { 0, value, 0 };
		gm.ExpectSensorData(SDL_SENSOR_ACCEL, expected);
	}

	for (float value = -32; value <= 32; value += 0.25) {
		WinUHidPS5SetAccelState(&report, 0, 0, value);
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);

		float expected[3] = { 0, 0, value };
		gm.ExpectSensorData(SDL_SENSOR_ACCEL, expected);
	}

	WinUHidPS5Destroy(gamepad);
}

//
// The touchpad is 1080 points tall, but SDL treats it as 1070. Since we want to
// validate against SDL's representation, we must also use 1070 as the height.
//

#define TOUCHPAD_WIDTH 1920
#define TOUCHPAD_HEIGHT 1070

#define TP_X_AS_FLOAT(x) ((x) / 1920.f)
#define TP_Y_AS_FLOAT(y) ((y) / 1070.f)

TEST(PS5, TouchpadMapping) {
	auto gamepad = WinUHidPS5Create(NULL, NULL, NULL, NULL, NULL, NULL, NULL);
	ASSERT_TRUE(gamepad) << "Failed to create PS5 gamepad";

	SDLGamepadManager gm;
	ASSERT_EQ(gm.GetGamepadCount(), 1) << "Unable to detect PS5 gamepad with SDL";

	WINUHID_PS5_INPUT_REPORT report;
	WinUHidPS5InitializeInputReport(&report);

	//
	// It's too expensive to exhaustively test all coordinates, so we'll test (literally) edge cases
	//

	auto testCoordinates = [&](Uint16 x, Uint16 y) {
		WinUHidPS5SetTouchState(&report, 0, TRUE, x, y);
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
		gm.ExpectTouchpadFingerState(0, true, TP_X_AS_FLOAT(x), TP_Y_AS_FLOAT(y));
		gm.ExpectTouchpadFingerState(1, false, 0, 0);

		WinUHidPS5SetTouchState(&report, 1, TRUE, TOUCHPAD_WIDTH - x, TOUCHPAD_HEIGHT - y);
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
		gm.ExpectTouchpadFingerState(0, true, TP_X_AS_FLOAT(x), TP_Y_AS_FLOAT(y));
		gm.ExpectTouchpadFingerState(1, true, TP_X_AS_FLOAT(TOUCHPAD_WIDTH - x), TP_Y_AS_FLOAT(TOUCHPAD_HEIGHT - y));

		WinUHidPS5SetTouchState(&report, 1, FALSE, x, y);
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
		gm.ExpectTouchpadFingerState(0, true, TP_X_AS_FLOAT(x), TP_Y_AS_FLOAT(y));
		gm.ExpectTouchpadFingerState(1, false, 0, 0);

		WinUHidPS5SetTouchState(&report, 0, FALSE, x, y);
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
		gm.ExpectTouchpadFingerState(0, false, 0, 0);
		gm.ExpectTouchpadFingerState(1, false, 0, 0);
		};

	for (Uint16 x = 0; x <= TOUCHPAD_WIDTH; x++) {
		testCoordinates(x, 0);
		testCoordinates(x, TOUCHPAD_HEIGHT);
	}

	for (Uint16 y = 0; y <= TOUCHPAD_HEIGHT; y++) {
		testCoordinates(0, 0);
		testCoordinates(TOUCHPAD_WIDTH, y);
	}

	testCoordinates(TOUCHPAD_WIDTH / 2, TOUCHPAD_HEIGHT / 2);

	//
	// Test dragging and ensure our touch sequence number will overflow too
	//
	for (int i = 0; i < 0xFF; i++) {
		WinUHidPS5SetTouchState(&report, 0, TRUE, i, i);
		ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
		gm.ExpectTouchpadFingerState(0, true, TP_X_AS_FLOAT(i), TP_Y_AS_FLOAT(i));
	}

	WinUHidPS5SetTouchState(&report, 0, FALSE, 0, 0);
	ASSERT_EQ(WinUHidPS5ReportInput(gamepad, &report), TRUE);
	gm.ExpectTouchpadFingerState(0, false, 0, 0);
	gm.ExpectTouchpadFingerState(1, false, 0, 0);

	WinUHidPS5Destroy(gamepad);
}

#define MAKE_LED_VALUE(r, g, b) ((r) << 16 | (g) << 8 | (b))

TEST(PS5, LedEffects) {
	CallbackData<UINT> ledState;

	auto gamepad = WinUHidPS5Create(NULL, NULL,
		[](PVOID CallbackContext, UCHAR LedRed, UCHAR LedGreen, UCHAR LedBlue) {
			((CallbackData<UINT>*)CallbackContext)->Signal(MAKE_LED_VALUE(LedRed, LedGreen, LedBlue));
		}, NULL, NULL, NULL, &ledState);
	ASSERT_TRUE(gamepad) << "Failed to create PS5 gamepad";

	SDLGamepadManager gm;
	ASSERT_EQ(gm.GetGamepadCount(), 1) << "Unable to detect PS5 gamepad with SDL";

	for (Uint16 i = 0; i <= 0xFF; i++) {
		ASSERT_TRUE(SDL_SetGamepadLED(gm.GetGamepad(0), (Uint8)i, 0, 0));
		EXPECT_CB_VALUE(ledState, MAKE_LED_VALUE(i, 0, 0));
	}

	for (Uint16 i = 0; i <= 0xFF; i++) {
		ASSERT_TRUE(SDL_SetGamepadLED(gm.GetGamepad(0), 0, (Uint8)i, 0));
		EXPECT_CB_VALUE(ledState, MAKE_LED_VALUE(0, i, 0));
	}

	for (Uint16 i = 0; i <= 0xFF; i++) {
		ASSERT_TRUE(SDL_SetGamepadLED(gm.GetGamepad(0), 0, 0, (Uint8)i));
		EXPECT_CB_VALUE(ledState, MAKE_LED_VALUE(0, 0, i));
	}

	WinUHidPS5Destroy(gamepad);
}

TEST(PS5, PlayerLedEffects) {
	CallbackData<UCHAR> playerLedState;

	auto gamepad = WinUHidPS5Create(NULL, NULL, NULL,
		[](PVOID CallbackContext, UCHAR LedValue) {
			//
			// Mask the LED value to get only the enabled LED bits
			//
			((CallbackData<UCHAR>*)CallbackContext)->Signal(LedValue & 0x1F);
		}, NULL, NULL, &playerLedState);
	ASSERT_TRUE(gamepad) << "Failed to create PS5 gamepad";

	SDLGamepadManager gm;
	ASSERT_EQ(gm.GetGamepadCount(), 1) << "Unable to detect PS5 gamepad with SDL";

	//
	// These are the values used by hid-playstation and SDL
	//
	UCHAR playerIndexLightValues[] = {
		0x04,
		0x0A,
		0x15,
		0x1B,
		0x1F
	};

	//
	// Verify the initial player index set by SDL (should be 0)
	//
	ASSERT_EQ(SDL_GetGamepadPlayerIndex(gm.GetGamepad(0)), 0);
	EXPECT_CB_VALUE(playerLedState, playerIndexLightValues[0]);

	//
	// Turn off player LEDs
	//
	ASSERT_TRUE(SDL_SetGamepadPlayerIndex(gm.GetGamepad(0), -1));
	EXPECT_CB_VALUE(playerLedState, 0);

	//
	// Walk through the player index values that have distinct LED states
	//
	for (int i = 0; i < ARRAYSIZE(playerIndexLightValues); i++) {
		ASSERT_TRUE(SDL_SetGamepadPlayerIndex(gm.GetGamepad(0), i));
		EXPECT_CB_VALUE(playerLedState, playerIndexLightValues[i]);
	}

	WinUHidPS5Destroy(gamepad);
}

#define MAKE_RUMBLE_VALUE(l, r) ((l) << 8 | (r))

TEST(PS5, RumbleEffects) {
	CallbackData<UINT> rumbleState;

	auto gamepad = WinUHidPS5Create(NULL,
		[](PVOID CallbackContext, UCHAR LeftMotor, UCHAR RightMotor) {
			((CallbackData<UINT>*)CallbackContext)->Signal(MAKE_RUMBLE_VALUE(LeftMotor, RightMotor));
		}, NULL, NULL, NULL, NULL, &rumbleState);
	ASSERT_TRUE(gamepad) << "Failed to create PS5 gamepad";

	SDLGamepadManager gm;
	ASSERT_EQ(gm.GetGamepadCount(), 1) << "Unable to detect PS5 gamepad with SDL";

	for (Uint16 i = 1; i <= 0xFF; i++) {
		ASSERT_TRUE(SDL_RumbleGamepad(gm.GetGamepad(0), i << 8, 0, 100));
		EXPECT_CB_VALUE(rumbleState, MAKE_RUMBLE_VALUE(i, 0));
	}

	for (Uint16 i = 1; i <= 0xFF; i++) {
		ASSERT_TRUE(SDL_RumbleGamepad(gm.GetGamepad(0), 0, i << 8, 100));
		EXPECT_CB_VALUE(rumbleState, MAKE_RUMBLE_VALUE(0, i));
	}

	//
	// Rumble will cease after 100ms
	//
	EXPECT_CB_VALUE(rumbleState, MAKE_RUMBLE_VALUE(0, 0));

	WinUHidPS5Destroy(gamepad);
}