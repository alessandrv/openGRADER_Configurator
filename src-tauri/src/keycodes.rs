// Common keycodes for OpenGrader (matching QMK keycodes)
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keycode {
    pub code: u16,
    pub name: String,
    pub display_name: String,
    pub category: String,
}

#[allow(dead_code)]
pub fn get_keycode_map() -> HashMap<u16, Keycode> {
    let mut map = HashMap::new();
    
    // Basic keys
    map.insert(0x0000, Keycode { code: 0x0000, name: "KC_NO".to_string(), display_name: "None".to_string(), category: "Basic".to_string() });
    map.insert(0x0001, Keycode { code: 0x0001, name: "KC_ROLL_OVER".to_string(), display_name: "Roll Over".to_string(), category: "Basic".to_string() });
    map.insert(0x0002, Keycode { code: 0x0002, name: "KC_POST_FAIL".to_string(), display_name: "Post Fail".to_string(), category: "Basic".to_string() });
    map.insert(0x0003, Keycode { code: 0x0003, name: "KC_UNDEFINED".to_string(), display_name: "Undefined".to_string(), category: "Basic".to_string() });
    
    // Letters
    map.insert(0x0004, Keycode { code: 0x0004, name: "KC_A".to_string(), display_name: "A".to_string(), category: "Letters".to_string() });
    map.insert(0x0005, Keycode { code: 0x0005, name: "KC_B".to_string(), display_name: "B".to_string(), category: "Letters".to_string() });
    map.insert(0x0006, Keycode { code: 0x0006, name: "KC_C".to_string(), display_name: "C".to_string(), category: "Letters".to_string() });
    map.insert(0x0007, Keycode { code: 0x0007, name: "KC_D".to_string(), display_name: "D".to_string(), category: "Letters".to_string() });
    map.insert(0x0008, Keycode { code: 0x0008, name: "KC_E".to_string(), display_name: "E".to_string(), category: "Letters".to_string() });
    map.insert(0x0009, Keycode { code: 0x0009, name: "KC_F".to_string(), display_name: "F".to_string(), category: "Letters".to_string() });
    map.insert(0x000A, Keycode { code: 0x000A, name: "KC_G".to_string(), display_name: "G".to_string(), category: "Letters".to_string() });
    map.insert(0x000B, Keycode { code: 0x000B, name: "KC_H".to_string(), display_name: "H".to_string(), category: "Letters".to_string() });
    map.insert(0x000C, Keycode { code: 0x000C, name: "KC_I".to_string(), display_name: "I".to_string(), category: "Letters".to_string() });
    map.insert(0x000D, Keycode { code: 0x000D, name: "KC_J".to_string(), display_name: "J".to_string(), category: "Letters".to_string() });
    map.insert(0x000E, Keycode { code: 0x000E, name: "KC_K".to_string(), display_name: "K".to_string(), category: "Letters".to_string() });
    map.insert(0x000F, Keycode { code: 0x000F, name: "KC_L".to_string(), display_name: "L".to_string(), category: "Letters".to_string() });
    map.insert(0x0010, Keycode { code: 0x0010, name: "KC_M".to_string(), display_name: "M".to_string(), category: "Letters".to_string() });
    map.insert(0x0011, Keycode { code: 0x0011, name: "KC_N".to_string(), display_name: "N".to_string(), category: "Letters".to_string() });
    map.insert(0x0012, Keycode { code: 0x0012, name: "KC_O".to_string(), display_name: "O".to_string(), category: "Letters".to_string() });
    map.insert(0x0013, Keycode { code: 0x0013, name: "KC_P".to_string(), display_name: "P".to_string(), category: "Letters".to_string() });
    map.insert(0x0014, Keycode { code: 0x0014, name: "KC_Q".to_string(), display_name: "Q".to_string(), category: "Letters".to_string() });
    map.insert(0x0015, Keycode { code: 0x0015, name: "KC_R".to_string(), display_name: "R".to_string(), category: "Letters".to_string() });
    map.insert(0x0016, Keycode { code: 0x0016, name: "KC_S".to_string(), display_name: "S".to_string(), category: "Letters".to_string() });
    map.insert(0x0017, Keycode { code: 0x0017, name: "KC_T".to_string(), display_name: "T".to_string(), category: "Letters".to_string() });
    map.insert(0x0018, Keycode { code: 0x0018, name: "KC_U".to_string(), display_name: "U".to_string(), category: "Letters".to_string() });
    map.insert(0x0019, Keycode { code: 0x0019, name: "KC_V".to_string(), display_name: "V".to_string(), category: "Letters".to_string() });
    map.insert(0x001A, Keycode { code: 0x001A, name: "KC_W".to_string(), display_name: "W".to_string(), category: "Letters".to_string() });
    map.insert(0x001B, Keycode { code: 0x001B, name: "KC_X".to_string(), display_name: "X".to_string(), category: "Letters".to_string() });
    map.insert(0x001C, Keycode { code: 0x001C, name: "KC_Y".to_string(), display_name: "Y".to_string(), category: "Letters".to_string() });
    map.insert(0x001D, Keycode { code: 0x001D, name: "KC_Z".to_string(), display_name: "Z".to_string(), category: "Letters".to_string() });
    
    // Numbers
    map.insert(0x001E, Keycode { code: 0x001E, name: "KC_1".to_string(), display_name: "1".to_string(), category: "Numbers".to_string() });
    map.insert(0x001F, Keycode { code: 0x001F, name: "KC_2".to_string(), display_name: "2".to_string(), category: "Numbers".to_string() });
    map.insert(0x0020, Keycode { code: 0x0020, name: "KC_3".to_string(), display_name: "3".to_string(), category: "Numbers".to_string() });
    map.insert(0x0021, Keycode { code: 0x0021, name: "KC_4".to_string(), display_name: "4".to_string(), category: "Numbers".to_string() });
    map.insert(0x0022, Keycode { code: 0x0022, name: "KC_5".to_string(), display_name: "5".to_string(), category: "Numbers".to_string() });
    map.insert(0x0023, Keycode { code: 0x0023, name: "KC_6".to_string(), display_name: "6".to_string(), category: "Numbers".to_string() });
    map.insert(0x0024, Keycode { code: 0x0024, name: "KC_7".to_string(), display_name: "7".to_string(), category: "Numbers".to_string() });
    map.insert(0x0025, Keycode { code: 0x0025, name: "KC_8".to_string(), display_name: "8".to_string(), category: "Numbers".to_string() });
    map.insert(0x0026, Keycode { code: 0x0026, name: "KC_9".to_string(), display_name: "9".to_string(), category: "Numbers".to_string() });
    map.insert(0x0027, Keycode { code: 0x0027, name: "KC_0".to_string(), display_name: "0".to_string(), category: "Numbers".to_string() });
    
    // Special keys
    map.insert(0x0028, Keycode { code: 0x0028, name: "KC_ENTER".to_string(), display_name: "Enter".to_string(), category: "Special".to_string() });
    map.insert(0x0029, Keycode { code: 0x0029, name: "KC_ESCAPE".to_string(), display_name: "Escape".to_string(), category: "Special".to_string() });
    map.insert(0x002A, Keycode { code: 0x002A, name: "KC_BACKSPACE".to_string(), display_name: "Backspace".to_string(), category: "Special".to_string() });
    map.insert(0x002B, Keycode { code: 0x002B, name: "KC_TAB".to_string(), display_name: "Tab".to_string(), category: "Special".to_string() });
    map.insert(0x002C, Keycode { code: 0x002C, name: "KC_SPACE".to_string(), display_name: "Space".to_string(), category: "Special".to_string() });
    
    // Punctuation
    map.insert(0x002D, Keycode { code: 0x002D, name: "KC_MINUS".to_string(), display_name: "-".to_string(), category: "Punctuation".to_string() });
    map.insert(0x002E, Keycode { code: 0x002E, name: "KC_EQUAL".to_string(), display_name: "=".to_string(), category: "Punctuation".to_string() });
    map.insert(0x002F, Keycode { code: 0x002F, name: "KC_LEFT_BRACKET".to_string(), display_name: "[".to_string(), category: "Punctuation".to_string() });
    map.insert(0x0030, Keycode { code: 0x0030, name: "KC_RIGHT_BRACKET".to_string(), display_name: "]".to_string(), category: "Punctuation".to_string() });
    map.insert(0x0031, Keycode { code: 0x0031, name: "KC_BACKSLASH".to_string(), display_name: "\\".to_string(), category: "Punctuation".to_string() });
    map.insert(0x0033, Keycode { code: 0x0033, name: "KC_SEMICOLON".to_string(), display_name: ";".to_string(), category: "Punctuation".to_string() });
    map.insert(0x0034, Keycode { code: 0x0034, name: "KC_QUOTE".to_string(), display_name: "'".to_string(), category: "Punctuation".to_string() });
    map.insert(0x0035, Keycode { code: 0x0035, name: "KC_GRAVE".to_string(), display_name: "`".to_string(), category: "Punctuation".to_string() });
    map.insert(0x0036, Keycode { code: 0x0036, name: "KC_COMMA".to_string(), display_name: ",".to_string(), category: "Punctuation".to_string() });
    map.insert(0x0037, Keycode { code: 0x0037, name: "KC_DOT".to_string(), display_name: ".".to_string(), category: "Punctuation".to_string() });
    map.insert(0x0038, Keycode { code: 0x0038, name: "KC_SLASH".to_string(), display_name: "/".to_string(), category: "Punctuation".to_string() });
    
    // Function keys
    map.insert(0x003A, Keycode { code: 0x003A, name: "KC_F1".to_string(), display_name: "F1".to_string(), category: "Function".to_string() });
    map.insert(0x003B, Keycode { code: 0x003B, name: "KC_F2".to_string(), display_name: "F2".to_string(), category: "Function".to_string() });
    map.insert(0x003C, Keycode { code: 0x003C, name: "KC_F3".to_string(), display_name: "F3".to_string(), category: "Function".to_string() });
    map.insert(0x003D, Keycode { code: 0x003D, name: "KC_F4".to_string(), display_name: "F4".to_string(), category: "Function".to_string() });
    map.insert(0x003E, Keycode { code: 0x003E, name: "KC_F5".to_string(), display_name: "F5".to_string(), category: "Function".to_string() });
    map.insert(0x003F, Keycode { code: 0x003F, name: "KC_F6".to_string(), display_name: "F6".to_string(), category: "Function".to_string() });
    map.insert(0x0040, Keycode { code: 0x0040, name: "KC_F7".to_string(), display_name: "F7".to_string(), category: "Function".to_string() });
    map.insert(0x0041, Keycode { code: 0x0041, name: "KC_F8".to_string(), display_name: "F8".to_string(), category: "Function".to_string() });
    map.insert(0x0042, Keycode { code: 0x0042, name: "KC_F9".to_string(), display_name: "F9".to_string(), category: "Function".to_string() });
    map.insert(0x0043, Keycode { code: 0x0043, name: "KC_F10".to_string(), display_name: "F10".to_string(), category: "Function".to_string() });
    map.insert(0x0044, Keycode { code: 0x0044, name: "KC_F11".to_string(), display_name: "F11".to_string(), category: "Function".to_string() });
    map.insert(0x0045, Keycode { code: 0x0045, name: "KC_F12".to_string(), display_name: "F12".to_string(), category: "Function".to_string() });
    map.insert(0x0039, Keycode { code: 0x0039, name: "KC_CAPSLOCK".to_string(), display_name: "Caps Lock".to_string(), category: "Special".to_string() });
    map.insert(0x0046, Keycode { code: 0x0046, name: "KC_PSCR".to_string(), display_name: "Print Screen".to_string(), category: "Function".to_string() });
    map.insert(0x0047, Keycode { code: 0x0047, name: "KC_SCRL".to_string(), display_name: "Scroll Lock".to_string(), category: "Function".to_string() });
    map.insert(0x0048, Keycode { code: 0x0048, name: "KC_PAUS".to_string(), display_name: "Pause".to_string(), category: "Function".to_string() });
    map.insert(0x0049, Keycode { code: 0x0049, name: "KC_INSERT".to_string(), display_name: "Insert".to_string(), category: "Navigation".to_string() });
    map.insert(0x004A, Keycode { code: 0x004A, name: "KC_HOME".to_string(), display_name: "Home".to_string(), category: "Navigation".to_string() });
    map.insert(0x004B, Keycode { code: 0x004B, name: "KC_PGUP".to_string(), display_name: "Page Up".to_string(), category: "Navigation".to_string() });
    map.insert(0x004C, Keycode { code: 0x004C, name: "KC_DELETE".to_string(), display_name: "Delete".to_string(), category: "Navigation".to_string() });
    map.insert(0x004D, Keycode { code: 0x004D, name: "KC_END".to_string(), display_name: "End".to_string(), category: "Navigation".to_string() });
    map.insert(0x004E, Keycode { code: 0x004E, name: "KC_PGDOWN".to_string(), display_name: "Page Down".to_string(), category: "Navigation".to_string() });
    
    // Arrow keys
    map.insert(0x004F, Keycode { code: 0x004F, name: "KC_RIGHT".to_string(), display_name: "→".to_string(), category: "Navigation".to_string() });
    map.insert(0x0050, Keycode { code: 0x0050, name: "KC_LEFT".to_string(), display_name: "←".to_string(), category: "Navigation".to_string() });
    map.insert(0x0051, Keycode { code: 0x0051, name: "KC_DOWN".to_string(), display_name: "↓".to_string(), category: "Navigation".to_string() });
    map.insert(0x0052, Keycode { code: 0x0052, name: "KC_UP".to_string(), display_name: "↑".to_string(), category: "Navigation".to_string() });

    // Keypad keys
    map.insert(0x0053, Keycode { code: 0x0053, name: "KC_NUMLOCK".to_string(), display_name: "Num Lock".to_string(), category: "Keypad".to_string() });
    map.insert(0x0054, Keycode { code: 0x0054, name: "KC_KP_SLASH".to_string(), display_name: "Keypad /".to_string(), category: "Keypad".to_string() });
    map.insert(0x0055, Keycode { code: 0x0055, name: "KC_KP_ASTERISK".to_string(), display_name: "Keypad *".to_string(), category: "Keypad".to_string() });
    map.insert(0x0056, Keycode { code: 0x0056, name: "KC_KP_MINUS".to_string(), display_name: "Keypad -".to_string(), category: "Keypad".to_string() });
    map.insert(0x0057, Keycode { code: 0x0057, name: "KC_KP_PLUS".to_string(), display_name: "Keypad +".to_string(), category: "Keypad".to_string() });
    map.insert(0x0058, Keycode { code: 0x0058, name: "KC_KP_ENTER".to_string(), display_name: "Keypad Enter".to_string(), category: "Keypad".to_string() });
    map.insert(0x0059, Keycode { code: 0x0059, name: "KC_KP_1".to_string(), display_name: "Keypad 1".to_string(), category: "Keypad".to_string() });
    map.insert(0x005A, Keycode { code: 0x005A, name: "KC_KP_2".to_string(), display_name: "Keypad 2".to_string(), category: "Keypad".to_string() });
    map.insert(0x005B, Keycode { code: 0x005B, name: "KC_KP_3".to_string(), display_name: "Keypad 3".to_string(), category: "Keypad".to_string() });
    map.insert(0x005C, Keycode { code: 0x005C, name: "KC_KP_4".to_string(), display_name: "Keypad 4".to_string(), category: "Keypad".to_string() });
    map.insert(0x005D, Keycode { code: 0x005D, name: "KC_KP_5".to_string(), display_name: "Keypad 5".to_string(), category: "Keypad".to_string() });
    map.insert(0x005E, Keycode { code: 0x005E, name: "KC_KP_6".to_string(), display_name: "Keypad 6".to_string(), category: "Keypad".to_string() });
    map.insert(0x005F, Keycode { code: 0x005F, name: "KC_KP_7".to_string(), display_name: "Keypad 7".to_string(), category: "Keypad".to_string() });
    map.insert(0x0060, Keycode { code: 0x0060, name: "KC_KP_8".to_string(), display_name: "Keypad 8".to_string(), category: "Keypad".to_string() });
    map.insert(0x0061, Keycode { code: 0x0061, name: "KC_KP_9".to_string(), display_name: "Keypad 9".to_string(), category: "Keypad".to_string() });
    map.insert(0x0062, Keycode { code: 0x0062, name: "KC_KP_0".to_string(), display_name: "Keypad 0".to_string(), category: "Keypad".to_string() });
    map.insert(0x0063, Keycode { code: 0x0063, name: "KC_KP_DOT".to_string(), display_name: "Keypad .".to_string(), category: "Keypad".to_string() });
    map.insert(0x0065, Keycode { code: 0x0065, name: "KC_APP".to_string(), display_name: "Menu".to_string(), category: "Modifiers".to_string() });
    
    // Modifiers
    map.insert(0x00E0, Keycode { code: 0x00E0, name: "KC_LEFT_CTRL".to_string(), display_name: "L Ctrl".to_string(), category: "Modifiers".to_string() });
    map.insert(0x00E1, Keycode { code: 0x00E1, name: "KC_LEFT_SHIFT".to_string(), display_name: "L Shift".to_string(), category: "Modifiers".to_string() });
    map.insert(0x00E2, Keycode { code: 0x00E2, name: "KC_LEFT_ALT".to_string(), display_name: "L Alt".to_string(), category: "Modifiers".to_string() });
    map.insert(0x00E3, Keycode { code: 0x00E3, name: "KC_LEFT_GUI".to_string(), display_name: "L GUI".to_string(), category: "Modifiers".to_string() });
    map.insert(0x00E4, Keycode { code: 0x00E4, name: "KC_RIGHT_CTRL".to_string(), display_name: "R Ctrl".to_string(), category: "Modifiers".to_string() });
    map.insert(0x00E5, Keycode { code: 0x00E5, name: "KC_RIGHT_SHIFT".to_string(), display_name: "R Shift".to_string(), category: "Modifiers".to_string() });
    map.insert(0x00E6, Keycode { code: 0x00E6, name: "KC_RIGHT_ALT".to_string(), display_name: "R Alt".to_string(), category: "Modifiers".to_string() });
    map.insert(0x00E7, Keycode { code: 0x00E7, name: "KC_RIGHT_GUI".to_string(), display_name: "R GUI".to_string(), category: "Modifiers".to_string() });
    
    // Note: OP MIDI range is large and encoding is dynamic. We add a couple of helpers and
    // rely on get_keycode_name/find_keycode_by_name to handle dynamic MIDI codes when displayed.
    // Add descriptive placeholders for the OP_MIDI_CC_BASE range start.
    map.insert(0x7E10, Keycode { code: 0x7E10, name: "OP_MIDI_CC_BASE".to_string(), display_name: "MIDI (base)".to_string(), category: "MIDI".to_string() });

    map
}

