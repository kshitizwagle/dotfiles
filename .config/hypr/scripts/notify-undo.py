#!/usr/bin/env python3
# Undo toast for a stashed window: sends the "Minimized to stash" notification with a
# Restore default action and brings the window back when it is invoked (left-click on the
# toast, or `makoctl invoke <id> default`). Right-click closes the toast without restoring
# (mako criteria binding). The toast stays out of mako's history: the helper closes it
# quietly once the 10s undo window passes, and both the restore and close paths go through
# makoctl's --no-history (the DBus CloseNotification method would add a history entry).
# Mako supports actions, but notify-send's capability probe wrongly reports otherwise,
# so the notification goes out over DBus.
# Usage: notify-undo.py <window-address> [title]
import subprocess
import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

DEST = "org.freedesktop.Notifications"
PATH = "/org/freedesktop/Notifications"
IFACE = "org.freedesktop.Notifications"

UNDO_WINDOW_MS = 10_000
# Backstop for the case where this helper dies without closing the toast: mako expires it
# a little later instead of leaving it on screen forever.
BACKSTOP_MS = 15_000


def dismiss_quietly(notif_id: int) -> None:
    subprocess.run(["makoctl", "dismiss", "-n", str(notif_id), "--no-history"], capture_output=True)


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
                (
                    "Stash",
                    0,
                    "",
                    "Minimized to stash",
                    f"{title} · left-click: restore · right-click: close",
                    ["default", "Restore"],
                    {},
                    BACKSTOP_MS,
                ),
            ),
            None, Gio.DBusCallFlags.NONE, -1, None,
        )
    except GLib.Error:
        sys.exit(0)
    notif_id = reply.unpack()[0]

    loop = GLib.MainLoop()

    def on_deadline() -> bool:
        dismiss_quietly(notif_id)
        loop.quit()
        return GLib.SOURCE_REMOVE

    def on_signal(_conn, _sender, _path, _iface, signal, params) -> None:
        if signal == "ActionInvoked":
            sid, action = params.unpack()
            if sid == notif_id and action == "default":
                subprocess.Popen([f"{GLib.get_home_dir()}/.config/hypr/scripts/unminimize.sh", address])
                dismiss_quietly(notif_id)
                loop.quit()
        elif signal == "NotificationClosed":
            if params.unpack()[0] == notif_id:
                loop.quit()

    bus.signal_subscribe(DEST, IFACE, None, PATH, None, Gio.DBusSignalFlags.NONE, on_signal)
    GLib.timeout_add(UNDO_WINDOW_MS, on_deadline)
    loop.run()


if __name__ == "__main__":
    main()
