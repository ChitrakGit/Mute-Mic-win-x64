# Add Global Hotkey Functionality

This plan details how we will add a global hotkey (`Ctrl + Shift + M`) to toggle the microphone mute state. The hotkey will work from anywhere in Windows as long as the application is running in the background.

## Proposed Changes

### 1. `Cargo.toml`
Add the keyboard and mouse input API feature to the `windows` crate dependencies so we can register global hotkeys.

#### [MODIFY] [Cargo.toml](file:///c:/Company%20Files/Study/rust/muteMic/Cargo.toml)
- Add `"Win32_UI_Input_KeyboardAndMouse"` to the `windows` crate features.

### 2. `src/main.rs`
Update the application to register the hotkey and listen for hotkey events.

#### [MODIFY] [main.rs](file:///c:/Company%20Files/Study/rust/muteMic/src/main.rs)
- **Imports:** Add `RegisterHotKey`, `UnregisterHotKey`, `MOD_CONTROL`, `MOD_SHIFT`, and `HOT_KEY_MODIFIERS` from `windows::Win32::UI::Input::KeyboardAndMouse`, and `WM_HOTKEY` from `windows::Win32::UI::WindowsAndMessaging`.
- **Constants:** Define the hotkey ID (`const ID_HOTKEY_MUTE: i32 = 1;`).
- **Initialization:** In the `create_window` function, immediately after the window is created successfully, call `RegisterHotKey(hwnd, ID_HOTKEY_MUTE, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, 0x4D)` (where `0x4D` is the 'M' key).
- **Event Handling:** In the `wndproc` message loop, add a case for `WM_HOTKEY`. 
  - When triggered, it will read the current mic state.
  - It will toggle the mute state.
  - It will update the system tray icon's tooltip to reflect the new state.
- **Cleanup:** In the `WM_DESTROY` case, call `UnregisterHotKey(hwnd, ID_HOTKEY_MUTE)` to cleanly release the hotkey back to the operating system before exiting.

### 3. `SYSTEM_TRAY_GUIDE.md`
Update the documentation to cover the new hotkey functionality.

#### [MODIFY] [SYSTEM_TRAY_GUIDE.md](file:///c:/Company%20Files/Study/rust/muteMic/SYSTEM_TRAY_GUIDE.md)
- Add a section on "Global Hotkeys" explaining the `RegisterHotKey` API and how `WM_HOTKEY` is processed in the message loop.

## Verification Plan

### Manual Verification
1. Run the application (`cargo run --release`).
2. Switch to a different application (e.g., Notepad or a web browser) to ensure `muteMic` is purely in the background.
3. Press `Ctrl + Shift + M`.
4. Hover over the system tray icon to verify the tooltip correctly updated to "MuteMic - Muted".
5. Press `Ctrl + Shift + M` again and verify it toggles back to "Unmuted".
6. Right-click the tray icon and select "Exit", verifying the hotkey is unregistered and no longer functions.
