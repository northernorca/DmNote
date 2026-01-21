use std::io::Write;

use anyhow::{anyhow, Result};
use serde_json::to_string;

use crate::ipc::{DaemonCommand, HookKeyState, HookMessage, InputDeviceKind};
use crate::keyboard::{
    daemon::write_message,
    labels::{build_key_labels, KeyPress, KeyboardEvent, KeyboardKey},
};
use crate::models::{ShortcutBinding, ShortcutsState};

#[repr(C)]
#[derive(Clone, Copy)]
struct EvdevEvent {
    time: nix::libc::timeval,
    type_: u16,
    code: u16,
    value: i32,
}

const EV_SZ: usize = std::mem::size_of::<EvdevEvent>();

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

fn parse_input_events(bytes: &[u8]) -> impl Iterator<Item = EvdevEvent> + '_ {
    bytes.chunks_exact(EV_SZ).filter_map(move |c| {
        let mut ev = EvdevEvent::default();
        unsafe {
            std::ptr::copy_nonoverlapping(c.as_ptr(), &mut ev as *mut EvdevEvent as *mut u8, EV_SZ);
        }
        Some(ev)
    })
}

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

pub(super) fn run_linux() -> Result<()> {
    use anyhow::Context;
    use nix::poll::PollFd;
    use std::{
        fs::File,
        os::fd::AsRawFd,
        path::{Path, PathBuf},
    };

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
        use std::{io::Read, os::fd::AsFd};

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
