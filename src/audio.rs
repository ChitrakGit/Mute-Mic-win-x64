use windows::core::Result;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitialize, CoUninitialize, CLSCTX_INPROC_SERVER,
};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eConsole, IMMDeviceEnumerator, MMDeviceEnumerator,
};

/// Initializes the Windows COM library for the current thread.
/// This must be called before interacting with AudioEndpoint APIs.
pub fn init_com() {
    unsafe {
        // We use .ok() to ignore errors if it was already initialized.
        CoInitialize(None).ok();
    }
}

/// Cleans up the Windows COM library for the current thread.
pub fn uninit_com() {
    unsafe {
        CoUninitialize();
    }
}

/// Retrieves the IAudioEndpointVolume interface for the default microphone.
pub fn get_default_mic_volume() -> Result<IAudioEndpointVolume> {
    unsafe {
        // Create a device enumerator
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(
            &MMDeviceEnumerator,
            None,
            CLSCTX_INPROC_SERVER,
        )?;

        // Find the default capture device (Microphone)
        let device = enumerator.GetDefaultAudioEndpoint(eCapture, eConsole)?;

        // Activate the volume control interface on the device
        let volume_control: IAudioEndpointVolume = device.Activate(CLSCTX_INPROC_SERVER, None)?;
        
        Ok(volume_control)
    }
}
