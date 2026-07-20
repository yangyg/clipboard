"""Flood-fill near-white corners of app-icon.png to true transparency."""
from collections import deque
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "app-icon.png"


def is_bg(r: int, g: int, b: int, a: int, tol: int = 28) -> bool:
    return a > 200 and r >= 255 - tol and g >= 255 - tol and b >= 255 - tol


def main() -> None:
    img = Image.open(SRC).convert("RGBA")
    w, h = img.size
    px = img.load()

    seeds = [
        (0, 0),
        (w - 1, 0),
        (0, h - 1),
        (w - 1, h - 1),
        (w // 2, 0),
        (w // 2, h - 1),
        (0, h // 2),
        (w - 1, h // 2),
    ]
    visited = [[False] * h for _ in range(w)]
    q: deque[tuple[int, int]] = deque()
    for x, y in seeds:
        if is_bg(*px[x, y]):
            q.append((x, y))
            visited[x][y] = True

    cleared = 0
    while q:
        x, y = q.popleft()
        r, g, b, _a = px[x, y]
        px[x, y] = (r, g, b, 0)
        cleared += 1
        for nx, ny in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
            if 0 <= nx < w and 0 <= ny < h and not visited[nx][ny] and is_bg(*px[nx, ny]):
                visited[nx][ny] = True
                q.append((nx, ny))

    img.save(SRC, "PNG")
    alpha = list(img.getchannel("A").getdata())
    print(f"size={w}x{h} cleared={cleared}")
    print(
        "transparent=",
        sum(v == 0 for v in alpha),
        "opaque=",
        sum(v == 255 for v in alpha),
        "partial=",
        sum(0 < v < 255 for v in alpha),
    )
    print("corners=", [img.getpixel(c) for c in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)]])
    print("center=", img.getpixel((w // 2, h // 2)))


if __name__ == "__main__":
    main()
