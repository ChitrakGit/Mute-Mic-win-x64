# muteMic

**muteMic** is a lightweight, background Windows application written in Rust that allows you to easily mute and unmute your default microphone. It sits quietly in your system tray and provides a global hotkey so you can quickly toggle your microphone from anywhere in Windows.

## Features
- **Background Application:** No console window, runs entirely in the background.
- **System Tray Icon:** Right-click to mute, unmute, or exit. Hover over the icon to see your current microphone status.
- **Global Hotkey:** Press `Ctrl + Shift + M` from anywhere in Windows to instantly toggle your microphone mute state.
- **Auto-Unmute:** Automatically unmutes your microphone when the application is closed, ensuring your microphone is never accidentally left disabled.

## Prerequisites & Dependencies

To compile this project, you need the Rust toolchain and the MSVC C++ build tools installed on your Windows machine.

### 1. Install Visual Studio Build Tools
The `windows` Rust crate requires the MSVC toolchain to compile the Windows API bindings.
1. Download the [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).
2. Run the installer.
3. Select the **"Desktop development with C++"** workload.
4. Ensure the **Windows 10 (or 11) SDK** and **MSVC v143 - VS 2022 C++ x64/x86 build tools** are checked on the right-hand panel.
5. Click **Install**.

### 2. Install Rust
If you haven't already, install the Rust compiler and `cargo` package manager.
1. Go to [rustup.rs](https://rustup.rs/).
2. Download and run `rustup-init.exe`.
3. Follow the on-screen prompts (the default installation options are fine).

## Building from Source

1. Clone or download this repository.
2. Open a terminal (PowerShell or Command Prompt) and navigate to the project directory:
   ```powershell
   cd path\to\muteMic
   ```
3. Build the application in release mode for optimal performance and to hide the console window:
   ```powershell
   cargo build --release
   ```
4. The compiled executable will be located at `target\release\muteMic.exe`.

## How to Use

1. Double-click the `target\release\muteMic.exe` file to start the application.
2. An icon will appear in your system tray (near the clock on your taskbar).
3. **Tray Menu:** Right-click the icon to manually select `Mute`, `Unmute`, or `Exit`.
4. **Hotkey:** Press `Ctrl + Shift + M` on your keyboard at any time to toggle the mute state.
5. **Hover:** Hover your mouse over the tray icon to verify if your microphone is currently muted or unmuted.

## Built With
- [Rust](https://www.rust-lang.org/)
- [windows-rs](https://github.com/microsoft/windows-rs) - Official Microsoft Windows API bindings for Rust.
