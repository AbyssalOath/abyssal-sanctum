# Abyssal Sanctum: `startx` starts the Xfce desktop (ADR-0007).
# Desktop settings depend on the connected monitors, so they are written
# now, before Xfce starts and reads them.
/usr/local/bin/sanctum-desktop-setup
exec startxfce4
