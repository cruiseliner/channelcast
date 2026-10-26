# ChannelCast Android

**Phone APK** (Samsung S25 Ultra) and **Android TV leanback APK** that a custom TV launcher can open (same idea as Guide → TiviMate).

**Status (v0.1.17):** UniFFI bridge live. Default channel is **top**. Phone: dark Material 3 hero/photo cards (Coil), chips, pull-to-refresh, Custom Tabs. Story images via MediaRSS/og:image. TV: leanback focus UI + in-app WebView. Sports removed. Space Coast + enriched excerpts in core.

## Layout

```
android/
  scripts/build-native.sh     # cargo-ndk → jniLibs (arm64-v8a + armeabi-v7a)
  app/src/main/
    jniLibs/                  # *.so gitignored; built by build-native.sh
    java/com/.../MainActivity.kt
    java/uniffi/channelcast_ffi/  # generated UniFFI Kotlin (committed)
  dist/                       # local APK copies (*.apk gitignored)
crates/channelcast-ffi/       # UniFFI cdylib + list_channels / fetch_headlines
docs/ANDROID.md
```

## Phone vs TV APKs

| Flavor | Gradle assemble | applicationId (debug) | Launcher category |
|--------|-----------------|------------------------|-------------------|
| **mobile** | `./gradlew :app:assembleMobileDebug` | `com.cruiseliner.channelcast.debug` | `LAUNCHER` |
| **tv** | `./gradlew :app:assembleTvDebug` | `com.cruiseliner.channelcast.tv.debug` | `LEANBACK_LAUNCHER` |

Release builds drop the `.debug` suffix. Both share `MainActivity` and `channelcast://` deep links.

## Build (JDK 17+ + Android SDK + NDK)

```bash
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
export ANDROID_HOME=$HOME/Android/Sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.0.12077973

# 1) Rust → .so (arm64-v8a required; armeabi-v7a also built)
./android/scripts/build-native.sh

# 2) Kotlin APKs
cd android
cp local.properties.example local.properties   # first time
chmod +x gradlew
./gradlew --no-daemon :app:assembleTvDebug :app:assembleMobileDebug

mkdir -p dist
cp app/build/outputs/apk/tv/debug/app-tv-debug.apk dist/channelcast-tv-debug.apk
cp app/build/outputs/apk/mobile/debug/app-mobile-debug.apk dist/channelcast-mobile-debug.apk
```

Native libs are **not** committed (large). APKs on [GitHub Releases](https://github.com/cruiseliner/channelcast/releases) already embed `libchannelcast_ffi.so`.

### Regenerate UniFFI Kotlin (after FFI API changes)

```bash
cargo build -p channelcast-ffi --release
cargo run -p channelcast-ffi --features bindgen --bin uniffi-bindgen -- generate \
  --library target/release/libchannelcast_ffi.so \
  --language kotlin --out-dir android/app/src/main/java --no-format
```

### Prove the bridge without a device

```bash
cargo test -p channelcast-ffi fetch_watchman_smoke -- --nocapture
```

## Install on Android TV (TCL / Google TV)

1. Enable **Developer options** → **USB debugging** (or wireless adb).
2. Sideload the TV APK from the latest `v*-android` release:

```bash
adb connect <tv-ip>:5555
adb install -r channelcast-tv-debug.apk
adb shell am start -a android.intent.action.VIEW \
  -d "channelcast://open" com.cruiseliner.channelcast.tv.debug
# or open Watchman directly:
adb shell am start -a android.intent.action.VIEW \
  -d "channelcast://channel/watchman" com.cruiseliner.channelcast.tv.debug
```

Package: `com.cruiseliner.channelcast.tv.debug`

### Phone (S25 Ultra)

Always prefer the newest `v*-android` release that lists **both** `channelcast-mobile-debug.apk` and `channelcast-tv-debug.apk`.

```bash
adb install -r channelcast-mobile-debug.apk
```

Package: `com.cruiseliner.channelcast.debug`

## TV launcher deep-link (custom pill)

```text
channelcast://open
channelcast://channel/tech
channelcast://channel/spacecoast
channelcast://channel/watchman
```

```kotlin
val intent = Intent(Intent.ACTION_VIEW, Uri.parse("channelcast://open")).apply {
    addCategory(Intent.CATEGORY_DEFAULT)
    setPackage("com.cruiseliner.channelcast.tv.debug") // drop .debug for release
}
startActivity(intent)
```

Explicit component:

```text
com.cruiseliner.channelcast.tv.debug / com.cruiseliner.channelcast.MainActivity
```

## UniFFI API

| Rust / Kotlin | Purpose |
|---------------|---------|
| `list_channels(include_disabled)` / `listChannels` | Enabled channels from built-in `channels.toml` |
| `fetch_headlines(id, limit)` / `fetchHeadlines` | Live fetch + keyword filter + rank (blocking; call on `Dispatchers.IO`) |

ABIs: **arm64-v8a** (S25 Ultra + modern Android TV) and **armeabi-v7a**.

## CI policy

- Default CI: `cargo test --workspace` (includes `channelcast-ffi` unit + optional network smoke).
- [`.github/workflows/android-stub.yml`](../.github/workflows/android-stub.yml) remains **`workflow_dispatch`** — needs NDK step before it can assemble with the bridge (update when CI Android builds are wanted).
- Publish APKs on tags like `v0.1.7-android` as Release assets (do not commit `.so` / `.apk` to git).

## Roadmap

1. **Done:** Gradle mobile + tv flavors, deep links, stub UI
2. **Done:** Real debug APKs
3. **Done (v0.1.7):** UniFFI + NDK `arm64-v8a` / `armeabi-v7a`, live headlines in Compose
4. **Next:** Sideload TV APK + wire launcher News pill on Adam’s TCL
5. **Later:** Compose for TV / leanback rows, release signing, CI APK job with NDK

## License

Same MIT as the repo root.
