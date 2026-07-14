use std::ptr;
// Use `_getch` from the MSVCRT to read single key presses on Windows.
unsafe extern "C" {
    fn _getch() -> i32;
}

use windows::core::Result;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitialize, CLSCTX_ALL, CLSCTX_INPROC_SERVER,
};
use windows::Win32::System::Console::{
    SetConsoleCtrlHandler, CTRL_C_EVENT, CTRL_CLOSE_EVENT,
};
use windows::Win32::Media::Audio::{
    eCapture, eConsole, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;

// This callback function is triggered by Windows when the console is closing
// This callback function is triggered by Windows when the console is closing
unsafe extern "system" fn console_handler(ctrl_type: u32) -> BOOL {
    // Check if the event is the 'X' button or Ctrl+C
    if ctrl_type == CTRL_CLOSE_EVENT || ctrl_type == CTRL_C_EVENT {
        
        // Explicitly open an unsafe block for the Windows API calls
        unsafe {
            // Because Windows runs this handler on a new thread, we must initialize COM here too
            CoInitialize(None).ok();

            // Quickly re-fetch the microphone and force it to unmute
            if let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(
                &MMDeviceEnumerator,
                None,
                CLSCTX_INPROC_SERVER,
            ) {
                if let Ok(device) = enumerator.GetDefaultAudioEndpoint(eCapture, eConsole) {
                    if let Ok(volume) = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None as Option<*const _>) {
                        
                        // Unmute the mic!
                        let _ = volume.SetMute(false, ptr::null());
                    }
                }
            }
        }
        
        // Returning false tells Windows we are done and it can proceed to close the app
        return false.into();
    }
    
    // Ignore other console events
    false.into()
}

fn main() -> Result<()> {
    unsafe {
        // 0. Register our custom handler to catch the console closing
        SetConsoleCtrlHandler(Some(console_handler), true)?;

        // 1. Initialize the COM library on the main thread
        CoInitialize(None).ok();

        // 2. Create the COM instance for the Device Enumerator
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(
            &MMDeviceEnumerator,
            None,
            CLSCTX_INPROC_SERVER,
        )?;

        // 3. Get the default audio capture device (microphone)
        let device = enumerator.GetDefaultAudioEndpoint(eCapture, eConsole)?;

        // 4. Activate the volume control interface for the device
        let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None as Option<*const _>)?;

        println!("Controls: press keys - m:mute, u:unmute, t:toggle, q:quit");

        loop {
            // Safe wrapper around C `_getch` which returns an int representing the key.
            println!("Press a key: ");
            let ch = _getch();
            if ch <= 0 {
                continue;
            }
            
            // map ASCII letters (support upper and lower)
            let c = (ch as u8) as char;
            println!("Input: {}", c);
            
            match c {
                'm' | 'M' => {
                    volume.SetMute(true, ptr::null())?;
                    println!("Microphone muted");
                }
                'u' | 'U' => {
                    volume.SetMute(false, ptr::null())?;
                    println!("Microphone unmuted");
                }
                't' | 'T' => {
                    let current = volume.GetMute()?;
                    let new = !current;
                    volume.SetMute(new, ptr::null())?;
                    let status = if new.into() { "Muted" } else { "Unmuted" };
                    println!("Microphone {}", status);
                }
                'q' | 'Q' => {
                    // Gracefully unmute before quitting via 'q'
                    volume.SetMute(false, ptr::null())?;
                    println!("Unmuting and Exiting");
                    break;
                }
                _ => {}
            }
        }
    }
    
    Ok(())
}