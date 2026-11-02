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

winit reports every touch contact as `egui::Event::Touch { force }`, normalised 0 to 1 from the
MotionEvent pressure. [`stylus.rs`](../crates/ui-egui/src/stylus.rs) already reads that for
Windows, so a stylus writes with its pressure and the ink code is unchanged. Check real
readings with the pressure canvas before tuning the curve (see
[ink-and-pressure.md](ink-and-pressure.md)).

## Icon

An adaptive icon: a solid purple background and the koala from `assets/brand/notecraft-512.png`
inside the 66 dp safe zone, in `apps/notecraft-android/res`. Older devices use the plain tile.

## Gaps

Listed in [gaps.md](gaps.md).
