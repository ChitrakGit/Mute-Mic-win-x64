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
use windows::core::{w, HSTRING};           // w!() macro creates wide string literals; HSTRING is a heap-allocated wide string
use windows::core::{AgileReference, Result}; // AgileReference: thread-safe COM wrapper; Result: Windows error type

// Foundation types — basic Win32 handle and message types
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, WPARAM};
// BOOL: Win32 boolean (not Rust's bool), HWND: window handle,
// LPARAM/WPARAM: message parameters, LRESULT: message return value

// HMENU: Handle to a menu — also used as a control ID when creating child windows (buttons, labels)
use windows::Win32::UI::WindowsAndMessaging::HMENU;

// IAudioEndpointVolume: The COM interface that controls mute/unmute and volume of an audio device
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;

// COM initialization and object creation functions
use windows::Win32::System::Com::{
    CoCreateInstance,       // Creates a COM object (here: the device enumerator)
    CoInitialize,           // Initializes COM for the current thread
    CLSCTX_INPROC_SERVER,  // Flag: create COM object in the same process
    CoUninitialize,         // Cleans up COM when we're done
};

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
use windows::Win32::Media::Audio::{
    eCapture,              // We want a capture device (microphone), not a playback device (speakers)
    eConsole,              // We want the default device for the "console" role (general use)
    IMMDeviceEnumerator,   // COM interface to list/find audio devices
    MMDeviceEnumerator,    // CLSID (class identifier) to create the device enumerator
};

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
    SetWindowTextW,       // Changes the text of a window or control (used for status label)
    ShowWindow,           // Shows or hides a window
    TranslateMessage,     // Translates virtual-key messages into character messages
    CS_HREDRAW,           // Redraw entire window when width changes
    CS_VREDRAW,           // Redraw entire window when height changes
    CW_USEDEFAULT,        // Let Windows choose the default position
    GWLP_USERDATA,        // Index for storing our custom data (AppState) with the window
    IDC_ARROW,            // Standard arrow cursor
    MSG,                  // Message structure for the message loop
    SW_SHOW,              // Flag to make the window visible
    WM_COMMAND,           // Message sent when a button is clicked
    WM_CREATE,            // Message sent when the window is first created
    WM_DESTROY,           // Message sent when the window is being destroyed (closed)
    WNDCLASSW,            // Structure that defines our window class
    WS_CHILD,             // Style: this is a child window (for buttons/labels inside our main window)
    WS_OVERLAPPEDWINDOW,  // Style: standard window with title bar, borders, min/max/close buttons
    WS_VISIBLE,           // Style: the control is visible immediately after creation
};

// ============================================================================
// AppState — holds everything our window needs to operate
// ============================================================================
// This struct is heap-allocated (Box<AppState>) and its raw pointer is stored
// with the window via SetWindowLongPtrW. We retrieve it in WM_COMMAND and
// WM_DESTROY to access the volume controller and status label.
struct AppState {
    volume_control: IAudioEndpointVolume,  // COM interface to mute/unmute the mic
    _mute_button: HWND,                    // Handle to the "Mute" button (prefixed with _ because we don't read it after creation)
    _unmute_button: HWND,                  // Handle to the "Unmute" button (same as above)
    status_label: HWND,                    // Handle to the "Status: ..." text label (we update its text)
}

// ============================================================================
// main() — Entry point: sets up COM, gets the mic, creates the window, runs the message loop
// ============================================================================
fn main() -> Result<()> {
    unsafe {
        // Step 1: Initialize COM library for this thread.
        // COM must be initialized before calling any COM functions.
        // We use .ok() to ignore the "already initialized" case.
        CoInitialize(None).ok();

        // Step 2: Create a device enumerator to find audio devices.
        // CoCreateInstance creates a COM object given its class ID (MMDeviceEnumerator).
        // CLSCTX_INPROC_SERVER means "create the object in our process".
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(
            &MMDeviceEnumerator,
            None,
            CLSCTX_INPROC_SERVER,
        )?;

        // Step 3: Get the default microphone device.
        // eCapture = input device (microphone), eConsole = general-purpose role.
        let device = enumerator.GetDefaultAudioEndpoint(eCapture, eConsole)?;

        // Step 4: Activate the IAudioEndpointVolume interface on the mic device.
        // This gives us the ability to get/set mute state and volume level.
        // Note: We use Activate() instead of cast() because IMMDevice requires
        // activation to create the volume control interface.
        let volume_control: IAudioEndpointVolume = device.Activate(CLSCTX_INPROC_SERVER, None)?;

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
    // Must be called once for each successful CoInitialize call.
    unsafe { CoUninitialize() };

    Ok(())
}

// ============================================================================
// Constants — Button IDs used to identify which button was clicked
// ============================================================================
// When Windows sends WM_COMMAND, it includes the control ID in wparam.
// We compare against these to know if "Mute" or "Unmute" was clicked.
const MUTE_BUTTON_ID: isize = 1;
const UNMUTE_BUTTON_ID: isize = 2;

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

        // Create the actual window.
        //   - Default::default(): no extended styles
        //   - class_name: the class we just registered
        //   - title: "Mute/Unmute Microphone" in the title bar
        //   - WS_OVERLAPPEDWINDOW: standard window with title bar + borders + min/max/close
        //   - CW_USEDEFAULT: let Windows choose position
        //   - 300x150: window size in pixels
        //   - None: no parent window, no menu
        //   - instance: our executable handle
        //   - None: no extra creation data
        // Note: The window is NOT shown yet — that happens in WM_CREATE via ShowWindow.
        CreateWindowExW(
            Default::default(),
            class_name,
            w!("Mute/Unmute Microphone"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            300, // width
            150, // height
            None,
            None,
            instance,
            None,
        );

        Ok(())
    }
}

