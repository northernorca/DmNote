use std::io::Write;

use anyhow::{anyhow, Result};
use serde_json::to_string;

use crate::ipc::{DaemonCommand, HookKeyState, HookMessage, InputDeviceKind};
use crate::models::{ShortcutBinding, ShortcutsState};

fn load_hotkeys_from_env() -> ShortcutsState {
    std::env::var("DMNOTE_HOTKEYS_V1")
        .ok()
        .and_then(|value| serde_json::from_str::<ShortcutsState>(&value).ok())
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
use crate::{
    ipc::pipe_client_connect,
    keyboard_labels::{
        build_key_labels, should_skip_keyboard_event, IsKeyboardEventInjected, KeyboardEvent,
        KeyboardKey, KeyPress,
    },
};

/// Global hotkey state tracker
#[cfg(target_os = "windows")]
struct HotkeyState {
    ctrl_left: bool,
    ctrl_right: bool,
    shift_left: bool,
    shift_right: bool,
    alt_left: bool,
    alt_right: bool,
    meta_left: bool,
    meta_right: bool,
    toggle_overlay: Option<ParsedHotkey>,
    toggle_overlay_lock: Option<ParsedHotkey>,
    toggle_always_on_top: Option<ParsedHotkey>,
}

#[cfg(target_os = "windows")]
impl HotkeyState {
    fn new(toggle_overlay: ShortcutBinding, toggle_overlay_lock: ShortcutBinding, toggle_always_on_top: ShortcutBinding) -> Self {
        Self {
            ctrl_left: false,
            ctrl_right: false,
            shift_left: false,
            shift_right: false,
            alt_left: false,
            alt_right: false,
            meta_left: false,
            meta_right: false,
            toggle_overlay: ParsedHotkey::from_binding(&toggle_overlay),
            toggle_overlay_lock: ParsedHotkey::from_binding(&toggle_overlay_lock),
            toggle_always_on_top: ParsedHotkey::from_binding(&toggle_always_on_top),
        }
    }

    /// Update modifier state and check for hotkey triggers
    /// Returns Some(command) if a hotkey was triggered
    fn update(&mut self, vk_code: u32, is_down: bool) -> Option<DaemonCommand> {
        // VK codes for modifiers
        const VK_LCONTROL: u32 = 0xA2;
        const VK_RCONTROL: u32 = 0xA3;
        const VK_LSHIFT: u32 = 0xA0;
        const VK_RSHIFT: u32 = 0xA1;
        const VK_LMENU: u32 = 0xA4; // Left Alt
        const VK_RMENU: u32 = 0xA5; // Right Alt
        const VK_LWIN: u32 = 0x5B;
        const VK_RWIN: u32 = 0x5C;

        match vk_code {
            VK_LCONTROL => self.ctrl_left = is_down,
            VK_RCONTROL => self.ctrl_right = is_down,
            VK_LSHIFT => self.shift_left = is_down,
            VK_RSHIFT => self.shift_right = is_down,
            VK_LMENU => self.alt_left = is_down,
            VK_RMENU => self.alt_right = is_down,
            VK_LWIN => self.meta_left = is_down,
            VK_RWIN => self.meta_right = is_down,
            _ => {}
        }

        if !is_down {
            return None;
        }

        let ctrl = self.ctrl_left || self.ctrl_right;
        let shift = self.shift_left || self.shift_right;
        let alt = self.alt_left || self.alt_right;
        let meta = self.meta_left || self.meta_right;

        let matches = |hotkey: &ParsedHotkey| {
            vk_code == hotkey.key_vk
                && ctrl == hotkey.ctrl
                && shift == hotkey.shift
                && alt == hotkey.alt
                && meta == hotkey.meta
        };

        if let Some(hk) = self.toggle_overlay.as_ref() {
            if matches(hk) {
                return Some(DaemonCommand::ToggleOverlay);
            }
        }
        if let Some(hk) = self.toggle_overlay_lock.as_ref() {
            if matches(hk) {
                return Some(DaemonCommand::ToggleOverlayLock);
            }
        }
        if let Some(hk) = self.toggle_always_on_top.as_ref() {
            if matches(hk) {
                return Some(DaemonCommand::ToggleAlwaysOnTop);
            }
        }

        None
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct ParsedHotkey {
    key_vk: u32,
    ctrl: bool,
    shift: bool,
    alt: bool,
    meta: bool,
}

#[cfg(target_os = "windows")]
impl ParsedHotkey {
    fn from_binding(binding: &ShortcutBinding) -> Option<Self> {
        let key_vk = vk_from_key_code(&binding.key)?;
        Some(Self {
            key_vk,
            ctrl: binding.ctrl,
            shift: binding.shift,
            alt: binding.alt,
            meta: binding.meta,
        })
    }
}

#[cfg(target_os = "windows")]
fn vk_from_key_code(code: &str) -> Option<u32> {
    // Uses KeyboardEvent.code-style keys (e.g., KeyO, Tab, Digit1).
    if let Some(rest) = code.strip_prefix("Key") {
        if rest.len() == 1 {
            let ch = rest.chars().next()?.to_ascii_uppercase();
            if ('A'..='Z').contains(&ch) {
                return Some(ch as u32);
            }
        }
    }

    if let Some(rest) = code.strip_prefix("Digit") {
        if rest.len() == 1 {
            let ch = rest.chars().next()?;
            if ('0'..='9').contains(&ch) {
                return Some(ch as u32);
            }
        }
    }

    if let Some(rest) = code.strip_prefix("F") {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            let n: u32 = rest.parse().ok()?;
            if (1..=24).contains(&n) {
                return Some(0x6F + n);
            }
        }
    }

    match code {
        "Tab" => Some(0x09),
        "Enter" => Some(0x0D),
        "Escape" => Some(0x1B),
        "Space" => Some(0x20),
        "Backspace" => Some(0x08),
        "Insert" => Some(0x2D),
        "Delete" => Some(0x2E),
        "Home" => Some(0x24),
        "End" => Some(0x23),
        "PageUp" => Some(0x21),
        "PageDown" => Some(0x22),
        "ArrowLeft" => Some(0x25),
        "ArrowUp" => Some(0x26),
        "ArrowRight" => Some(0x27),
        "ArrowDown" => Some(0x28),
        "Comma" => Some(0xBC),
        "Period" => Some(0xBE),
        "Slash" => Some(0xBF),
        "Semicolon" => Some(0xBA),
        "Quote" => Some(0xDE),
        "BracketLeft" => Some(0xDB),
        "BracketRight" => Some(0xDD),
        "Backslash" => Some(0xDC),
        "Backquote" => Some(0xC0),
        "Minus" => Some(0xBD),
        "Equal" => Some(0xBB),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
struct MacHotkeyState {
    ctrl_left: bool,
    ctrl_right: bool,
    shift_left: bool,
    shift_right: bool,
    alt_left: bool,
    alt_right: bool,
    meta_left: bool,
    meta_right: bool,
    toggle_overlay_key: String,
    toggle_overlay: ShortcutBinding,
    toggle_overlay_lock_key: String,
    toggle_overlay_lock: ShortcutBinding,
    toggle_always_on_top_key: String,
    toggle_always_on_top: ShortcutBinding,
}

#[cfg(target_os = "macos")]
impl MacHotkeyState {
    fn new(toggle_overlay: ShortcutBinding, toggle_overlay_lock: ShortcutBinding, toggle_always_on_top: ShortcutBinding) -> Self {
        let toggle_overlay_key = toggle_overlay.key.to_ascii_lowercase();
        let toggle_overlay_lock_key = toggle_overlay_lock.key.to_ascii_lowercase();
        let toggle_always_on_top_key = toggle_always_on_top.key.to_ascii_lowercase();
        Self {
            ctrl_left: false,
            ctrl_right: false,
            shift_left: false,
            shift_right: false,
            alt_left: false,
            alt_right: false,
            meta_left: false,
            meta_right: false,
            toggle_overlay_key,
            toggle_overlay,
            toggle_overlay_lock_key,
            toggle_overlay_lock,
            toggle_always_on_top_key,
            toggle_always_on_top,
        }
    }

    fn update(&mut self, key_name: &str, is_down: bool) -> Option<DaemonCommand> {
        match key_name {
            "controlleft" | "controlright" => {
                if key_name == "controlleft" {
                    self.ctrl_left = is_down;
                } else {
                    self.ctrl_right = is_down;
                }
            }
            "shiftleft" | "shiftright" => {
                if key_name == "shiftleft" {
                    self.shift_left = is_down;
                } else {
                    self.shift_right = is_down;
                }
            }
            "alt" | "altleft" | "altright" | "option" => {
                if key_name == "altright" {
                    self.alt_right = is_down;
                } else {
                    self.alt_left = is_down;
                }
            }
            "metaleft" | "metaright" | "command" => {
                if key_name == "metaright" {
                    self.meta_right = is_down;
                } else {
                    self.meta_left = is_down;
                }
            }
            _ => {}
        }

        if !is_down {
            return None;
        }

        let ctrl = self.ctrl_left || self.ctrl_right;
        let shift = self.shift_left || self.shift_right;
        let alt = self.alt_left || self.alt_right;
        let meta = self.meta_left || self.meta_right;

        let matches = |key: &str, binding: &ShortcutBinding| {
            !key.trim().is_empty()
                && key_name == key
                && ctrl == binding.ctrl
                && shift == binding.shift
                && alt == binding.alt
                && meta == binding.meta
        };

        if matches(&self.toggle_overlay_key, &self.toggle_overlay) {
            return Some(DaemonCommand::ToggleOverlay);
        }
        if matches(&self.toggle_overlay_lock_key, &self.toggle_overlay_lock) {
            return Some(DaemonCommand::ToggleOverlayLock);
        }
        if matches(&self.toggle_always_on_top_key, &self.toggle_always_on_top) {
            return Some(DaemonCommand::ToggleAlwaysOnTop);
        }

        None
    }
}

#[cfg(target_os = "macos")]
fn labels_from_name_hint(name: &str) -> Option<Vec<String>> {
    if name.chars().count() != 1 {
        return None;
    }
    let ch = name.chars().next()?;
    let label = match ch {
        'a'..='z' => ch.to_ascii_uppercase().to_string(),
        'A'..='Z' => ch.to_string(),
        '0'..='9' => ch.to_string(),
        ' ' => "SPACE".to_string(),
        ',' => "COMMA".to_string(),
        '.' => "DOT".to_string(),
        '/' => "FORWARD SLASH".to_string(),
        '-' => "MINUS".to_string(),
        '=' => "EQUALS".to_string(),
        '[' => "SQUARE BRACKET OPEN".to_string(),
        ']' => "SQUARE BRACKET CLOSE".to_string(),
        ';' => "SEMICOLON".to_string(),
        '\'' => "QUOTE".to_string(),
        '`' => "SECTION".to_string(),
        '\\' => "BACKSLASH".to_string(),
        _ => return None,
    };
    Some(vec![label])
}

#[cfg(target_os = "macos")]
fn labels_from_key_name(name: &str) -> Vec<String> {
    let name_lower = name.to_ascii_lowercase();
    let mut labels = match name_lower.as_str() {
        "shiftleft" => vec!["LEFT SHIFT".to_string()],
        "shiftright" => vec!["RIGHT SHIFT".to_string()],
        "controlleft" => vec!["LEFT CTRL".to_string()],
        "controlright" => vec!["25".to_string(), "RIGHT CTRL".to_string()],
        "alt" | "altleft" => vec!["LEFT ALT".to_string()],
        "altgr" | "altright" => vec!["21".to_string(), "RIGHT ALT".to_string()],
        "metaleft" => vec!["91".to_string()],
        "metaright" => vec!["92".to_string()],
        "space" => vec!["SPACE".to_string()],
        "return" | "enter" => vec!["RETURN".to_string()],
        "tab" => vec!["TAB".to_string()],
        "backspace" | "back_space" => vec!["BACKSPACE".to_string()],
        "capslock" | "caps_lock" => vec!["CAPS LOCK".to_string()],
        "escape" => vec!["ESCAPE".to_string()],
        "uparrow" | "arrowup" => vec!["UP ARROW".to_string()],
        "downarrow" | "arrowdown" => vec!["DOWN ARROW".to_string()],
        "leftarrow" | "arrowleft" => vec!["LEFT ARROW".to_string()],
        "rightarrow" | "arrowright" => vec!["RIGHT ARROW".to_string()],
        "home" => vec!["HOME".to_string()],
        "end" => vec!["END".to_string()],
        "pageup" | "page_up" => vec!["PAGE UP".to_string()],
        "pagedown" | "page_down" => vec!["PAGE DOWN".to_string()],
        "insert" => vec!["INS".to_string()],
        "delete" => vec!["DELETE".to_string()],
        "printscreen" | "print_screen" => vec!["PRINT SCREEN".to_string()],
        "scrolllock" | "scroll_lock" => vec!["SCROLL LOCK".to_string()],
        "pause" => vec!["19".to_string()],
        "contextmenu" | "context_menu" => vec!["CONTEXT MENU".to_string()],
        "fn" => vec!["FN".to_string()],
        "numlock" | "num_lock" => vec!["NUM LOCK".to_string()],
        "minus" => vec!["MINUS".to_string()],
        "equal" | "equals" => vec!["EQUALS".to_string()],
        "bracketleft" | "leftbracket" | "bracket_left" => {
            vec!["SQUARE BRACKET OPEN".to_string()]
        }
        "bracketright" | "rightbracket" | "bracket_right" => {
            vec!["SQUARE BRACKET CLOSE".to_string()]
        }
        "semicolon" => vec!["SEMICOLON".to_string()],
        "quote" | "apostrophe" => vec!["QUOTE".to_string()],
        "backquote" | "back_quote" | "grave" => vec!["SECTION".to_string()],
        "backslash" | "back_slash" => vec!["BACKSLASH".to_string()],
        "comma" => vec!["COMMA".to_string()],
        "dot" | "period" => vec!["DOT".to_string()],
        "slash" => vec!["FORWARD SLASH".to_string()],
        _ => Vec::new(),
    };

    if !labels.is_empty() {
        return labels;
    }

    if let Some(rest) = name_lower.strip_prefix("key") {
        if rest.len() == 1 && rest.chars().all(|c| c.is_ascii_alphabetic()) {
            labels.push(rest.to_ascii_uppercase());
            return labels;
        }
    }

    if let Some(rest) = name_lower.strip_prefix("num") {
        if rest.len() == 1 && rest.chars().all(|c| c.is_ascii_digit()) {
            labels.push(rest.to_string());
            return labels;
        }
    }

    if let Some(rest) = name_lower.strip_prefix("f") {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            labels.push(format!("F{}", rest));
            return labels;
        }
    }

    let numpad_prefixes = ["numpad", "num_pad"];
    for prefix in numpad_prefixes {
        if let Some(rest) = name_lower.strip_prefix(prefix) {
            if rest.len() == 1 && rest.chars().all(|c| c.is_ascii_digit()) {
                labels.push(format!("NUMPAD {}", rest));
                return labels;
            }
            let numpad_label = match rest {
                "add" => Some("NUMPAD PLUS"),
                "subtract" => Some("NUMPAD MINUS"),
                "multiply" => Some("NUMPAD MULTIPLY"),
                "divide" => Some("NUMPAD DIVIDE"),
                "decimal" => Some("NUMPAD DELETE"),
                "enter" | "return" => Some("NUMPAD RETURN"),
                _ => None,
            };
            if let Some(value) = numpad_label {
                labels.push(value.to_string());
                return labels;
            }
        }
    }

    labels
}

#[cfg(target_os = "macos")]
fn mac_key_labels(key: rdev::Key, name_hint: Option<&str>) -> Vec<String> {
    if let Some(name) = name_hint {
        if let Some(labels) = labels_from_name_hint(name) {
            return labels;
        }
    }
    let key_name = format!("{:?}", key);
    labels_from_key_name(&key_name)
}

#[cfg(target_os = "macos")]
fn mac_mouse_label(button: rdev::Button) -> Option<String> {
    use rdev::Button::*;
    match button {
        Left => Some("MOUSE1".to_string()),
        Right => Some("MOUSE2".to_string()),
        Middle => Some("MOUSE3".to_string()),
        Unknown(4) => Some("MOUSE4".to_string()),
        Unknown(5) => Some("MOUSE5".to_string()),
        _ => None,
    }
}

fn write_message(
    sink: &mut Box<dyn Write + Send>,
    message: &HookMessage,
) -> Result<()> {
    let line = to_string(message)?;
    sink.write_all(line.as_bytes())?;
    sink.write_all(b"\n")?;
    Ok(())
}

fn write_command(
    sink: &mut Box<dyn Write + Send>,
    command: &DaemonCommand,
) -> Result<()> {
    let line = to_string(command)?;
    sink.write_all(line.as_bytes())?;
    sink.write_all(b"\n")?;
    Ok(())
}

pub fn run() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        return run_raw_input();
    }

    #[cfg(target_os = "macos")]
    {
        return run_macos();
    }

    #[cfg(target_os = "linux")]
    {
        return run_linux();
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os="linux")))]
    {
        Err(anyhow!("Raw input backend is only available on Windows and macOS"))
    }
}

