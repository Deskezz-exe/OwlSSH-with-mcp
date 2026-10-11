"""Record docs/media/demo.gif from the browser build with the mocked backend.

Needs Python with Playwright (`pip install playwright && playwright install chromium`)
and ffmpeg on PATH. Start the UI first with `npm run dev`, then:

    python scripts/record-demo.py

Set CHROME_PATH to use an existing Chrome or Chromium instead of Playwright's own.
"""
import os, shutil, subprocess, tempfile, time
from playwright.sync_api import sync_playwright

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = tempfile.mkdtemp(prefix="owlssh-demo-")
GIF = os.path.join(ROOT, "docs", "media", "demo.gif")
W, H = 1280, 760

CURSOR = """
window.addEventListener('DOMContentLoaded', () => {
  const c = document.createElement('div');
  c.id = '__cursor';
  c.innerHTML = `<svg width="22" height="22" viewBox="0 0 24 24"><path d="M3 2l7.5 19 2.6-7.9L21 10.5z" fill="#fff" stroke="#111" stroke-width="1.5" stroke-linejoin="round"/></svg>`;
  Object.assign(c.style, {position:'fixed', left:'-40px', top:'-40px', zIndex: 2147483647, pointerEvents:'none', transition:'transform .08s', transformOrigin:'3px 2px'});
  document.body.appendChild(c);
  const mv = e => { c.style.left = (e.clientX - 3) + 'px'; c.style.top = (e.clientY - 2) + 'px'; };
  window.addEventListener('pointermove', mv, true);
  window.addEventListener('mousemove', mv, true);
  window.addEventListener('pointerdown', () => c.style.transform = 'scale(.82)', true);
  window.addEventListener('pointerup', () => c.style.transform = '', true);
});
"""

frames = []  # (path, timestamp)
t0 = None


def snap(pg):
    global t0
    path = os.path.join(OUT, f"f{len(frames):04d}.png")
    pg.screenshot(path=path)
    now = time.monotonic()
    if t0 is None:
        t0 = now
    frames.append((path, now))


def hold(pg, ms):
    end = time.monotonic() + ms / 1000
    while time.monotonic() < end:
        snap(pg)


pos = [W // 2, H // 2]


def glide(pg, x, y, steps=10):
    for i in range(1, steps + 1):
        pg.mouse.move(pos[0] + (x - pos[0]) * i / steps, pos[1] + (y - pos[1]) * i / steps)
        if i % 2 == 0:
            snap(pg)
    pos[0], pos[1] = x, y


def center(loc):
    b = loc.bounding_box()
    return b["x"] + b["width"] / 2, b["y"] + b["height"] / 2


def click(pg, loc, after=500):
    x, y = center(loc)
    glide(pg, x, y)
    pg.mouse.down()
    snap(pg)
    pg.mouse.up()
    hold(pg, after)


def type_text(pg, text, delay=55):
    for ch in text:
        pg.keyboard.type(ch)
        pg.wait_for_timeout(delay)
        snap(pg)


with sync_playwright() as p:
    b = p.chromium.launch(executable_path=os.environ.get("CHROME_PATH") or None)
    pg = b.new_page(viewport={"width": W, "height": H}, device_scale_factor=1)
    pg.add_init_script(CURSOR)
    pg.goto("http://localhost:1420")
    pg.wait_for_selector(".host")
    pg.mouse.move(*pos)
    pg.wait_for_timeout(600)
    hold(pg, 1400)

    # 1. Open a terminal from the dashboard card.
    click(pg, pg.get_by_role("button", name="Terminal").first, after=1300)
    pg.locator(".xterm").first.click()
    type_text(pg, "uptime")
    pg.keyboard.press("Enter")
    hold(pg, 700)
    type_text(pg, "docker ps")
    pg.keyboard.press("Enter")
    hold(pg, 1800)

    # 2. Dashboard with live load.
    click(pg, pg.locator(".nav", has_text="Dashboard"), after=600)
    click(pg, pg.get_by_role("button", name="Terminal").nth(1), after=1100)
    click(pg, pg.locator(".nav", has_text="Dashboard"), after=2600)

    # 3. File manager and a drag and drop transfer.
    click(pg, pg.get_by_role("button", name="Files").first, after=1300)
    src = pg.locator("[data-pane=local] .row", has_text="backup.tar.gz").first
    dst = pg.locator("[data-pane=remote]").first
    sx, sy = center(src)
    glide(pg, sx, sy)
    pg.mouse.down()
    snap(pg)
    dx, dy = center(dst)
    glide(pg, dx, dy + 60, steps=16)
    pg.mouse.up()
    hold(pg, 3200)

    # 4. Ctrl+K palette.
    pg.keyboard.press("Control+k")
    hold(pg, 400)
    type_text(pg, "db", delay=120)
    hold(pg, 1200)
    pg.keyboard.press("Escape")
    hold(pg, 200)

    # 5. AI assistants: journal and an approval request.
    click(pg, pg.locator(".nav", has_text="AI assistants"), after=1600)
    pg.evaluate(
        """p => window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {event: 'mcp-confirm', payload: p})""",
        {"id": 7, "client": "Claude Code", "server": "web-prod", "action": "command",
         "detail": "sudo systemctl restart nginx"},
    )
    hold(pg, 2200)
    click(pg, pg.get_by_role("button", name="Allow"), after=1000)
    click(pg, pg.locator(".nav", has_text="Dashboard"), after=1500)
    b.close()

with open(os.path.join(OUT, "list.txt"), "w") as f:
    for i, (path, ts) in enumerate(frames):
        nxt = frames[i + 1][1] if i + 1 < len(frames) else ts + 1.5
        f.write(f"file '{os.path.basename(path)}'\nduration {max(nxt - ts, 0.02):.3f}\n")
    f.write(f"file '{os.path.basename(frames[-1][0])}'\n")
subprocess.run(
    ["ffmpeg", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i", os.path.join(OUT, "list.txt"),
     "-vf", "fps=15,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];"
            "[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
     "-loop", "0", GIF],
    check=True,
)
shutil.rmtree(OUT)
print(f"{len(frames)} frames, {frames[-1][1] - frames[0][1]:.1f} s -> {GIF}")
