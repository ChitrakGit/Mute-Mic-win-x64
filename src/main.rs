#![windows_subsystem = "windows"]

// ============================================================================
// muteMic - A simple Windows GUI app to mute/unmute the microphone
// ============================================================================
//
// How it works:
//   1. Initializes COM (Component Object Model) — required to use Windows audio APIs.
//   2. Gets the default microphone device and its volume controller.
//   3. Creates a simple window with "Mute" and "Unmute" buttons + a status label.
//   4. Runs a Win32 message loop to handle button clicks and window events.
//   5. Automatically unmutes the mic when the app exits (window close, Ctrl+C, etc.)
//
// Key concepts:
//   - COM (Component Object Model): Windows API framework for inter-process communication.
//     Must call CoInitialize before using COM objects and CoUninitialize when done.
//   - IAudioEndpointVolume: COM interface to control volume/mute of an audio device.
//   - AgileReference: A thread-safe wrapper that lets us use COM objects across threads.
//   - Win32 Window Procedure (wndproc): Callback function that handles window messages
//     like button clicks (WM_COMMAND), window creation (WM_CREATE), and close (WM_DESTROY).
// ============================================================================

// --- Standard library imports ---
use std::ptr::null;       // Used as a null pointer for COM method calls that need a GUID parameter
use std::sync::OnceLock;  // Thread-safe, write-once global storage (initialized once, read many times)

// --- Windows crate imports ---
// Core types and macros
use windows::core::{w, AgileReference, Result}; // w!() macro creates wide string literals; AgileReference: thread-safe COM wrapper; Result: Windows error type

// Foundation types — basic Win32 handle and message types
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, WPARAM, POINT};
// BOOL: Win32 boolean (not Rust's bool), HWND: window handle,
// LPARAM/WPARAM: message parameters, LRESULT: message return value, POINT: x,y coordinates

// Menu and UI interaction
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, AppendMenuW, TrackPopupMenu,
    SetForegroundWindow, MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    WM_HOTKEY,
};

// Global Hotkeys API
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_SHIFT, HOT_KEY_MODIFIERS
};

// System tray (Shell) API
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW,
    NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NIF_MESSAGE, NIF_ICON, NIF_TIP,
};

// IAudioEndpointVolume: The COM interface that controls mute/unmute and volume of an audio device
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;

// COM initialization and object creation functions
// Moved to src/audio.rs

// Console control handler — lets us intercept Ctrl+C, window close, logoff, shutdown
use windows::Win32::System::Console::{
    SetConsoleCtrlHandler,  // Registers our callback for console events
    CTRL_C_EVENT,           // Ctrl+C pressed
    CTRL_CLOSE_EVENT,       // Console window closed
    CTRL_LOGOFF_EVENT,      // User logs off
    CTRL_SHUTDOWN_EVENT,    // System shutting down
};

// GetModuleHandleW: Gets the handle of the current executable (needed for window creation)
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

// Audio device enumeration types
// Moved to src/audio.rs

// Win32 windowing functions and constants
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW,      // Creates a window or child control (button, label, etc.)
    DefWindowProcW,       // Default handler for messages we don't process ourselves
    DispatchMessageW,     // Sends a message to the window's wndproc
    GetMessageW,          // Waits for and retrieves the next message from the message queue
    GetWindowLongPtrW,    // Retrieves data stored with the window (we store AppState pointer)
    LoadCursorW,          // Loads a system cursor (arrow, hand, etc.)
    PostQuitMessage,      // Posts WM_QUIT to exit the message loop
    RegisterClassW,       // Registers our window class with Windows
    SetWindowLongPtrW,    // Stores data (AppState pointer) with the window
    TranslateMessage,     // Translates virtual-key messages into character messages
    CS_HREDRAW,           // Redraw entire window when width changes
    CS_VREDRAW,           // Redraw entire window when height changes
    GWLP_USERDATA,        // Index for storing our custom data (AppState) with the window
    IDC_ARROW,            // Standard arrow cursor
    MSG,                  // Message structure for the message loop
    WM_COMMAND,           // Message sent when a menu item is clicked
    WM_CREATE,            // Message sent when the window is first created
    WM_DESTROY,           // Message sent when the window is being destroyed (closed)
    WNDCLASSW,            // Structure that defines our window class
    IDI_APPLICATION,      // Default application icon
};