#[cfg(target_os = "windows")]
fn run_raw_input() -> Result<()> {
    use std::ffi::c_void;
    use std::mem::size_of;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Input::{
        GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
        RAWINPUTHEADER, RIDEV_INPUTSINK, RIDEV_NOLEGACY, RID_INPUT, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassExW,
        TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, MSG, WNDCLASSEXW, WM_DESTROY,
        WM_INPUT, WM_QUIT, WS_OVERLAPPEDWINDOW, PostQuitMessage, RI_KEY_BREAK, RI_KEY_E0,
    };

    // Try to connect to named pipe; fall back to stdout if unavailable
    let mut sink: Box<dyn Write + Send> = match pipe_client_connect("dmnote_keys_v1") {
        Ok(file) => Box::new(file),
        Err(_) => Box::new(std::io::stdout()),
    };

    // Global hotkey state tracker
    let hotkeys = load_hotkeys_from_env();
    let mut hotkey_state = HotkeyState::new(
        hotkeys.toggle_overlay,
        hotkeys.toggle_overlay_lock,
        hotkeys.toggle_always_on_top,
    );

    // Raw Input mouse button flags (not exposed as constants in windows crate today).
    const RI_MOUSE_LEFT_BUTTON_DOWN: u16 = 0x0001;
    const RI_MOUSE_LEFT_BUTTON_UP: u16 = 0x0002;
    const RI_MOUSE_RIGHT_BUTTON_DOWN: u16 = 0x0004;
    const RI_MOUSE_RIGHT_BUTTON_UP: u16 = 0x0008;
    const RI_MOUSE_MIDDLE_BUTTON_DOWN: u16 = 0x0010;
    const RI_MOUSE_MIDDLE_BUTTON_UP: u16 = 0x0020;
    const RI_MOUSE_BUTTON_4_DOWN: u16 = 0x0040;
    const RI_MOUSE_BUTTON_4_UP: u16 = 0x0080;
    const RI_MOUSE_BUTTON_5_DOWN: u16 = 0x0100;
    const RI_MOUSE_BUTTON_5_UP: u16 = 0x0200;
    // Wheel constants kept for completeness but not used (wheel events disabled).
    const _RI_MOUSE_WHEEL: u16 = 0x0400;
    const _RI_MOUSE_HWHEEL: u16 = 0x0800;

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_DESTROY => {
                // Signal message loop to quit; keyboard daemon process should exit shortly after.
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    unsafe {
        // Register a minimal window class for receiving WM_INPUT.
        let class_name: Vec<u16> = "DmNoteRawInput".encode_utf16().chain(std::iter::once(0)).collect();
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        let hinstance = GetModuleHandleW(None)?;

        let wnd_class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        if RegisterClassExW(&wnd_class) == 0 {
            return Err(anyhow!("RegisterClassExW failed: {:?}", GetLastError()));
        }

        let hwnd = CreateWindowExW(
            Default::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(class_name.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;

        // Register for Raw Input keyboard + mouse events, even when not in focus.
        let devices = [
            RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x06, // Keyboard
                dwFlags: RIDEV_INPUTSINK | RIDEV_NOLEGACY,
                hwndTarget: hwnd,
            },
            RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x02, // Mouse
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
        ];

        RegisterRawInputDevices(&devices, size_of::<RAWINPUTDEVICE>() as u32)
            .map_err(|e| anyhow!("RegisterRawInputDevices failed: {e}"))?;

        // Message loop: process WM_INPUT and translate to HookMessage.
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if msg.message == WM_INPUT {
                // First query required buffer size.
                let mut size: u32 = 0;
                let header_size = size_of::<RAWINPUTHEADER>() as u32;
                let hraw = HRAWINPUT(msg.lParam.0 as *mut c_void);
                let res = GetRawInputData(hraw, RID_INPUT, None, &mut size, header_size);
                if res == u32::MAX || size == 0 {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                    continue;
                }

                let mut buffer: Vec<u8> = Vec::with_capacity(size as usize);
                buffer.set_len(size as usize);

                let hraw = HRAWINPUT(msg.lParam.0 as *mut c_void);
                let res = GetRawInputData(
                    hraw,
                    RID_INPUT,
                    Some(buffer.as_mut_ptr() as *mut c_void),
                    &mut size,
                    header_size,
                );
                if res == u32::MAX {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                    continue;
                }

                let raw: &RAWINPUT = &*(buffer.as_ptr() as *const RAWINPUT);
                match raw.header.dwType {
                    t if t == RIM_TYPEKEYBOARD.0 => {
                        let kbd = raw.data.keyboard;
                        let vkey = kbd.VKey as u32;
                        let scan_code = kbd.MakeCode as u32;
                        let flags = kbd.Flags as u32;

                        // Normalize virtual key so that left/right modifiers and others
                        // match willhook's expectations.
                        let mut vk_norm = vkey;

                        const VK_SHIFT: u32 = 0x10;
                        const VK_CONTROL: u32 = 0x11;
                        const VK_MENU: u32 = 0x12;
                        const VK_LSHIFT: u32 = 0xA0;
                        const VK_RSHIFT: u32 = 0xA1;
                        const VK_LCONTROL: u32 = 0xA2;
                        const VK_RCONTROL: u32 = 0xA3;
                        const VK_LMENU: u32 = 0xA4;
                        const VK_RMENU: u32 = 0xA5;

                        if vk_norm == VK_SHIFT {
                            match scan_code {
                                42 => vk_norm = VK_LSHIFT,
                                54 => vk_norm = VK_RSHIFT,
                                _ => {}
                            }
                        }

                        if vk_norm == VK_CONTROL {
                            if (flags & RI_KEY_E0) != 0 {
                                vk_norm = VK_RCONTROL;
                            } else {
                                vk_norm = VK_LCONTROL;
                            }
                        }

                        if vk_norm == VK_MENU {
                            if (flags & RI_KEY_E0) != 0 {
                                vk_norm = VK_RMENU;
                            } else {
                                vk_norm = VK_LMENU;
                            }
                        }

                        let key = Some(KeyboardKey::from(vk_norm));

                        // Map Raw Input flags to KeyPress (down/up),
                        // using RI_KEY_BREAK similar to multiinput.
                        let is_break = (flags & RI_KEY_BREAK) != 0;
                        let pressed = if is_break {
                            KeyPress::Up(false)
                        } else {
                            KeyPress::Down(false)
                        };

                        // Check for global hotkeys (Ctrl+Shift+O for overlay toggle)
                        if let Some(command) = hotkey_state.update(vk_norm, !is_break) {
                            let _ = write_command(&mut sink, &command);
                            // Continue processing the key event normally
                        }

                        // Map Raw Input extended flag to low-level hook-style flags
                        // so that keyboard_labels' numpad/extended logic behaves identically.
                        let mut ll_flags = 0u32;
                        if (flags & RI_KEY_E0) != 0 {
                            // LLKHF_EXTENDED == 0x01 in keyboard_labels.rs
                            ll_flags |= 0x01;
                        }

                        let event = KeyboardEvent {
                            pressed,
                            key,
                            vk_code: Some(vk_norm),
                            scan_code: Some(scan_code),
                            flags: Some(ll_flags),
                            is_injected: Some(IsKeyboardEventInjected::NotInjected),
                        };

                        if should_skip_keyboard_event(&event) {
                            let _ = TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                            continue;
                        }

                        let labels = build_key_labels(&event);
                        if labels.is_empty() {
                            let _ = TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                            continue;
                        }

                        let state = match event.pressed {
                            KeyPress::Down(_) => HookKeyState::Down,
                            KeyPress::Up(_) => HookKeyState::Up,
                        };

                        let message = HookMessage {
                            device: InputDeviceKind::Keyboard,
                            labels,
                            state,
                            vk_code: event.vk_code,
                            scan_code: event.scan_code,
                            flags: event.flags,
                        };

                        let _ = write_message(&mut sink, &message);
                    }
                    t if t == RIM_TYPEMOUSE.0 => {
                        let mouse = raw.data.mouse;
                        let button_flags = mouse.Anonymous.Anonymous.usButtonFlags;

                        let mut events: Vec<(String, HookKeyState)> = Vec::new();
                        let mut push = |label: &str, state: HookKeyState| {
                            events.push((label.to_string(), state));
                        };

                        if (button_flags & RI_MOUSE_LEFT_BUTTON_DOWN) != 0 {
                            push("MOUSE1", HookKeyState::Down);
                        }
                        if (button_flags & RI_MOUSE_LEFT_BUTTON_UP) != 0 {
                            push("MOUSE1", HookKeyState::Up);
                        }
                        if (button_flags & RI_MOUSE_RIGHT_BUTTON_DOWN) != 0 {
                            push("MOUSE2", HookKeyState::Down);
                        }
                        if (button_flags & RI_MOUSE_RIGHT_BUTTON_UP) != 0 {
                            push("MOUSE2", HookKeyState::Up);
                        }
                        if (button_flags & RI_MOUSE_MIDDLE_BUTTON_DOWN) != 0 {
                            push("MOUSE3", HookKeyState::Down);
                        }
                        if (button_flags & RI_MOUSE_MIDDLE_BUTTON_UP) != 0 {
                            push("MOUSE3", HookKeyState::Up);
                        }
                        if (button_flags & RI_MOUSE_BUTTON_4_DOWN) != 0 {
                            push("MOUSE4", HookKeyState::Down);
                        }
                        if (button_flags & RI_MOUSE_BUTTON_4_UP) != 0 {
                            push("MOUSE4", HookKeyState::Up);
                        }
                        if (button_flags & RI_MOUSE_BUTTON_5_DOWN) != 0 {
                            push("MOUSE5", HookKeyState::Down);
                        }
                        if (button_flags & RI_MOUSE_BUTTON_5_UP) != 0 {
                            push("MOUSE5", HookKeyState::Up);
                        }

                        for (label, state) in events {
                            let _ = write_message(
                                &mut sink,
                                &HookMessage {
                                    device: InputDeviceKind::Mouse,
                                    labels: vec![label],
                                    state,
                                    vk_code: None,
                                    scan_code: None,
                                    flags: None,
                                },
                            );
                        }
                    }
                    _ => {}
                }
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);

            if msg.message == WM_QUIT {
                break;
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn run_macos() -> Result<()> {
    use rdev::{listen, EventType};

    let mut sink: Box<dyn Write + Send> = Box::new(std::io::stdout());
    let hotkeys = load_hotkeys_from_env();
    let mut hotkey_state = MacHotkeyState::new(
        hotkeys.toggle_overlay,
        hotkeys.toggle_overlay_lock,
        hotkeys.toggle_always_on_top,
    );

    let callback = move |event: rdev::Event| {
        match event.event_type {
            EventType::KeyPress(key) => {
                let key_name = format!("{:?}", key).to_ascii_lowercase();
                if let Some(command) = hotkey_state.update(&key_name, true) {
                    let _ = write_command(&mut sink, &command);
                }

                let labels = mac_key_labels(key, event.name.as_deref());
                if labels.is_empty() {
                    return;
                }

                let message = HookMessage {
                    device: InputDeviceKind::Keyboard,
                    labels,
                    state: HookKeyState::Down,
                    vk_code: None,
                    scan_code: None,
                    flags: None,
                };
                let _ = write_message(&mut sink, &message);
            }
            EventType::KeyRelease(key) => {
                let key_name = format!("{:?}", key).to_ascii_lowercase();
                let _ = hotkey_state.update(&key_name, false);

                let labels = mac_key_labels(key, event.name.as_deref());
                if labels.is_empty() {
                    return;
                }

                let message = HookMessage {
                    device: InputDeviceKind::Keyboard,
                    labels,
                    state: HookKeyState::Up,
                    vk_code: None,
                    scan_code: None,
                    flags: None,
                };
                let _ = write_message(&mut sink, &message);
            }
            EventType::ButtonPress(button) => {
                if let Some(label) = mac_mouse_label(button) {
                    let _ = write_message(
                        &mut sink,
                        &HookMessage {
                            device: InputDeviceKind::Mouse,
                            labels: vec![label],
                            state: HookKeyState::Down,
                            vk_code: None,
                            scan_code: None,
                            flags: None,
                        },
                    );
                }
            }
            EventType::ButtonRelease(button) => {
                if let Some(label) = mac_mouse_label(button) {
                    let _ = write_message(
                        &mut sink,
                        &HookMessage {
                            device: InputDeviceKind::Mouse,
                            labels: vec![label],
                            state: HookKeyState::Up,
                            vk_code: None,
                            scan_code: None,
                            flags: None,
                        },
                    );
                }
            }
            _ => {}
        }
    };

    listen(callback).map_err(|err| anyhow!("macOS input listener failed: {err:?}"))?;
    Ok(())
}

#[cfg(target_os = "linux")]
use crate::keyboard_labels::KeyboardKey;

#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Clone, Copy)]
struct EvdevEvent {
    time: nix::libc::timeval,
    type_: u16,
    code: u16,
    value: i32,
}

#[cfg(target_os = "linux")]
const EV_SZ: usize = std::mem::size_of::<EvdevEvent>();

#[cfg(target_os = "linux")]
impl Default for EvdevEvent {
    fn default() -> Self {
        Self {
            time: nix::libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            type_: 0,
            code: 0,
            value: 0,
        }
    }
}

#[cfg(target_os = "linux")]
fn parse_input_events(bytes: &[u8]) -> impl Iterator<Item = EvdevEvent> + '_ {
    bytes.chunks_exact(EV_SZ).filter_map(move |c| {
        let mut ev = EvdevEvent::default();
        unsafe {
            std::ptr::copy_nonoverlapping(c.as_ptr(), &mut ev as *mut EvdevEvent as *mut u8, EV_SZ);
        }
        Some(ev)
    })
}

#[cfg(target_os = "linux")]
fn parse_uid_gid() -> Result<(u32, u32)> {
    let mut uid: Option<u32> = None;
    let mut gid: Option<u32> = None;
    for arg in std::env::args() {
        if let Some(v) = arg.strip_prefix("--drop-uid=") {
            uid = Some(v.parse()?);
        } else if let Some(v) = arg.strip_prefix("--drop-gid=") {
            gid = Some(v.parse()?);
        }
    }
    let uid = uid.ok_or_else(|| anyhow!("missing --drop-uid="))?;
    let gid = gid.ok_or_else(|| anyhow!("missing --drop-gid="))?;
    Ok((uid, gid))
}

#[cfg(target_os = "linux")]
fn drop_privileges_permanently(target_uid: u32, target_gid: u32) -> anyhow::Result<()> {
    use nix::unistd::{self, Gid, Uid};

    unistd::setgroups(&[]).ok(); // 실패해도 진행하게 두고 싶으면 ok()다요
    unistd::setresgid(
        Gid::from_raw(target_gid),
        Gid::from_raw(target_gid),
        Gid::from_raw(target_gid),
    )?;
    unistd::setresuid(
        Uid::from_raw(target_uid),
        Uid::from_raw(target_uid),
        Uid::from_raw(target_uid),
    )?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn mouse_label_from_evdev(code: u32) -> Option<String> {
    match code {
        272 => Some("MOUSE1"),
        273 => Some("MOUSE2"),
        274 => Some("MOUSE3"),
        275 => Some("MOUSE4"),
        276 => Some("MOUSE5"),
        _ => None,
    }
    .map(|s| s.to_string())
}

#[cfg(target_os = "linux")]
fn key_label_from_evdev(code: u32) -> KeyboardKey {
    match code {
        16 => KeyboardKey::Q, // KEY_Q
        17 => KeyboardKey::W, // KEY_W
        18 => KeyboardKey::E, // KEY_E
        19 => KeyboardKey::R, // KEY_R
        20 => KeyboardKey::T, // KEY_T
        21 => KeyboardKey::Y, // KEY_Y
        22 => KeyboardKey::U, // KEY_U
        23 => KeyboardKey::I, // KEY_I
        24 => KeyboardKey::O, // KEY_O
        25 => KeyboardKey::P, // KEY_P

        30 => KeyboardKey::A, // KEY_A
        31 => KeyboardKey::S, // KEY_S
        32 => KeyboardKey::D, // KEY_D
        33 => KeyboardKey::F, // KEY_F
        34 => KeyboardKey::G, // KEY_G
        35 => KeyboardKey::H, // KEY_H
        36 => KeyboardKey::J, // KEY_J
        37 => KeyboardKey::K, // KEY_K
        38 => KeyboardKey::L, // KEY_L

        44 => KeyboardKey::Z, // KEY_Z
        45 => KeyboardKey::X, // KEY_X
        46 => KeyboardKey::C, // KEY_C
        47 => KeyboardKey::V, // KEY_V
        48 => KeyboardKey::B, // KEY_B
        49 => KeyboardKey::N, // KEY_N
        50 => KeyboardKey::M, // KEY_M

        2 => KeyboardKey::Number1,  // KEY_1
        3 => KeyboardKey::Number2,  // KEY_2
        4 => KeyboardKey::Number3,  // KEY_3
        5 => KeyboardKey::Number4,  // KEY_4
        6 => KeyboardKey::Number5,  // KEY_5
        7 => KeyboardKey::Number6,  // KEY_6
        8 => KeyboardKey::Number7,  // KEY_7
        9 => KeyboardKey::Number8,  // KEY_8
        10 => KeyboardKey::Number9, // KEY_9
        11 => KeyboardKey::Number0, // KEY_0

        56 => KeyboardKey::LeftAlt,      // KEY_LEFTALT
        100 => KeyboardKey::RightAlt,    // KEY_RIGHTALT
        42 => KeyboardKey::LeftShift,    // KEY_LEFTSHIFT
        54 => KeyboardKey::RightShift,   // KEY_RIGHTSHIFT
        29 => KeyboardKey::LeftControl,  // KEY_LEFTCTRL
        97 => KeyboardKey::RightControl, // KEY_RIGHTCTRL

        14 => KeyboardKey::BackSpace, // KEY_BACKSPACE
        15 => KeyboardKey::Tab,       // KEY_TAB
        28 => KeyboardKey::Enter,     // KEY_ENTER
        1 => KeyboardKey::Escape,     // KEY_ESC
        57 => KeyboardKey::Space,     // KEY_SPACE

        104 => KeyboardKey::PageUp,     // KEY_PAGEUP
        109 => KeyboardKey::PageDown,   // KEY_PAGEDOWN
        102 => KeyboardKey::Home,       // KEY_HOME
        105 => KeyboardKey::ArrowLeft,  // KEY_LEFT
        103 => KeyboardKey::ArrowUp,    // KEY_UP
        106 => KeyboardKey::ArrowRight, // KEY_RIGHT
        108 => KeyboardKey::ArrowDown,  // KEY_DOWN

        210 => KeyboardKey::Print,      // KEY_PRINT (AC Print)
        99 => KeyboardKey::PrintScreen, // KEY_SYSRQ (Print Screen / SysRq)

        110 => KeyboardKey::Insert, // KEY_INSERT
        111 => KeyboardKey::Delete, // KEY_DELETE

        125 => KeyboardKey::LeftWindows,  // KEY_LEFTMETA
        126 => KeyboardKey::RightWindows, // KEY_RIGHTMETA

        51 => KeyboardKey::Comma,         // KEY_COMMA
        52 => KeyboardKey::Period,        // KEY_DOT
        53 => KeyboardKey::Slash,         // KEY_SLASH
        39 => KeyboardKey::SemiColon,     // KEY_SEMICOLON
        40 => KeyboardKey::Apostrophe,    // KEY_APOSTROPHE
        26 => KeyboardKey::LeftBrace,     // KEY_LEFTBRACE
        43 => KeyboardKey::BackwardSlash, // KEY_BACKSLASH
        27 => KeyboardKey::RightBrace,    // KEY_RIGHTBRACE
        41 => KeyboardKey::Grave,         // KEY_GRAVE

        78 => KeyboardKey::Add,        // KEY_KPPLUS
        74 => KeyboardKey::Subtract,   // KEY_KPMINUS
        83 => KeyboardKey::Decimal,    // KEY_KPDOT
        98 => KeyboardKey::Divide,     // KEY_KPSLASH
        55 => KeyboardKey::Multiply,   // KEY_KPASTERISK
        121 => KeyboardKey::Separator, // KEY_KPCOMMA

        59 => KeyboardKey::F1,  // KEY_F1
        60 => KeyboardKey::F2,  // KEY_F2
        61 => KeyboardKey::F3,  // KEY_F3
        62 => KeyboardKey::F4,  // KEY_F4
        63 => KeyboardKey::F5,  // KEY_F5
        64 => KeyboardKey::F6,  // KEY_F6
        65 => KeyboardKey::F7,  // KEY_F7
        66 => KeyboardKey::F8,  // KEY_F8
        67 => KeyboardKey::F9,  // KEY_F9
        68 => KeyboardKey::F10, // KEY_F10
        87 => KeyboardKey::F11, // KEY_F11
        88 => KeyboardKey::F12, // KEY_F12

        183 => KeyboardKey::F13, // KEY_F13
        184 => KeyboardKey::F14, // KEY_F14
        185 => KeyboardKey::F15, // KEY_F15
        186 => KeyboardKey::F16, // KEY_F16
        187 => KeyboardKey::F17, // KEY_F17
        188 => KeyboardKey::F18, // KEY_F18
        189 => KeyboardKey::F19, // KEY_F19
        190 => KeyboardKey::F20, // KEY_F20
        191 => KeyboardKey::F21, // KEY_F21
        192 => KeyboardKey::F22, // KEY_F22
        193 => KeyboardKey::F23, // KEY_F23
        194 => KeyboardKey::F24, // KEY_F24

        69 => KeyboardKey::NumLock,    // KEY_NUMLOCK
        70 => KeyboardKey::ScrollLock, // KEY_SCROLLLOCK
        58 => KeyboardKey::CapsLock,   // KEY_CAPSLOCK

        82 => KeyboardKey::Numpad0, // KEY_KP0
        79 => KeyboardKey::Numpad1, // KEY_KP1
        80 => KeyboardKey::Numpad2, // KEY_KP2
        81 => KeyboardKey::Numpad3, // KEY_KP3
        75 => KeyboardKey::Numpad4, // KEY_KP4
        76 => KeyboardKey::Numpad5, // KEY_KP5
        77 => KeyboardKey::Numpad6, // KEY_KP6
        71 => KeyboardKey::Numpad7, // KEY_KP7
        72 => KeyboardKey::Numpad8, // KEY_KP8
        73 => KeyboardKey::Numpad9, // KEY_KP9

        other => KeyboardKey::Other(other),
    }
}

#[cfg(target_os = "linux")]
fn run_linux() -> Result<()> {
    use std::{
         fs::File, path::{Path, PathBuf},
        os::fd::AsRawFd,
    };
    use anyhow::Context;
    use nix::poll::PollFd;

    const EV_KEY: u16 = 0x01;

    //=== RUNS IN ROOT
    let mut sink: Box<dyn Write + Send> = Box::new(std::io::stdout());

    let (uid, gid) = parse_uid_gid().context("Failed to get UID/GID")?;

    // Collect /dev/input/event* evdev entries.
    // The program is running as root at this stage due to the access rules with evdev.
    let mut paths: Vec<PathBuf> = vec![];
    let by_id = {
        let path = Path::new("/dev/input/by-id/");
        if path.is_dir() {
            path
        } else {
            Path::new("/dev/input/by-path/")
        }
    };
    if by_id.is_dir() {
        for entry in
            std::fs::read_dir(by_id).with_context(|| format!("Failed to read dir {by_id:?}"))?
        {
            let ent =
                entry.with_context(|| format!("Error while fetching entries in {by_id:?}"))?;
            let p = ent.path();
            let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if name.ends_with("-event-kbd") || name.ends_with("-event-mouse") {
                paths.push(p);
            }
        }
    }
    if paths.is_empty() {
        return Err(anyhow!("No kbd/mouse devices found in /dev/input"));
    }

    // Open up device entries
    // TODO: as O_NONBLOCK
    let mut devs: Vec<(File, Vec<u8>)> = vec![];
    for p in paths {
        if let Ok(f) = File::open(&p) {
            devs.push((f, Vec::new()));
        }
    }
    if devs.is_empty() {
        return Err(anyhow!("None of the found devices could be opened"));
    }

    //=== ROOT PRIV ENDS
    drop_privileges_permanently(uid, gid).context("Failed to drop privileges")?;

    // Read events from devices
    let mut tmp = vec![0u8; EV_SZ * 4096];

    loop {
        use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
        use std::{os::fd::AsFd, io::Read};

        // Poll devices
        let mut fds: Vec<PollFd> = devs
            .iter()
            .map(|(f, _)| PollFd::new(f.as_fd(), PollFlags::POLLIN))
            .collect();
        let n = poll(&mut fds, PollTimeout::NONE).context("poll(fds) failed")?;
        if n <= 0 {
            continue;
        }

        // Search for devices ready to be read from
        let mut ready_read = vec![];
        for (idx, pfd) in fds.iter().enumerate() {
            let Some(re) = pfd.revents() else {
                continue;
            };
            if !re.contains(PollFlags::POLLIN) {
                continue;
            }
            ready_read.push(idx);
        }

        // Actually read from devices
        for idx in ready_read {
            let (file, buf) = &mut devs[idx];
            let read_n = match file.read(&mut tmp) {
                Ok(0) | Err(_) => continue,
                Ok(n) => n,
            };

            // Save the output of devs[idx].0 (the evdev file) into devs[idx].1 (the buffer)
            buf.extend_from_slice(&tmp[..read_n]);

            // Now drain the buffer into chunks of ev_sz
            let complete_len = (buf.len() / EV_SZ) * EV_SZ;
            if complete_len == 0 {
                continue;
            }
            for chunk in buf[..complete_len].chunks_exact(EV_SZ) {
                // Convert the chunk to event
                let mut ev = EvdevEvent::default();
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        chunk.as_ptr(),
                        &mut ev as *mut EvdevEvent as *mut u8,
                        std::mem::size_of::<EvdevEvent>(),
                    );
                }
                let ev = ev;

                // Process the keypress event
                if ev.type_ != EV_KEY {
                    continue;
                }
                let state = match ev.value {
                    1 => HookKeyState::Down,
                    0 => HookKeyState::Up,
                    _ => continue,
                };
                let code = ev.code as u32;

                if let Some(label) = mouse_label_from_evdev(code) {
                    // If mouse
                    let _ = write_message(
                        &mut sink,
                        &HookMessage {
                            device: InputDeviceKind::Mouse,
                            labels: vec![label],
                            state,
                            vk_code: None,
                            scan_code: Some(code),
                            flags: None,
                        },
                    );
                } else {
                    use crate::keyboard_labels::{build_key_labels, KeyPress, KeyboardEvent};
                    let key = key_label_from_evdev(code);
                    let labels = build_key_labels(&KeyboardEvent {
                        pressed: match state {
                            HookKeyState::Down => KeyPress::Down(false),
                            HookKeyState::Up => KeyPress::Up(false),
                        },
                        key: Some(key),
                        vk_code: None,
                        scan_code: None,
                        flags: None,
                        is_injected: None,
                    });
                    if labels.is_empty() {
                        continue;
                    }
                    let _ = write_message(
                        &mut sink,
                        &HookMessage {
                            device: InputDeviceKind::Keyboard,
                            labels,
                            state,
                            vk_code: None,
                            scan_code: None,
                            flags: None,
                        },
                    );
                }
            }
            buf.drain(..complete_len);
        }
    }
}
