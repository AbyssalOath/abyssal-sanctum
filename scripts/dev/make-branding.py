#!/usr/bin/env python3
"""Regenerate the small branding images in build/profile from assets/.

- build/profile/syslinux/splash.png: the 640x480 BIOS boot menu background.
  The emblem sits at the top; the menu box is drawn below it from y=170.
- build/profile/airootfs/usr/share/pixmaps/abyssal-sanctum.png: a 128x128
  icon for the Xfce panel menu button.

The full-size desktop background is copied from assets/ by the build.
Run this after changing assets/abyssal-sanctum-emblem.png. Needs Pillow
(development machines only; the ISO build does not run it).
"""

from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[2]
EMBLEM = REPO / "assets/abyssal-sanctum-emblem.png"

emblem = Image.open(EMBLEM).convert("RGBA")

# BIOS boot splash.
splash_emblem = emblem.resize((164, 164), Image.LANCZOS)
splash = Image.new("RGB", (640, 480), (4, 6, 12))
splash.paste(splash_emblem, ((640 - 164) // 2, 4), splash_emblem)
splash.save(REPO / "build/profile/syslinux/splash.png", optimize=True)

# Panel icon.
icon_path = REPO / "build/profile/airootfs/usr/share/pixmaps/abyssal-sanctum.png"
icon_path.parent.mkdir(parents=True, exist_ok=True)
emblem.resize((128, 128), Image.LANCZOS).save(icon_path, optimize=True)
