#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HidCommand {
    Key {
        code: u8,
        modifier: u8,
    },
    KeyRelease,
    KeyPress {
        code: u8,
        modifier: u8,
    },
    Mouse {
        x: i8,
        y: i8,
        buttons: u8,
    },
    Click {
        button: u8,
    },
    MoveTo {
        x: i32,
        y: i32,
    },
    ClickAt {
        x: i32,
        y: i32,
        button: u8,
        count: u8,
    },
    Scroll {
        wheel: i8,
    },
    Combo {
        keys: [u8; 6],
        modifier: u8,
    },
    Delay {
        ms: u16,
    },
}

pub fn trim_bytes(data: &[u8]) -> &[u8] {
    let start = data.iter().position(|&b| b > b' ').unwrap_or(data.len());
    let end = data
        .iter()
        .rposition(|&b| b > b' ')
        .map(|i| i + 1)
        .unwrap_or(start);
    &data[start..end]
}

pub fn parse_command(data: &[u8]) -> Option<HidCommand> {
    let trimmed = trim_bytes(data);

    #[derive(serde::Deserialize)]
    struct Cmd<'a> {
        #[serde(rename = "type")]
        cmd_type: &'a str,
        #[serde(default)]
        code: u8,
        #[serde(default)]
        modifier: u8,
        #[serde(default)]
        x: i32,
        #[serde(default)]
        y: i32,
        #[serde(default)]
        buttons: u8,
        #[serde(default)]
        button: u8,
        #[serde(default)]
        wheel: i8,
        #[serde(default)]
        keys: [u8; 6],
        #[serde(default)]
        ms: u16,
        #[serde(default = "default_count")]
        count: u8,
    }

    fn default_count() -> u8 {
        1
    }

    let (cmd, _) = serde_json_core::from_slice::<Cmd>(trimmed).ok()?;
    match cmd.cmd_type {
        "key" => Some(HidCommand::Key {
            code: cmd.code,
            modifier: cmd.modifier,
        }),
        "key_release" => Some(HidCommand::KeyRelease),
        "keypress" => Some(HidCommand::KeyPress {
            code: cmd.code,
            modifier: cmd.modifier,
        }),
        "mouse" => Some(HidCommand::Mouse {
            x: cmd.x.clamp(-128, 127) as i8,
            y: cmd.y.clamp(-128, 127) as i8,
            buttons: cmd.buttons,
        }),
        "click" => Some(HidCommand::Click {
            button: default_button(cmd.button),
        }),
        "move_to" => Some(HidCommand::MoveTo { x: cmd.x, y: cmd.y }),
        "click_at" => Some(HidCommand::ClickAt {
            x: cmd.x,
            y: cmd.y,
            button: default_button(cmd.button),
            count: default_count_value(cmd.count),
        }),
        "scroll" => Some(HidCommand::Scroll { wheel: cmd.wheel }),
        "combo" => Some(HidCommand::Combo {
            keys: cmd.keys,
            modifier: cmd.modifier,
        }),
        "delay" => Some(HidCommand::Delay { ms: cmd.ms }),
        _ => None,
    }
}

fn default_button(button: u8) -> u8 {
    if button == 0 {
        1
    } else {
        button
    }
}

fn default_count_value(count: u8) -> u8 {
    if count == 0 {
        1
    } else {
        count
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_command, trim_bytes, HidCommand};

    #[test]
    fn trims_ascii_whitespace_without_allocating() {
        assert_eq!(
            trim_bytes(b" \r\n\t{\"type\":\"delay\",\"ms\":1}\n"),
            br#"{"type":"delay","ms":1}"#
        );
        assert_eq!(trim_bytes(b"   "), b"");
    }

    #[test]
    fn parses_mouse_command_and_clamps_relative_axes() {
        match parse_command(br#"{"type":"mouse","x":200,"y":-200,"buttons":3}"#) {
            Some(HidCommand::Mouse { x, y, buttons }) => {
                assert_eq!(x, 127);
                assert_eq!(y, -128);
                assert_eq!(buttons, 3);
            }
            _ => panic!("expected mouse command"),
        }
    }

    #[test]
    fn parses_click_at_defaults_button_and_count() {
        match parse_command(br#"{"type":"click_at","x":120,"y":240,"button":0,"count":0}"#) {
            Some(HidCommand::ClickAt {
                x,
                y,
                button,
                count,
            }) => {
                assert_eq!(x, 120);
                assert_eq!(y, 240);
                assert_eq!(button, 1);
                assert_eq!(count, 1);
            }
            _ => panic!("expected click_at command"),
        }
    }
}
