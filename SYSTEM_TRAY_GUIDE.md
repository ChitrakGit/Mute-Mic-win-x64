# Converting muteMic to a System Tray Application

## Problem
Currently, running `muteMic.exe` opens a console window (terminal) alongside the GUI.
We want to make it a real Windows application that:
- **No console window** appears
- Runs in the **system tray** (notification area near the clock)
- User can **right-click the tray icon** to Mute/Unmute/Exit
- Shows current mic status via **tooltip** on hover

---

## Steps to Implement

### Step 1: Hide the Console Window
Add this attribute at the very top of `main.rs` (before any `use` statements):

```rust
#![windows_subsystem = "windows"]
```

**What it does**: Tells the Windows linker to build a GUI application instead of a console
application. This is the single line that eliminates the terminal window.

---

### Step 2: Add Required Cargo Features
Add these features to `Cargo.toml` under `[dependencies.windows]`:

```toml
"Win32_UI_Shell",              # For Shell_NotifyIconW (system tray)
"Win32_UI_WindowsAndMessaging" # Already present — also has menu functions
```

---

### Step 3: Replace the Visible Window with a Hidden Message-Only Window
Instead of `WS_OVERLAPPEDWINDOW` (visible window), create a **hidden window** that only
receives messages. The system tray icon becomes the user's only way to interact.

```rust
// Change: No WS_OVERLAPPEDWINDOW, no ShowWindow
// The window exists only to receive tray icon messages
CreateWindowExW(
    Default::default(),
    class_name,
    w!("MuteMicHidden"),
    Default::default(), // No visible style
    0, 0, 0, 0,         // No size needed
    None, None, instance, None,
);
```

---

### Step 4: Add a System Tray Icon
After creating the hidden window, add a notification icon using `Shell_NotifyIconW`:

```rust
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW,
    NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NIF_MESSAGE, NIF_ICON, NIF_TIP,
};

// Custom message ID for tray icon events
const WM_TRAYICON: u32 = WM_USER + 1;

// Set up the NOTIFYICONDATAW struct
let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
nid.hWnd = hwnd;                      // Our hidden window
nid.uID = 1;                          // Unique icon ID
nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
nid.uCallbackMessage = WM_TRAYICON;   // Message sent on icon events
nid.hIcon = LoadIconW(None, IDI_APPLICATION)?;  // Default app icon

// Set tooltip text ("Mic: Unmuted")
let tip = "MuteMic - Unmuted";
for (i, c) in tip.encode_utf16().enumerate() {
    if i >= nid.szTip.len() - 1 { break; }
    nid.szTip[i] = c;
}

Shell_NotifyIconW(NIM_ADD, &nid);
```

---

### Step 5: Handle Tray Icon Messages in wndproc
When the user clicks the tray icon, Windows sends our custom `WM_TRAYICON` message.
We check if it was a right-click and show a popup menu:

```rust
WM_TRAYICON => {
    let event = (lparam.0 & 0xFFFF) as u32;
    if event == WM_RBUTTONUP {
        // Show the context menu
        show_tray_menu(hwnd);
    }
    LRESULT(0)
}
```

---

### Step 6: Create a Right-Click Context Menu
Build a popup menu with Mute, Unmute, and Exit options:

```rust
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, AppendMenuW, TrackPopupMenu,
    SetForegroundWindow, GetCursorPos,
    MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
};

const ID_MUTE: u16 = 1001;
const ID_UNMUTE: u16 = 1002;
const ID_EXIT: u16 = 1003;

fn show_tray_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu().unwrap();
        AppendMenuW(menu, MF_STRING, ID_MUTE as usize, w!("Mute"));
        AppendMenuW(menu, MF_STRING, ID_UNMUTE as usize, w!("Unmute"));
        AppendMenuW(menu, MF_STRING, ID_EXIT as usize, w!("Exit"));

        let mut pt = POINT::default();
        GetCursorPos(&mut pt);

        // Required: Makes the menu close when clicking outside
        SetForegroundWindow(hwnd);
        TrackPopupMenu(menu, TPM_BOTTOMALIGN | TPM_LEFTALIGN,
                       pt.x, pt.y, 0, hwnd, None);
    }
}
```

---

### Step 7: Handle Menu Clicks in WM_COMMAND
When the user clicks a menu item, Windows sends WM_COMMAND with the item ID:

```rust
WM_COMMAND => {
    let command = wparam.0 as u16;
    match command {
        ID_MUTE => {
            // Mute the mic, update tooltip to "Muted"
        }
        ID_UNMUTE => {
            // Unmute the mic, update tooltip to "Unmuted"
        }
        ID_EXIT => {
            // Unmute, remove tray icon, post quit
            Shell_NotifyIconW(NIM_DELETE, &nid);
            unmute_mic();
            PostQuitMessage(0);
        }
        _ => {}
    }
    LRESULT(0)
}
```

---

### Step 8: Remove the Tray Icon on Exit
In `WM_DESTROY`, remove the icon so it doesn't leave a ghost in the tray:

```rust
WM_DESTROY => {
    unmute_mic();
    Shell_NotifyIconW(NIM_DELETE, &nid);  // Remove tray icon
    PostQuitMessage(0);
    LRESULT(0)
}
```

---

### Step 9: Update the Tooltip When Mute State Changes
After muting/unmuting, update the tray icon tooltip so the user sees the current status on hover:

```rust
// Update tooltip
let tip = "MuteMic - Muted";
for (i, c) in tip.encode_utf16().enumerate() {
    nid.szTip[i] = c;
}
nid.szTip[tip.encode_utf16().count()] = 0; // null terminate
nid.uFlags = NIF_TIP;
Shell_NotifyIconW(NIM_MODIFY, &nid);
```

---

### Step 10: Build the Release Version
```powershell
cargo build --release
```

The resulting `target/release/muteMic.exe` will:
- Have **no console window**
- Show a **tray icon** near the clock
- Let users **right-click → Mute / Unmute / Exit**
- **Auto-unmute** on exit

---

## Summary of Changes

| File | Change |
|------|--------|
| `main.rs` | Add `#![windows_subsystem = "windows"]` at top |
| `main.rs` | Replace visible window with hidden message window |
| `main.rs` | Add tray icon setup with `Shell_NotifyIconW` |
| `main.rs` | Add `WM_TRAYICON` handler for right-click menu |
| `main.rs` | Add popup menu (Mute/Unmute/Exit) |
| `main.rs` | Remove tray icon in `WM_DESTROY` |
| `main.rs` | Remove old buttons/label UI code |
| `Cargo.toml` | Add `"Win32_UI_Shell"` feature |

---

## Optional Enhancements (Later)
- **Custom icon**: Use a microphone icon (`.ico` file) instead of the default app icon
- **Different icons for muted/unmuted**: Swap the tray icon when state changes
- **Keyboard shortcut**: Add a global hotkey (e.g., `Ctrl+Shift+M`) to toggle mute
- **Startup with Windows**: Add a registry key to auto-launch on login