// Helper to generate a user-friendly display for OP MIDI codes
#[allow(dead_code)]
pub fn describe_op_midi(code: u16) -> Option<String> {
    if code < 0x7E10 { return None; }
    // decode: base + (channel-1)<<11 + controller<<4 + index
    let base = 0x7E10u16;
    let delta = code.wrapping_sub(base);
    let channel = ((delta >> 11) & 0x0F) + 1;
    let controller = (delta >> 4) & 0x7F;
    let index = (delta & 0x0F) as usize;
    if (delta & 0x0F) == 0x0F {
        // Note message
        let note = controller as u8; // note is in controller bits for note encoding
        return Some(format!("MIDI Note ch{} note{}", channel, note));
    }
    // value lookup table similar to firmware
    let values = [0u8,1,7,15,31,43,45,63,64,79,95,111,120,127,50,100];
    let val = values.get(index).copied().unwrap_or(127u8);
    Some(format!("MIDI CC ch{} ctrl{} val{}", channel, controller, val))
}

// Function without tauri::command decorator to avoid duplicate
#[allow(dead_code)]
pub fn get_keycodes() -> HashMap<u16, Keycode> {
    get_keycode_map()
}

#[allow(dead_code)]
pub fn get_keycode_name(code: u16) -> String {
    let map = get_keycode_map();
    match map.get(&code) {
        Some(k) => k.display_name.clone(),
        None => {
            if let Some(desc) = describe_op_midi(code) {
                desc
            } else {
                format!("0x{:04X}", code)
            }
        }
    }
}

#[allow(dead_code)]
pub fn find_keycode_by_name(name: &str) -> Option<u16> {
    let map = get_keycode_map();
    map.iter()
        .find(|(_, keycode)| {
            keycode.name.eq_ignore_ascii_case(name) ||
            keycode.display_name.eq_ignore_ascii_case(name)
        })
        .map(|(code, _)| *code)
}