use std::sync::{Arc, Mutex};
use std::thread;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Clone)]
pub struct HotkeyEntry {
    pub vk_code: u32,
    pub modifiers: Modifiers,
    pub modifier_only: bool,
    pub shortcut_type: String,
    pub key_string: String,
}

pub type HotkeyTable = Arc<Mutex<Vec<HotkeyEntry>>>;
type ActionCallback  = Arc<dyn Fn(String) + Send + Sync>;

struct HookState {
    table:    HotkeyTable,
    callback: ActionCallback,
    dirty:    Mutex<bool>,
}

static HOOK_STATE: std::sync::OnceLock<HookState> = std::sync::OnceLock::new();

#[inline]
fn is_modifier_vk(vk: u32) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0xA0..=0xA5)
}

#[inline]
unsafe fn read_modifiers() -> Modifiers {
    Modifiers {
        ctrl:  GetAsyncKeyState(0x11) as u16 & 0x8000 != 0,
        alt:   GetAsyncKeyState(0x12) as u16 & 0x8000 != 0,
        shift: GetAsyncKeyState(0x10) as u16 & 0x8000 != 0,
    }
}

#[inline]
fn modifiers_before_keyup(vk: u32, after: &Modifiers) -> Modifiers {
    Modifiers {
        ctrl:  after.ctrl  || matches!(vk, 0x11 | 0xA2 | 0xA3),
        alt:   after.alt   || matches!(vk, 0x12 | 0xA4 | 0xA5),
        shift: after.shift || matches!(vk, 0x10 | 0xA0 | 0xA1),
    }
}

#[inline]
fn all_up(m: &Modifiers) -> bool {
    !m.ctrl && !m.alt && !m.shift
}

unsafe extern "system" fn keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let msg  = w_param.0 as u32;
        let kb   = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
        let vk   = kb.vkCode;

        if let Some(state) = HOOK_STATE.get() {
            match msg {
                m if (m == WM_KEYDOWN || m == WM_SYSKEYDOWN) => {
                    if is_modifier_vk(vk) {
                        let mods = read_modifiers();
                        if (mods.ctrl as u8 + mods.alt as u8 + mods.shift as u8) == 1 {
                            *state.dirty.lock().unwrap() = false;
                        }
                    } else {
                        *state.dirty.lock().unwrap() = true;

                        let mods  = read_modifiers();
                        let table = state.table.lock().unwrap();
                        if let Some(e) = table.iter().find(|e| {
                            !e.modifier_only && e.vk_code == vk && e.modifiers == mods
                        }) {
                            let action = e.shortcut_type.clone();
                            drop(table);
                            (state.callback)(action);
                        }
                    }
                }

                m if (m == WM_KEYUP || m == WM_SYSKEYUP) && is_modifier_vk(vk) => {
                    let dirty = *state.dirty.lock().unwrap();
                    let after = read_modifiers();

                    if !dirty && all_up(&after) {
                        let was   = modifiers_before_keyup(vk, &after);
                        let table = state.table.lock().unwrap();
                        if let Some(e) = table.iter().find(|e| {
                            e.modifier_only && e.modifiers == was
                        }) {
                            let action = e.shortcut_type.clone();
                            drop(table);
                            (state.callback)(action);
                        }
                    }

                    if all_up(&after) {
                        *state.dirty.lock().unwrap() = false;
                    }
                }

                _ => {}
            }
        }
    }

    CallNextHookEx(None, n_code, w_param, l_param)
}

pub fn start_hook(
    table: HotkeyTable,
    callback: impl Fn(String) + Send + Sync + 'static,
) {
    HOOK_STATE.set(HookState {
        table,
        callback: Arc::new(callback),
        dirty: Mutex::new(false),
    }).ok();

    thread::Builder::new()
        .name("keyboard-hook".into())
        .spawn(|| unsafe {
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)
                .expect("Failed to install WH_KEYBOARD_LL hook");
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWindowsHookEx(hook);
        })
        .expect("Failed to spawn keyboard hook thread");
}

pub fn parse_shortcut(key_string: &str, shortcut_type: &str) -> Option<HotkeyEntry> {
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    let mut vk: Option<u32> = None;

    for part in key_string.split('+') {
        match part {
            "Ctrl"  => ctrl  = true,
            "Alt"   => alt   = true,
            "Shift" => shift = true,
            "Super" => {}
            other   => vk = key_to_vk(other),
        }
    }

    if !ctrl && !alt && !shift && vk.is_none() { return None; }

    Some(HotkeyEntry {
        vk_code:       vk.unwrap_or(0),
        modifiers:     Modifiers { ctrl, alt, shift },
        modifier_only: vk.is_none(),
        shortcut_type: shortcut_type.to_string(),
        key_string:    key_string.to_string(),
    })
}

fn key_to_vk(key: &str) -> Option<u32> {
    if let Some(n) = key.strip_prefix('F').and_then(|s| s.parse::<u32>().ok()) {
        if (1..=24).contains(&n) { return Some(0x70 + n - 1); }
    }
    if let Some(c) = key.strip_prefix("Key").and_then(|s| s.chars().next()) {
        if c.is_ascii_uppercase() { return Some(0x41 + (c as u32 - 'A' as u32)); }
    }
    if let Some(d) = key.strip_prefix("Digit").and_then(|s| s.parse::<u32>().ok()) {
        if d <= 9 { return Some(0x30 + d); }
    }
    if let Some(d) = key.strip_prefix("Numpad").and_then(|s| s.parse::<u32>().ok()) {
        if d <= 9 { return Some(0x60 + d); }
    }
    Some(match key {
        "NumpadMultiply" => 0x6A, "NumpadAdd"      => 0x6B,
        "NumpadSubtract" => 0x6D, "NumpadDecimal"  => 0x6E,
        "NumpadDivide"   => 0x6F, "NumpadEnter"    => 0x0D,
        "Space"          => 0x20, "Enter"          => 0x0D,
        "Tab"            => 0x09, "Backspace"      => 0x08,
        "Escape"         => 0x1B, "Delete"         => 0x2E,
        "Insert"         => 0x2D, "Home"           => 0x24,
        "End"            => 0x23, "PageUp"         => 0x21,
        "PageDown"       => 0x22, "PrintScreen"    => 0x2C,
        "Pause"          => 0x13,
        "ArrowLeft"      => 0x25, "ArrowUp"        => 0x26,
        "ArrowRight"     => 0x27, "ArrowDown"      => 0x28,
        "Minus"          => 0xBD, "Equal"          => 0xBB,
        "BracketLeft"    => 0xDB, "BracketRight"   => 0xDD,
        "Backslash"      => 0xDC, "Semicolon"      => 0xBA,
        "Quote"          => 0xDE, "Comma"          => 0xBC,
        "Period"         => 0xBE, "Slash"          => 0xBF,
        "Backquote"      => 0xC0,
        _ => return None,
    })
}