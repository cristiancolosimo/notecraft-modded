# NoteCraft on Android

The same window and engine as the desktop app, started from a NativeActivity
(`apps/notecraft-android`). Notebooks and settings are kept in the app's private folder, so no
storage permission is asked for. The Welcome notebook is installed on the first start.

## Build

```bash
rustup target add aarch64-linux-android
cargo install cargo-apk
cargo xtask android            # release APK; options go to `cargo apk build`
```

`ANDROID_HOME` and `ANDROID_NDK_HOME` are read from the environment; without them the task looks
in `~/Android/Sdk`. The APK is written under `target/release/apk/`. Install it with
`adb install -r target/release/apk/NoteCraft.apk`.

## Pen

The stylus arrives as `egui::Event::Touch { force }`, normalised 0 to 1 from the MotionEvent
pressure. [`stylus.rs`](../crates/ui-egui/src/stylus.rs) already reads that for Windows, so the
ink code is unchanged. Upstream winit gives every contact a force, so `vendor/winit` (a copy
with one change, see its `NOTECRAFT.md`) sends one only for the stylus and eraser tools. A finger
then has no force, `Stylus::pen_contact` is false, and `set_pen_only` makes the ink layer ignore
it: only the pen draws. Check real readings with the pressure canvas before tuning the curve (see
[ink-and-pressure.md](ink-and-pressure.md)).

## Screen

The window is laid out inside the system insets read through JNI (`WindowInsets`: status bar,
navigation bar and keyboard), so no button sits under a bar. The ribbon scrolls sideways. A
header switch keeps the on-screen keyboard from opening; it is remembered. The pen draws by
default (Type brings the caret back), and a finger drag on a drawing page scrolls it.

## Full page view

Full page view (the button at the top right of the page, or Draw, Full Page View) hides the
notebooks and the ribbon and shows a single strip of large targets: back out, undo a stroke,
Type, Select, Lasso, the eraser, and every pen on the rack as a swatch of its colour. A tab on
the header brings the ribbon back.

## Kinetic scrolling

A finger drag on a drawing page or on the ribbon keeps coasting after the finger lifts, slowing
to a stop (`kinetic.rs`); a touch of the glass catches it. The lists beside the page use egui's
own touch scrolling. In Type mode a finger drag over text selects it, as before.

## Performance counter

File, Options, "Show the performance counter" puts a small readout at the top right: frames a
second, the time between frames (mean and worst, rests left out), the process's processor and
memory use and thread count (from `/proc`, so Linux and Android only), the window size and scale,
and the page's ink. It takes no touches, is off by default and is remembered. Use it to see
whether a stutter is a slow frame (a low fps and a high worst time) or something else.

## Endless page

A page scrolls both ways and has no end down or to the right. It is as wide as its column until
ink comes within about 120 points of the right edge; from then on it is half a screen wider than
the furthest ink, so there is always room to keep writing, and it grows the same way downward.
Drag a finger (or use the wheel, shift and wheel, or the scroll bars) to reach it. The ink file
declares a sheet at least as big as its ink. Not built: room above and to the left of the page's
corner (it would mean moving everything on the page).

## Sync

File, Sync keeps the notebooks folder in a Git repository on a server (see [sync.md](sync.md)).
It uses libgit2 with OpenSSL built from source, so building for Android needs `perl` with the
`Time::Piece` module (Fedora: `perl-Time-Piece`). The app asks for the INTERNET permission.

## Export

File, Export writes a notebook as a ZIP. There is no save dialog on Android yet, so the ZIP goes
to `Android/data/com.alientechsoftware.notecraft/files/Exports` on the tablet's storage, and the
status bar says where. Copy it off with
`adb pull /sdcard/Android/data/com.alientechsoftware.notecraft/files/Exports`.

## Icon

An adaptive icon: a solid purple background and the koala from `assets/brand/notecraft-512.png`
inside the 66 dp safe zone, in `apps/notecraft-android/res`. Older devices use the plain tile.

## Gaps

Listed in [gaps.md](gaps.md).