// ============================================================================
// wndproc() — Window procedure: handles all messages sent to our window
// ============================================================================
// Windows calls this function every time something happens to our window
// (created, button clicked, closed, resized, etc.).
// We handle 3 messages and let Windows handle everything else via DefWindowProcW.
unsafe extern "system" fn wndproc(
    hwnd: HWND,       // Handle to the window receiving the message
    msg: u32,         // The message type (WM_CREATE, WM_COMMAND, WM_DESTROY, etc.)
    wparam: WPARAM,   // Additional message data (e.g., which button was clicked)
    lparam: LPARAM,   // Additional message data (e.g., pointer to creation struct)
) -> LRESULT {
    match msg {
        // ----------------------------------------------------------------
        // WM_CREATE — Window is being created. Set up the UI controls.
        // ----------------------------------------------------------------
        WM_CREATE => {
            let instance = GetModuleHandleW(None).unwrap();

            // Create child controls inside our window:
            //   - "BUTTON" class: a clickable button
            //   - "STATIC" class: a text label (non-interactive)
            //   - WS_VISIBLE | WS_CHILD: visible and inside the parent window
            //   - x, y, width, height: position and size relative to parent
            //   - HMENU(ID): the control ID (used to identify it in WM_COMMAND)
            let mute_button = CreateWindowExW(Default::default(), w!("BUTTON"), w!("Mute"), WS_VISIBLE | WS_CHILD, 20, 20, 100, 30, hwnd, HMENU(MUTE_BUTTON_ID), instance, None);
            let unmute_button = CreateWindowExW(Default::default(), w!("BUTTON"), w!("Unmute"), WS_VISIBLE | WS_CHILD, 140, 20, 100, 30, hwnd, HMENU(UNMUTE_BUTTON_ID), instance, None);
            let status_label = CreateWindowExW(Default::default(), w!("STATIC"), w!("Status: Unknown"), WS_VISIBLE | WS_CHILD, 20, 70, 220, 20, hwnd, HMENU(-1isize), instance, None);

            // Retrieve the volume controller from the global static variable.
            // .resolve() creates a thread-local COM proxy from the AgileReference.
            let agile_ref = VOLUME_CONTROL.get().expect("Volume control not initialized");
            let volume_control = agile_ref.resolve().expect("Failed to resolve agile reference");

            // Check the current mute state of the mic and display it in the status label.
            if let Ok(is_muted) = volume_control.GetMute() {
                let status_text = if is_muted.as_bool() { "Status: Muted" } else { "Status: Unmuted" };
                let _ = SetWindowTextW(status_label, &HSTRING::from(status_text));
            }

            // Bundle all our state into a heap-allocated struct.
            // Box::into_raw converts it to a raw pointer so we can store it with the window.
            // We'll retrieve this pointer in WM_COMMAND and WM_DESTROY.
            let app_state = Box::new(AppState {
                volume_control,
                _mute_button: mute_button,
                _unmute_button: unmute_button,
                status_label,
            });
            // Store the raw pointer in the window's GWLP_USERDATA slot.
            // This is the standard Win32 pattern for associating custom data with a window.
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(app_state) as _);

            // Now show the window on screen.
            ShowWindow(hwnd, SW_SHOW);
            LRESULT(0) // Return 0 = success for WM_CREATE
        }

        // ----------------------------------------------------------------
        // WM_COMMAND — A button was clicked. Mute or unmute accordingly.
        // ----------------------------------------------------------------
        WM_COMMAND => {
            // Retrieve our AppState from the window's GWLP_USERDATA slot.
            // We stored a raw pointer there in WM_CREATE.
            let app_state = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const AppState);

            // The low word of wparam contains the control ID of the button that was clicked.
            let command = wparam.0 as u16;
            if command == MUTE_BUTTON_ID as u16 {
                // Mute button clicked: set mic to muted.
                // null() is passed as the event context GUID (not needed for our use case).
                let _ = app_state.volume_control.SetMute(true, null());
                let _ = SetWindowTextW(app_state.status_label, &HSTRING::from("Status: Muted"));
            } else if command == UNMUTE_BUTTON_ID as u16 {
                // Unmute button clicked: set mic to unmuted.
                let _ = app_state.volume_control.SetMute(false, null());
                let _ = SetWindowTextW(app_state.status_label, &HSTRING::from("Status: Unmuted"));
            }
            LRESULT(0) // Return 0 = we handled the message
        }

        // ----------------------------------------------------------------
        // WM_DESTROY — Window is being closed. Clean up and unmute.
        // ----------------------------------------------------------------
        WM_DESTROY => {
            // Retrieve the AppState pointer we stored in WM_CREATE.
            let app_state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
            if !app_state_ptr.is_null() {
                // Unmute the mic before we exit — so the user isn't left muted.
                let app_state = &*app_state_ptr;
                let _ = app_state.volume_control.SetMute(false, null());

                // Convert the raw pointer back into a Box so Rust properly drops/frees it.
                // This prevents a memory leak of the AppState struct.
                drop(Box::from_raw(app_state_ptr));
            }

            // Post WM_QUIT to the message queue, which causes GetMessageW to return false
            // and exit the message loop in main().
            PostQuitMessage(0);
            LRESULT(0) // Return 0 = we handled the message
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