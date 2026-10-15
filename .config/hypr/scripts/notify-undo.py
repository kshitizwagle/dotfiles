#!/usr/bin/env python3
# Undo toast for a stashed window: sends the "Minimized to stash" notification with a
# Restore default action and brings the window back when it is invoked (left-click on the
# toast, or `makoctl invoke <id> default`). Mako supports actions, but notify-send's
# capability probe wrongly reports otherwise, so the notification goes out over DBus.
# Usage: notify-undo.py <window-address> [title]
import subprocess
import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

DEST = "org.freedesktop.Notifications"
PATH = "/org/freedesktop/Notifications"
IFACE = "org.freedesktop.Notifications"


def main() -> None:
    if len(sys.argv) < 2:
        sys.exit(2)
    address = sys.argv[1]
    title = sys.argv[2] if len(sys.argv) > 2 else "window"

    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    try:
        reply = bus.call_sync(
            DEST, PATH, IFACE, "Notify",
            GLib.Variant(
                "(susssasa{sv}i)",
                ("Stash", 0, "", "Minimized to stash", f"{title} · click to restore", ["default", "Restore"], {}, 10000),
            ),
            None, Gio.DBusCallFlags.NONE, -1, None,
        )
    except GLib.Error:
        sys.exit(0)
    notif_id = reply.unpack()[0]

    loop = GLib.MainLoop()

    def on_signal(_conn, _sender, _path, _iface, signal, params) -> None:
        if signal == "ActionInvoked":
            sid, action = params.unpack()
            if sid == notif_id and action == "default":
                subprocess.Popen([f"{GLib.get_home_dir()}/.config/hypr/scripts/unminimize.sh", address])
                loop.quit()
        elif signal == "NotificationClosed" and params.unpack()[0] == notif_id:
            loop.quit()

    bus.signal_subscribe(DEST, IFACE, None, PATH, None, Gio.DBusSignalFlags.NONE, on_signal)
    GLib.timeout_add_seconds(20, lambda: (loop.quit(), GLib.SOURCE_REMOVE)[1])  # safety net
    loop.run()


if __name__ == "__main__":
    main()