// ============================================================================
// AppState — holds everything our window needs to operate
// ============================================================================
// This struct is heap-allocated (Box<AppState>) and its raw pointer is stored
// with the window via SetWindowLongPtrW. We retrieve it in WM_COMMAND and
// WM_DESTROY to access the volume controller and tray icon data.
struct AppState {
    volume_control: IAudioEndpointVolume,  // COM interface to mute/unmute the mic
    nid: NOTIFYICONDATAW,                  // System tray icon data (needed to modify/delete the icon)
}

// ============================================================================
// main() — Entry point: sets up COM, gets the mic, creates the window, runs the message loop
// ============================================================================
fn main() -> Result<()> {
    unsafe {
        // Step 1: Initialize COM library for this thread.
        muteMic::audio::init_com();

        // Step 2-4: Get the default microphone device volume controller.
        let volume_control = muteMic::audio::get_default_mic_volume()?;

        // Step 5: Store the volume controller in a global static variable.
        // We wrap it in an AgileReference so it can be safely used from any thread.
        // OnceLock ensures this is only set once and is thread-safe.
        let _ = VOLUME_CONTROL.set(AgileReference::new(&volume_control)?);

        // Step 6: Register a console control handler.
        // This ensures we unmute the mic even if the app is killed via Ctrl+C,
        // the console window is closed, or the system is shutting down.
        let _ = SetConsoleCtrlHandler(Some(console_ctrl_handler), true);

        // Step 7: Create the GUI window (see create_window() below).
        create_window()?;

        // Step 8: Run the Win32 message loop.
        // GetMessageW blocks until a message is available. It returns false when
        // WM_QUIT is received (posted by PostQuitMessage in WM_DESTROY).
        // TranslateMessage converts keyboard messages, DispatchMessageW sends
        // the message to our wndproc callback for processing.
        let mut message = MSG::default();
        while GetMessageW(&mut message, HWND(0), 0, 0).into() {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        // Step 9: Safety net — unmute the mic after the message loop exits.
        // This handles edge cases where WM_DESTROY might not have fired properly.
        unmute_mic();
    }

    // Step 10: Uninitialize COM.
    muteMic::audio::uninit_com();

    Ok(())
}

// ============================================================================
// Constants — Menu IDs and custom messages
// ============================================================================
const ID_MUTE: u16 = 1001;
const ID_UNMUTE: u16 = 1002;
const ID_EXIT: u16 = 1003;
const ID_HOTKEY_MUTE: i32 = 1;

// Custom message ID for tray icon events (WM_USER + 1)
const WM_TRAYICON: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 1;
// Right-click event constant
const WM_RBUTTONUP: u32 = windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP;

// ============================================================================
// Global volume controller — accessible from any function in the app
// ============================================================================
// OnceLock<T>: Can only be written to once, then read from anywhere. Thread-safe.
// AgileReference<T>: Wraps a COM interface so it can cross thread boundaries safely.
//   When we need the actual interface, we call .resolve() to get a thread-local copy.
static VOLUME_CONTROL: OnceLock<AgileReference<IAudioEndpointVolume>> = OnceLock::new();

// ============================================================================
// create_window() — Registers a window class and creates the main application window
// ============================================================================
fn create_window() -> Result<()> {
    unsafe {
        // Get the handle to our executable — needed to associate the window with our app
        let instance = GetModuleHandleW(None)?;

        // Define a unique class name for our window type
        let class_name = w!("MuteMicWindowClass");

        // Fill in the WNDCLASSW structure that describes our window class:
        //   - style: CS_HREDRAW | CS_VREDRAW = redraw when resized
        //   - lpfnWndProc: pointer to our message handler function (wndproc)
        //   - hInstance: which executable this window belongs to
        //   - lpszClassName: unique name to identify this window class
        //   - hCursor: the mouse cursor to show (standard arrow)
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default() // Zero out all other fields
        };

        // Register the window class with Windows.
        // Returns 0 on failure, a non-zero "atom" on success.
        let atom = RegisterClassW(&wc);
        if atom == 0 {
            return Err(windows::core::Error::from_win32());
        }

        // Create the hidden message-only window.
        //   - Default::default(): no visible style
        //   - class_name: the class we just registered
        //   - title: "MuteMicHidden"
        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            w!("MuteMic"),
            windows::Win32::UI::WindowsAndMessaging::WS_POPUP,
            0, 0, 0, 0,
            HWND(0),
            None,
            instance,
            None,
        );

        // Register our global hotkey (Ctrl + Shift + M)
        // 0x4D is the virtual key code for 'M'
        let modifiers = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_SHIFT.0);
        let _ = RegisterHotKey(hwnd, ID_HOTKEY_MUTE, modifiers, 0x4D);

        Ok(())
    }
}

