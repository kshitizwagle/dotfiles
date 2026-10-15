#!/usr/bin/env bash
# set-avatar.sh <image>: centre-crop to a square, scale to 512 px and save as ~/.face
# (the standard avatar path; hyprlock shows it in a circle). The image itself never enters the repo.
src=${1:?usage: set-avatar.sh <image>}
python3 - "$src" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
s = min(im.size); l = (im.width - s) // 2; t = (im.height - s) // 2
im.crop((l, t, l + s, t + s)).resize((512, 512), Image.LANCZOS).save(
    __import__("os").path.expanduser("~/.face"), format="PNG")
print("saved ~/.face")
PY
