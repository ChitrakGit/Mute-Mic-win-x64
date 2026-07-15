use std::ptr::null;
use muteMic::audio::{init_com, uninit_com, get_default_mic_volume};

// Note on testing Windows COM objects:
// Each test in Rust runs in its own thread by default.
// COM must be initialized per-thread before any COM objects can be created.
// Therefore, we must call `init_com()` at the beginning of each test
// and `uninit_com()` at the end.

#[test]
fn test_get_microphone() {
    // 1. Initialize COM for this test thread
    init_com();

    // 2. Attempt to get the microphone volume controller
    let result = get_default_mic_volume();
    
    // 3. Verify it succeeds
    assert!(result.is_ok(), "Failed to get the default microphone volume controller. Is a microphone plugged in and enabled?");

    // 4. Clean up COM
    uninit_com();
}

#[test]
fn test_mute_toggle() {
    init_com();

    let volume_control = get_default_mic_volume().expect("Failed to get mic volume controller");

    unsafe {
        // Read the initial state
        let initial_state = volume_control.GetMute().expect("Failed to read mute state").as_bool();

        // Toggle the state
        let new_state = !initial_state;
        volume_control.SetMute(new_state, null()).expect("Failed to set mute state");

        // Verify the state changed
        let updated_state = volume_control.GetMute().expect("Failed to read updated mute state").as_bool();
        assert_eq!(updated_state, new_state, "Mute state did not update correctly");

        // Revert to initial state to not mess up the user's microphone during testing
        volume_control.SetMute(initial_state, null()).expect("Failed to revert mute state");
    }

    uninit_com();
}