// ============================================================================
// show_tray_menu() — Displays the right-click context menu for the tray icon
// ============================================================================
fn show_tray_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu().unwrap();
        AppendMenuW(menu, MF_STRING, ID_MUTE as usize, w!("Mute")).unwrap();
        AppendMenuW(menu, MF_STRING, ID_UNMUTE as usize, w!("Unmute")).unwrap();
        AppendMenuW(menu, MF_STRING, ID_EXIT as usize, w!("Exit")).unwrap();

        let mut pt = POINT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);

        // Required: Makes the menu close when clicking outside
        SetForegroundWindow(hwnd);
        TrackPopupMenu(menu, TPM_BOTTOMALIGN | TPM_LEFTALIGN,
                       pt.x, pt.y, 0, hwnd, None);
    }
}

// ============================================================================
// wndproc() — Window procedure: handles all messages sent to our window
// ============================================================================
unsafe extern "system" fn wndproc(
    hwnd: HWND,       // Handle to the window receiving the message
    msg: u32,         // The message type (WM_CREATE, WM_COMMAND, WM_DESTROY, etc.)
    wparam: WPARAM,   // Additional message data (e.g., which button was clicked)
    lparam: LPARAM,   // Additional message data (e.g., pointer to creation struct)
) -> LRESULT {
    match msg {
        // ----------------------------------------------------------------
        // WM_CREATE — Window is being created. Set up the tray icon.
        // ----------------------------------------------------------------
        WM_CREATE => {
            let agile_ref = VOLUME_CONTROL.get().expect("Volume control not initialized");
            let volume_control = agile_ref.resolve().expect("Failed to resolve agile reference");

            let is_muted = volume_control.GetMute().unwrap_or(BOOL(0)).as_bool();
            let tip_text = if is_muted { "MuteMic - Muted" } else { "MuteMic - Unmuted" };

            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAYICON;
            
            nid.hIcon = windows::Win32::UI::WindowsAndMessaging::LoadIconW(None, IDI_APPLICATION).unwrap();

            for (i, c) in tip_text.encode_utf16().enumerate() {
                if i >= nid.szTip.len() - 1 { break; }
                nid.szTip[i] = c;
            }

            Shell_NotifyIconW(NIM_ADD, &nid);

            let app_state = Box::new(AppState {
                volume_control,
                nid,
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(app_state) as _);

            LRESULT(0)
        }

        // ----------------------------------------------------------------
        // WM_COMMAND — A menu item was clicked. Mute/Unmute/Exit.
        // ----------------------------------------------------------------
        WM_COMMAND => {
            let app_state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
            if !app_state_ptr.is_null() {
                let app_state = &mut *app_state_ptr;
                let command = wparam.0 as u16;

                let mut update_tip = false;
                let mut tip_text = "";

                if command == ID_MUTE {
                    let _ = app_state.volume_control.SetMute(true, null());
                    tip_text = "MuteMic - Muted";
                    update_tip = true;
                } else if command == ID_UNMUTE {
                    let _ = app_state.volume_control.SetMute(false, null());
                    tip_text = "MuteMic - Unmuted";
                    update_tip = true;
                } else if command == ID_EXIT {
                    let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
                }

                if update_tip {
                    app_state.nid.szTip.fill(0);
                    for (i, c) in tip_text.encode_utf16().enumerate() {
                        if i >= app_state.nid.szTip.len() - 1 { break; }
                        app_state.nid.szTip[i] = c;
                    }
                    let _ = Shell_NotifyIconW(NIM_MODIFY, &app_state.nid);
                }
            }
            LRESULT(0)
        }

        // ----------------------------------------------------------------
        // WM_HOTKEY — Our global hotkey was pressed
        // ----------------------------------------------------------------
        WM_HOTKEY => {
            let app_state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
            if !app_state_ptr.is_null() {
                let app_state = &mut *app_state_ptr;
                if wparam.0 as i32 == ID_HOTKEY_MUTE {
                    let is_muted = app_state.volume_control.GetMute().unwrap_or(BOOL(0)).as_bool();
                    let _ = app_state.volume_control.SetMute(!is_muted, null());
                    
                    // Update tooltip
                    let tip_text = if !is_muted { "MuteMic - Muted" } else { "MuteMic - Unmuted" };
                    app_state.nid.szTip.fill(0);
                    for (i, c) in tip_text.encode_utf16().enumerate() {
                        if i >= app_state.nid.szTip.len() - 1 { break; }
                        app_state.nid.szTip[i] = c;
                    }
                    let _ = Shell_NotifyIconW(NIM_MODIFY, &app_state.nid);
                }
            }
            LRESULT(0)
        }

        // ----------------------------------------------------------------
        // WM_TRAYICON — Custom message from the system tray icon
        // ----------------------------------------------------------------
        WM_TRAYICON => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if event == WM_RBUTTONUP {
                show_tray_menu(hwnd);
            }
            LRESULT(0)
        }

        // ----------------------------------------------------------------
        // WM_DESTROY — Window is being closed. Clean up tray icon and unmute.
        // ----------------------------------------------------------------
        WM_DESTROY => {
            let app_state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
            if !app_state_ptr.is_null() {
                let app_state = &*app_state_ptr;
                let _ = app_state.volume_control.SetMute(false, null());
                Shell_NotifyIconW(NIM_DELETE, &app_state.nid);
                
                drop(Box::from_raw(app_state_ptr));
            }
            let _ = UnregisterHotKey(hwnd, ID_HOTKEY_MUTE);
            PostQuitMessage(0);
            LRESULT(0)
        }

        // ----------------------------------------------------------------
        // All other messages — let Windows handle them with default behavior.
        // ----------------------------------------------------------------
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ============================================================================
// unmute_mic() — Helper function to unmute the microphone from anywhere
// ============================================================================
// Uses the global VOLUME_CONTROL to resolve a thread-local COM proxy
// and set mute to false. Called from:
//   1. WM_DESTROY (window close)
//   2. console_ctrl_handler (Ctrl+C, console close, logoff, shutdown)
//   3. After the message loop exits (safety net in main)
fn unmute_mic() {
    // Check if the global volume controller was initialized
    if let Some(agile_ref) = VOLUME_CONTROL.get() {
        // Resolve the AgileReference to get a usable COM interface for this thread
        if let Ok(volume_control) = agile_ref.resolve() {
            unsafe {
                // Set mute to false (unmute). null() = no event context GUID needed.
                let _ = volume_control.SetMute(false, null());
            }
        }
    }
}

// ============================================================================
// console_ctrl_handler() — Handles console events to unmute mic on forced exit
// ============================================================================
// This is registered via SetConsoleCtrlHandler in main().
// It runs when the user presses Ctrl+C, closes the console window,
// logs off, or the system shuts down — cases where WM_DESTROY might
// not fire because the window message loop gets interrupted.
//
// Returns BOOL(1) = "we handled it", BOOL(0) = "pass to next handler".
unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
    match ctrl_type {
        // Match any of the termination events
        x if x == CTRL_C_EVENT          // User pressed Ctrl+C
            || x == CTRL_CLOSE_EVENT    // Console window X button clicked
            || x == CTRL_LOGOFF_EVENT   // User is logging off
            || x == CTRL_SHUTDOWN_EVENT // System is shutting down
        =>
        {
            unmute_mic();  // Ensure mic is unmuted before the process dies
            BOOL(1)        // Return TRUE = we handled the event
        }
        _ => BOOL(0), // Unknown event — let the default handler deal with it
    }
}