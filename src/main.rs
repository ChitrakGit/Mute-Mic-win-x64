use std::ptr::null;
use std::sync::OnceLock;

use windows::core::{w, HSTRING};
use windows::core::{AgileReference, Result};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::HMENU;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitialize, CLSCTX_INPROC_SERVER, CoUninitialize,
};
use windows::Win32::System::Console::{SetConsoleCtrlHandler, CTRL_C_EVENT, CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::Media::Audio::{eCapture, eConsole, IMMDeviceEnumerator, MMDeviceEnumerator};

use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
    LoadCursorW, PostQuitMessage, RegisterClassW, SetWindowLongPtrW, SetWindowTextW, ShowWindow,
    TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA, IDC_ARROW, MSG,
    SW_SHOW, WM_COMMAND, WM_CREATE, WM_DESTROY, WNDCLASSW, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

struct AppState {
    volume_control: IAudioEndpointVolume,
    _mute_button: HWND,
    _unmute_button: HWND,
    status_label: HWND,
}

fn main() -> Result<()> {
    unsafe {
        // Initialize COM
        CoInitialize(None).ok();

        // Get the audio endpoint volume controller
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(
            &MMDeviceEnumerator,
            None,
            CLSCTX_INPROC_SERVER,
        )?;
        let device = enumerator.GetDefaultAudioEndpoint(eCapture, eConsole)?;
        let volume_control: IAudioEndpointVolume = device.Activate(CLSCTX_INPROC_SERVER, None)?;

        // Store the volume controller in a static variable to access it in the window procedure
        // and when the application closes.
        let _ = VOLUME_CONTROL.set(AgileReference::new(&volume_control)?);

        // Register a console control handler to unmute on Ctrl+C, console close, etc.
        let _ = SetConsoleCtrlHandler(Some(console_ctrl_handler), true);

        // Create and run the GUI
        create_window()?;

        // Message loop
        let mut message = MSG::default();
        while GetMessageW(&mut message, HWND(0), 0, 0).into() {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        // Safety net: unmute when the message loop exits
        unmute_mic();
    }

    // Uninitialize COM
    unsafe { CoUninitialize() };

    Ok(())
}

const MUTE_BUTTON_ID: isize = 1;
const UNMUTE_BUTTON_ID: isize = 2;

// Use a static OnceLock to safely store the volume controller COM interface.
// AgileReference allows the COM interface to be used across different threads.
static VOLUME_CONTROL: OnceLock<AgileReference<IAudioEndpointVolume>> = OnceLock::new();

fn create_window() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class_name = w!("MuteMicWindowClass");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        if atom == 0 {
            return Err(windows::core::Error::from_win32());
        }

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

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let instance = GetModuleHandleW(None).unwrap();

            // Create buttons and static text label
            let mute_button = CreateWindowExW(Default::default(), w!("BUTTON"), w!("Mute"), WS_VISIBLE | WS_CHILD, 20, 20, 100, 30, hwnd, HMENU(MUTE_BUTTON_ID), instance, None);
            let unmute_button = CreateWindowExW(Default::default(), w!("BUTTON"), w!("Unmute"), WS_VISIBLE | WS_CHILD, 140, 20, 100, 30, hwnd, HMENU(UNMUTE_BUTTON_ID), instance, None);
            let status_label = CreateWindowExW(Default::default(), w!("STATIC"), w!("Status: Unknown"), WS_VISIBLE | WS_CHILD, 20, 70, 220, 20, hwnd, HMENU(-1isize), instance, None);

            // Get the volume controller from the static variable
            let agile_ref = VOLUME_CONTROL.get().expect("Volume control not initialized");
            let volume_control = agile_ref.resolve().expect("Failed to resolve agile reference");

            // Get initial mute state and update the label
            if let Ok(is_muted) = volume_control.GetMute() {
                let status_text = if is_muted.as_bool() { "Status: Muted" } else { "Status: Unmuted" };
                // We can ignore the result of SetWindowTextW in this context.
                let _ = SetWindowTextW(status_label, &HSTRING::from(status_text));
            }

            // Create a state struct and store it with the window for later use
            let app_state = Box::new(AppState {
                volume_control,
                _mute_button: mute_button,
                _unmute_button: unmute_button,
                status_label,
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(app_state) as _);

            ShowWindow(hwnd, SW_SHOW);
            LRESULT(0)
        }
        WM_COMMAND => {
            let app_state = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const AppState);

            let command = wparam.0 as u16;
            if command == MUTE_BUTTON_ID as u16 {
                let _ = app_state.volume_control.SetMute(true, null());
                let _ = SetWindowTextW(app_state.status_label, &HSTRING::from("Status: Muted"));
            } else if command == UNMUTE_BUTTON_ID as u16 {
                let _ = app_state.volume_control.SetMute(false, null());
                let _ = SetWindowTextW(app_state.status_label, &HSTRING::from("Status: Unmuted"));
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // Unmute on exit
            // We retrieve the AppState here to ensure we are using the resolved COM pointer
            // for the current thread before we clean it up.
            let app_state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
            if !app_state_ptr.is_null() {
                // First, get a reference to the state to use it.
                let app_state = &*app_state_ptr;
                let _ = app_state.volume_control.SetMute(false, null());
                // Now, we can safely drop it.
                drop(Box::from_raw(app_state_ptr));
            }

            // Clean up state

            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Unmute the microphone using the global VOLUME_CONTROL.
/// Called from WM_DESTROY, the console control handler, and as a safety net on exit.
fn unmute_mic() {
    if let Some(agile_ref) = VOLUME_CONTROL.get() {
        if let Ok(volume_control) = agile_ref.resolve() {
            unsafe {
                let _ = volume_control.SetMute(false, null());
            }
        }
    }
}

/// Console control handler — unmutes the mic on Ctrl+C, console close, logoff, shutdown.
unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
    match ctrl_type {
        x if x == CTRL_C_EVENT
            || x == CTRL_CLOSE_EVENT
            || x == CTRL_LOGOFF_EVENT
            || x == CTRL_SHUTDOWN_EVENT =>
        {
            unmute_mic();
            BOOL(1) // handled
        }
        _ => BOOL(0), // not handled
    }
}