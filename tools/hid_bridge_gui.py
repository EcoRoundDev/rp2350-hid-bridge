"""Tkinter test GUI for the RP2350 HID bridge UART protocol."""

from __future__ import annotations

import json
import queue
import threading
import time
import tkinter as tk
from tkinter import messagebox, scrolledtext, ttk
from typing import Any

try:
    import serial
    from serial.tools import list_ports
except ImportError:  # pragma: no cover - depends on local developer machine
    serial = None
    list_ports = None

try:
    from .hid_bridge_commands import (
        LALT,
        LCTRL,
        LGUI,
        click,
        combo,
        command_line,
        key_hold,
        keypress,
        mouse_move,
        scroll,
        text_to_commands,
    )
except ImportError:  # pragma: no cover - used when run as a script
    from hid_bridge_commands import (
        LALT,
        LCTRL,
        LGUI,
        click,
        combo,
        command_line,
        key_hold,
        keypress,
        mouse_move,
        scroll,
        text_to_commands,
    )


class HidBridgeGui:
    def __init__(self, root: tk.Tk) -> None:
        self.root = root
        self.root.title("RP2350 HID Bridge Test")
        self.root.geometry("920x680")
        self.root.minsize(820, 560)

        self.port_var = tk.StringVar()
        self.baud_var = tk.StringVar(value="115200")
        self.step_var = tk.IntVar(value=40)
        self.text_delay_var = tk.IntVar(value=60)

        self._serial: Any | None = None
        self._reader_thread: threading.Thread | None = None
        self._stop_reader = threading.Event()
        self._write_lock = threading.Lock()
        self._events: queue.Queue[tuple[str, str]] = queue.Queue()

        self._build_ui()
        self.refresh_ports()
        self.root.after(100, self._drain_events)
        self.root.protocol("WM_DELETE_WINDOW", self.close)

    def _build_ui(self) -> None:
        outer = ttk.Frame(self.root, padding=12)
        outer.pack(fill=tk.BOTH, expand=True)
        outer.columnconfigure(0, weight=1)
        outer.columnconfigure(1, weight=1)
        outer.rowconfigure(2, weight=1)

        connection = ttk.LabelFrame(outer, text="Connection", padding=10)
        connection.grid(row=0, column=0, columnspan=2, sticky="ew")
        connection.columnconfigure(1, weight=1)

        ttk.Label(connection, text="Port").grid(row=0, column=0, sticky="w")
        self.port_combo = ttk.Combobox(connection, textvariable=self.port_var, width=24)
        self.port_combo.grid(row=0, column=1, sticky="ew", padx=(8, 8))
        ttk.Button(connection, text="Refresh", command=self.refresh_ports).grid(row=0, column=2, padx=(0, 8))

        ttk.Label(connection, text="Baud").grid(row=0, column=3, sticky="w")
        ttk.Entry(connection, textvariable=self.baud_var, width=10).grid(row=0, column=4, padx=(8, 8))
        ttk.Button(connection, text="Open", command=self.open_port).grid(row=0, column=5, padx=(0, 8))
        ttk.Button(connection, text="Close", command=self.close_port).grid(row=0, column=6)

        mouse = ttk.LabelFrame(outer, text="Mouse", padding=10)
        mouse.grid(row=1, column=0, sticky="nsew", pady=(12, 8), padx=(0, 6))
        mouse.columnconfigure(0, weight=1)
        mouse.columnconfigure(1, weight=1)
        mouse.columnconfigure(2, weight=1)

        ttk.Label(mouse, text="Step").grid(row=0, column=0, sticky="w")
        ttk.Spinbox(mouse, from_=1, to=127, textvariable=self.step_var, width=8).grid(
            row=0, column=1, sticky="w"
        )
        ttk.Button(mouse, text="Up", command=lambda: self.send_command(mouse_move(0, -self._step()))).grid(
            row=1, column=1, sticky="ew", pady=4
        )
        ttk.Button(mouse, text="Left", command=lambda: self.send_command(mouse_move(-self._step(), 0))).grid(
            row=2, column=0, sticky="ew", padx=(0, 4)
        )
        ttk.Button(mouse, text="Right", command=lambda: self.send_command(mouse_move(self._step(), 0))).grid(
            row=2, column=2, sticky="ew", padx=(4, 0)
        )
        ttk.Button(mouse, text="Down", command=lambda: self.send_command(mouse_move(0, self._step()))).grid(
            row=3, column=1, sticky="ew", pady=4
        )

        ttk.Button(mouse, text="Left Click", command=lambda: self.send_command(click(1))).grid(
            row=4, column=0, sticky="ew", pady=(12, 4), padx=(0, 4)
        )
        ttk.Button(mouse, text="Right Click", command=lambda: self.send_command(click(2))).grid(
            row=4, column=1, sticky="ew", pady=(12, 4), padx=4
        )
        ttk.Button(mouse, text="Middle Click", command=lambda: self.send_command(click(4))).grid(
            row=4, column=2, sticky="ew", pady=(12, 4), padx=(4, 0)
        )
        ttk.Button(mouse, text="Scroll Up", command=lambda: self.send_command(scroll(5))).grid(
            row=5, column=0, sticky="ew", pady=4, padx=(0, 4)
        )
        ttk.Button(mouse, text="Scroll Down", command=lambda: self.send_command(scroll(-5))).grid(
            row=5, column=1, columnspan=2, sticky="ew", pady=4, padx=(4, 0)
        )

        keyboard = ttk.LabelFrame(outer, text="Keyboard", padding=10)
        keyboard.grid(row=1, column=1, sticky="nsew", pady=(12, 8), padx=(6, 0))
        keyboard.columnconfigure(0, weight=1)
        keyboard.columnconfigure(1, weight=1)
        keyboard.columnconfigure(2, weight=1)

        ttk.Button(keyboard, text="A", command=lambda: self.send_command(keypress(4))).grid(
            row=0, column=0, sticky="ew", padx=(0, 4), pady=4
        )
        ttk.Button(keyboard, text="Space", command=lambda: self.send_command(keypress(44))).grid(
            row=0, column=1, sticky="ew", padx=4, pady=4
        )
        ttk.Button(keyboard, text="Enter", command=lambda: self.send_command(keypress(40))).grid(
            row=0, column=2, sticky="ew", padx=(4, 0), pady=4
        )
        ttk.Button(keyboard, text="Ctrl+C", command=lambda: self.send_command(combo([6], LCTRL))).grid(
            row=1, column=0, sticky="ew", padx=(0, 4), pady=4
        )
        ttk.Button(keyboard, text="Ctrl+V", command=lambda: self.send_command(combo([25], LCTRL))).grid(
            row=1, column=1, sticky="ew", padx=4, pady=4
        )
        ttk.Button(keyboard, text="Release", command=lambda: self.send_command({"type": "key_release"})).grid(
            row=1, column=2, sticky="ew", padx=(4, 0), pady=4
        )
        ttk.Button(keyboard, text="Alt", command=lambda: self.send_command(keypress(0, LALT))).grid(
            row=2, column=0, sticky="ew", padx=(0, 4), pady=4
        )
        ttk.Button(keyboard, text="Win", command=lambda: self.send_command(keypress(0, LGUI))).grid(
            row=2, column=1, sticky="ew", padx=4, pady=4
        )
        ttk.Button(keyboard, text="Alt+Tab", command=lambda: self.send_command(combo([43], LALT))).grid(
            row=2, column=2, sticky="ew", padx=(4, 0), pady=4
        )
        ttk.Button(keyboard, text="Alt+F4", command=lambda: self.send_command(combo([61], LALT))).grid(
            row=3, column=0, sticky="ew", padx=(0, 4), pady=4
        )
        ttk.Button(keyboard, text="Win+D", command=lambda: self.send_command(combo([7], LGUI))).grid(
            row=3, column=1, sticky="ew", padx=4, pady=4
        )
        ttk.Button(keyboard, text="Win+R", command=lambda: self.send_command(combo([21], LGUI))).grid(
            row=3, column=2, sticky="ew", padx=(4, 0), pady=4
        )
        ttk.Button(keyboard, text="Win+E", command=lambda: self.send_command(combo([8], LGUI))).grid(
            row=4, column=0, sticky="ew", padx=(0, 4), pady=4
        )
        ttk.Button(keyboard, text="Alt Down", command=lambda: self.send_command(key_hold(0, LALT))).grid(
            row=4, column=1, sticky="ew", padx=4, pady=4
        )
        ttk.Button(keyboard, text="Win Down", command=lambda: self.send_command(key_hold(0, LGUI))).grid(
            row=4, column=2, sticky="ew", padx=(4, 0), pady=4
        )

        text_header = ttk.Frame(keyboard)
        text_header.grid(row=5, column=0, columnspan=3, sticky="ew", pady=(12, 4))
        text_header.columnconfigure(1, weight=1)
        ttk.Label(text_header, text="Text delay ms").grid(row=0, column=0, sticky="w")
        ttk.Spinbox(text_header, from_=0, to=1000, textvariable=self.text_delay_var, width=8).grid(
            row=0, column=1, sticky="w", padx=(8, 0)
        )
        ttk.Button(text_header, text="Send Text", command=self.send_text).grid(row=0, column=2, sticky="e")

        self.text_input = tk.Text(keyboard, height=5, wrap=tk.WORD)
        self.text_input.grid(row=6, column=0, columnspan=3, sticky="nsew")
        keyboard.rowconfigure(6, weight=1)

        custom = ttk.LabelFrame(outer, text="Custom JSON", padding=10)
        custom.grid(row=2, column=0, sticky="nsew", padx=(0, 6))
        custom.columnconfigure(0, weight=1)
        custom.rowconfigure(0, weight=1)
        self.custom_input = tk.Text(custom, height=4, wrap=tk.NONE)
        self.custom_input.insert("1.0", '{"type":"click","button":1}')
        self.custom_input.grid(row=0, column=0, sticky="nsew")
        ttk.Button(custom, text="Send JSON", command=self.send_custom_json).grid(row=1, column=0, sticky="e", pady=(8, 0))

        log_frame = ttk.LabelFrame(outer, text="Log", padding=10)
        log_frame.grid(row=2, column=1, sticky="nsew", padx=(6, 0))
        log_frame.columnconfigure(0, weight=1)
        log_frame.rowconfigure(0, weight=1)
        self.log = scrolledtext.ScrolledText(log_frame, height=12, state=tk.DISABLED, wrap=tk.WORD)
        self.log.grid(row=0, column=0, sticky="nsew")
        ttk.Button(log_frame, text="Clear", command=self.clear_log).grid(row=1, column=0, sticky="e", pady=(8, 0))

    def refresh_ports(self) -> None:
        if list_ports is None:
            self._log("ERR", "pyserial is not installed. Run: pip install -r requirements-dev.txt")
            return

        ports = [port.device for port in list_ports.comports()]
        self.port_combo["values"] = ports
        if ports and self.port_var.get() not in ports:
            self.port_var.set(ports[0])
        if not ports:
            self._log("INFO", "No serial ports found")

    def open_port(self) -> None:
        if serial is None:
            messagebox.showerror("Missing dependency", "Install pyserial first: pip install -r requirements-dev.txt")
            return

        port = self.port_var.get().strip()
        if not port:
            messagebox.showwarning("Port required", "Select or enter a serial port such as COM3.")
            return

        try:
            baud = int(self.baud_var.get())
        except ValueError:
            messagebox.showwarning("Invalid baud", "Baud rate must be a number.")
            return

        self.close_port()
        try:
            self._serial = serial.Serial(port, baudrate=baud, timeout=0.1, write_timeout=1)
        except Exception as exc:  # pragma: no cover - hardware dependent
            messagebox.showerror("Open failed", str(exc))
            return

        self._stop_reader.clear()
        self._reader_thread = threading.Thread(target=self._reader_loop, daemon=True)
        self._reader_thread.start()
        self._log("INFO", f"Opened {port} @ {baud}")

    def close_port(self) -> None:
        self._stop_reader.set()
        with self._write_lock:
            if self._serial is not None:
                try:
                    self._serial.close()
                except Exception:
                    pass
                self._serial = None
        self._reader_thread = None

    def send_command(self, command: dict[str, Any]) -> None:
        if not self._write_command(command):
            messagebox.showwarning("Not connected", "Open the UART serial port first.")

    def send_text(self) -> None:
        text = self.text_input.get("1.0", "end-1c")
        if not text:
            return

        try:
            commands = text_to_commands(text)
        except ValueError as exc:
            messagebox.showerror("Unsupported text", str(exc))
            return

        delay_ms = self._text_delay()
        thread = threading.Thread(target=self._send_command_sequence, args=(commands, delay_ms), daemon=True)
        thread.start()

    def send_custom_json(self) -> None:
        raw = self.custom_input.get("1.0", "end-1c").strip()
        if not raw:
            return

        try:
            command = json.loads(raw)
        except json.JSONDecodeError as exc:
            messagebox.showerror("Invalid JSON", str(exc))
            return

        if not isinstance(command, dict):
            messagebox.showerror("Invalid command", "The top-level JSON value must be an object.")
            return
        self.send_command(command)

    def clear_log(self) -> None:
        self.log.configure(state=tk.NORMAL)
        self.log.delete("1.0", tk.END)
        self.log.configure(state=tk.DISABLED)

    def close(self) -> None:
        self.close_port()
        self.root.destroy()

    def _reader_loop(self) -> None:
        while not self._stop_reader.is_set():
            with self._write_lock:
                ser = self._serial
            if ser is None:
                return

            try:
                line = ser.readline()
            except Exception as exc:  # pragma: no cover - hardware dependent
                if not self._stop_reader.is_set():
                    self._events.put(("ERR", f"Serial read failed: {exc}"))
                return

            if line:
                decoded = line.decode("utf-8", errors="replace").rstrip()
                self._events.put(("RX", decoded))

    def _send_command_sequence(self, commands: list[dict[str, Any]], delay_ms: int) -> None:
        delay_seconds = delay_ms / 1000
        for command in commands:
            if not self._write_command(command):
                self._events.put(("ERR", "Stopped text send: serial port is not open"))
                return
            if delay_seconds:
                time.sleep(delay_seconds)

    def _write_command(self, command: dict[str, Any]) -> bool:
        line = command_line(command)
        encoded = line.encode("utf-8")
        with self._write_lock:
            ser = self._serial
            if ser is None or not getattr(ser, "is_open", False):
                return False
            try:
                ser.write(encoded)
                ser.flush()
            except Exception as exc:  # pragma: no cover - hardware dependent
                self._events.put(("ERR", f"Serial write failed: {exc}"))
                return False

        self._events.put(("TX", line.rstrip()))
        return True

    def _drain_events(self) -> None:
        while True:
            try:
                kind, message = self._events.get_nowait()
            except queue.Empty:
                break
            self._log(kind, message)
        self.root.after(100, self._drain_events)

    def _log(self, kind: str, message: str) -> None:
        self.log.configure(state=tk.NORMAL)
        self.log.insert(tk.END, f"[{kind}] {message}\n")
        self.log.see(tk.END)
        self.log.configure(state=tk.DISABLED)

    def _step(self) -> int:
        try:
            value = int(self.step_var.get())
        except (tk.TclError, ValueError):
            value = 40
        return max(1, min(127, value))

    def _text_delay(self) -> int:
        try:
            value = int(self.text_delay_var.get())
        except (tk.TclError, ValueError):
            value = 60
        return max(0, min(1000, value))


def main() -> None:
    root = tk.Tk()
    HidBridgeGui(root)
    root.mainloop()


if __name__ == "__main__":
    main()