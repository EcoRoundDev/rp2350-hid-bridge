"""Command builders for the RP2350 HID bridge UART protocol."""

from __future__ import annotations

import json
from typing import Any

LCTRL = 0x01
LSHIFT = 0x02
LALT = 0x04
LGUI = 0x08

_UNSHIFTED: dict[str, int] = {
    **{chr(ord("a") + i): 4 + i for i in range(26)},
    **{str(i): 29 + i for i in range(1, 10)},
    "0": 39,
    "\n": 40,
    "\r": 40,
    "\b": 42,
    "\t": 43,
    " ": 44,
    "-": 45,
    "=": 46,
    "[": 47,
    "]": 48,
    "\\": 49,
    ";": 51,
    "'": 52,
    "`": 53,
    ",": 54,
    ".": 55,
    "/": 56,
}

_SHIFTED: dict[str, str] = {
    **{chr(ord("A") + i): chr(ord("a") + i) for i in range(26)},
    "!": "1",
    "@": "2",
    "#": "3",
    "$": "4",
    "%": "5",
    "^": "6",
    "&": "7",
    "*": "8",
    "(": "9",
    ")": "0",
    "_": "-",
    "+": "=",
    "{": "[",
    "}": "]",
    "|": "\\",
    ":": ";",
    '"': "'",
    "~": "`",
    "<": ",",
    ">": ".",
    "?": "/",
}


def command_line(command: dict[str, Any]) -> str:
    """Serialize one firmware command as a compact JSON line."""
    return json.dumps(command, separators=(",", ":")) + "\n"


def keypress(code: int, modifier: int = 0) -> dict[str, int | str]:
    return {"type": "keypress", "code": code, "modifier": modifier}


def key_hold(code: int, modifier: int = 0) -> dict[str, int | str]:
    return {"type": "key", "code": code, "modifier": modifier}


def combo(keys: list[int], modifier: int = 0) -> dict[str, int | str | list[int]]:
    padded = (keys + [0] * 6)[:6]
    return {"type": "combo", "keys": padded, "modifier": modifier}


def click(button: int = 1) -> dict[str, int | str]:
    return {"type": "click", "button": button}


def mouse_move(x: int, y: int, buttons: int = 0) -> dict[str, int | str]:
    return {"type": "mouse", "x": x, "y": y, "buttons": buttons}


def scroll(wheel: int) -> dict[str, int | str]:
    return {"type": "scroll", "wheel": wheel}


def delay(ms: int) -> dict[str, int | str]:
    return {"type": "delay", "ms": ms}


def char_to_keypress(char: str) -> dict[str, int | str]:
    """Map one ASCII character to a firmware keypress command.

    The mapping follows the standard USB HID keyboard usage table with a US
    keyboard layout. Non-ASCII text should be entered through an OS input method
    on the host rather than this raw HID keycode protocol.
    """
    if len(char) != 1:
        raise ValueError("char_to_keypress expects exactly one character")

    if char in _SHIFTED:
        return keypress(_UNSHIFTED[_SHIFTED[char]], LSHIFT)
    if char in _UNSHIFTED:
        return keypress(_UNSHIFTED[char], 0)

    raise ValueError(f"unsupported character for HID keycode mapping: {char!r}")


def text_to_commands(text: str) -> list[dict[str, int | str]]:
    return [char_to_keypress(char) for char in text]