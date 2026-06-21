import json
import unittest

from tools.hid_bridge_commands import (
    LALT,
    LCTRL,
    LGUI,
    LSHIFT,
    char_to_keypress,
    combo,
    command_line,
    key_hold,
    keypress,
    text_to_commands,
)


class HidBridgeCommandTests(unittest.TestCase):
    def test_command_line_compacts_json_and_adds_newline(self):
        self.assertEqual(command_line({"type": "click", "button": 1}), '{"type":"click","button":1}\n')

    def test_char_to_keypress_maps_letters_digits_and_shifted_symbols(self):
        self.assertEqual(char_to_keypress("a"), {"type": "keypress", "code": 4, "modifier": 0})
        self.assertEqual(char_to_keypress("A"), {"type": "keypress", "code": 4, "modifier": LSHIFT})
        self.assertEqual(char_to_keypress("1"), {"type": "keypress", "code": 30, "modifier": 0})
        self.assertEqual(char_to_keypress("!"), {"type": "keypress", "code": 30, "modifier": LSHIFT})
        self.assertEqual(char_to_keypress(" "), {"type": "keypress", "code": 44, "modifier": 0})
        self.assertEqual(char_to_keypress("\n"), {"type": "keypress", "code": 40, "modifier": 0})

    def test_modifier_only_keypresses_are_supported(self):
        self.assertEqual(keypress(0, LALT), {"type": "keypress", "code": 0, "modifier": LALT})
        self.assertEqual(keypress(0, LGUI), {"type": "keypress", "code": 0, "modifier": LGUI})
        self.assertEqual(key_hold(0, LALT), {"type": "key", "code": 0, "modifier": LALT})

    def test_common_shortcuts_are_combo_commands(self):
        self.assertEqual(combo([43], LALT), {"type": "combo", "keys": [43, 0, 0, 0, 0, 0], "modifier": LALT})
        self.assertEqual(combo([7], LGUI), {"type": "combo", "keys": [7, 0, 0, 0, 0, 0], "modifier": LGUI})
        self.assertEqual(combo([6], LCTRL), {"type": "combo", "keys": [6, 0, 0, 0, 0, 0], "modifier": LCTRL})

    def test_text_to_commands_preserves_order(self):
        commands = text_to_commands("Az!\n")
        self.assertEqual(
            commands,
            [
                {"type": "keypress", "code": 4, "modifier": LSHIFT},
                {"type": "keypress", "code": 29, "modifier": 0},
                {"type": "keypress", "code": 30, "modifier": LSHIFT},
                {"type": "keypress", "code": 40, "modifier": 0},
            ],
        )

    def test_unsupported_character_reports_the_character(self):
        with self.assertRaisesRegex(ValueError, "unsupported character"):
            char_to_keypress("\u4e2d")

    def test_every_command_is_json_serializable(self):
        for command in text_to_commands("Hello, RP2350!"):
            json.dumps(command)
        for command in [keypress(0, LGUI), key_hold(0, LALT), combo([43], LALT)]:
            json.dumps(command)


if __name__ == "__main__":
    unittest.main()