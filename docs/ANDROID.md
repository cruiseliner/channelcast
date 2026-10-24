# ChannelCast Android

**Phone APK** (Samsung S25 Ultra) and **Android TV leanback APK** that a custom TV launcher can open (same idea as Guide → TiviMate).

**Status:** Kotlin Compose **stub** that lists channels. **No UniFFI/Rust bridge yet** — headlines are toast placeholders. Real **debug APKs build** on a machine with JDK + Android SDK (verified on the agent box).

## Layout

```
android/
  settings.gradle.kts
  app/
    build.gradle.kts          # flavors: mobile, tv
    src/main/                 # shared Activity, deep links, placeholder channels
    src/mobile/               # LAUNCHER (phone)
    src/tv/                   # LEANBACK_LAUNCHER + leanback feature
  dist/                       # copy of built APKs (*.apk gitignored)
docs/ANDROID.md               # this file
```

Rust workspace (`channelcast-core`, CLI, desktop) is unchanged. Android is a **sibling** Gradle project under `android/`, not a Cargo member.

## Phone vs TV APKs

| Flavor | Gradle assemble | applicationId (debug) | Launcher category | Notes |
|--------|-----------------|------------------------|-------------------|--------|
| **mobile** | `./gradlew :app:assembleMobileDebug` | `com.cruiseliner.channelcast.debug` | `LAUNCHER` | Phone / S25 Ultra sideload |
| **tv** | `./gradlew :app:assembleTvDebug` | `com.cruiseliner.channelcast.tv.debug` | `LEANBACK_LAUNCHER` | 10-foot / D-pad; leanback required |

Release builds drop the `.debug` suffix (`…channelcast` / `…channelcast.tv`).

Both flavors share `MainActivity` and the `channelcast://` deep-link filters.

**Do not** publish fake APKs on GitHub Releases. Attach only APKs that actually built.

## Build steps (this box / any Linux)

Requires **JDK 17+** (OpenJDK 21 works), **Android SDK** (API 34–35 + build-tools).

```bash
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64   # or your JDK
export ANDROID_HOME=$HOME/Android/Sdk
# First time: install cmdline-tools, then:
#   yes | sdkmanager --licenses
#   sdkmanager --install "platform-tools" "platforms;android-35" "build-tools;35.0.0"

cd android
cp local.properties.example local.properties
# sdk.dir=/home/you/Android/Sdk

chmod +x gradlew
./gradlew --no-daemon :app:assembleTvDebug :app:assembleMobileDebug

mkdir -p dist
cp app/build/outputs/apk/tv/debug/app-tv-debug.apk dist/channelcast-tv-debug.apk
cp app/build/outputs/apk/mobile/debug/app-mobile-debug.apk dist/channelcast-mobile-debug.apk
```

Gradle outputs:

- `app/build/outputs/apk/tv/debug/app-tv-debug.apk`
- `app/build/outputs/apk/mobile/debug/app-mobile-debug.apk`

Convenient copies (when present): `android/dist/channelcast-*-debug.apk`

## Install on Android TV (TCL / Google TV)

1. Enable **Developer options** → **USB debugging** (or use wireless adb / a USB stick + file manager that can install APKs).
2. From a PC on the same network (or USB):

```bash
adb connect <tv-ip>:5555    # if wireless
adb install -r android/dist/channelcast-tv-debug.apk
```

3. Confirm package:

```bash
adb shell pm path com.cruiseliner.channelcast.tv.debug
adb shell monkey -p com.cruiseliner.channelcast.tv.debug -c android.intent.category.LEANBACK_LAUNCHER 1
```

4. Or open via deep link:

```bash
adb shell am start -a android.intent.action.VIEW -d "channelcast://open" com.cruiseliner.channelcast.tv.debug
```

LEANBACK_LAUNCHER also puts ChannelCast in the Android TV apps row.

### Phone (S25 Ultra)

```bash
adb install -r android/dist/channelcast-mobile-debug.apk
```

Package: `com.cruiseliner.channelcast.debug`

## TV launcher deep-link (custom pill)

Wire the News / ChannelCast pill like Guide → TiviMate.

### Preferred: custom scheme

```text
channelcast://open
channelcast://channel/tech
channelcast://channel/spacecoast
channelcast://channel/watchman
```

Intent (Kotlin / launcher) — **pin the TV debug package** while using debug APKs:

```kotlin
val intent = Intent(Intent.ACTION_VIEW, Uri.parse("channelcast://open")).apply {
    addCategory(Intent.CATEGORY_DEFAULT)
    setPackage("com.cruiseliner.channelcast.tv.debug") // drop .debug for release
}
startActivity(intent)
```

### Explicit component (no scheme)

```text
com.cruiseliner.channelcast.tv.debug / com.cruiseliner.channelcast.MainActivity
```

```bash
adb shell am start -n com.cruiseliner.channelcast.tv.debug/com.cruiseliner.channelcast.MainActivity
```

## What the stub shows

Channel list only (Top, Tech, Entertainment, Markets, Sports, Pro Wrestling, Watchman, Weather, Local, Space Coast). Selecting a channel shows a Toast — live headlines need the UniFFI bridge (next).

## UniFFI / Rust bridge plan (next)

Goal: call `channelcast-core` from Kotlin without rewriting fetch logic.

1. Add `crates/channelcast-ffi` with UniFFI exposing `list_channels` / `headlines`.
2. Build `libchannelcast.so` for `arm64-v8a` (phones + most TVs) via `cargo-ndk`.
3. Wire Gradle so `assemble*Debug` packs JNI libs.
4. Replace `PlaceholderChannels` + Toast with real FFI; keep Compose thin.

**Why not this pass:** NDK + UniFFI is a separate track. An installable TV APK with channel UI + deep links ships first.

## CI policy

- Default **CI** stays `cargo test --workspace` only.
- Optional workflow [`.github/workflows/android-stub.yml`](../.github/workflows/android-stub.yml) is **`workflow_dispatch` only** — builds mobile + tv debug APKs and uploads artifacts. It must never gate desktop CI on push to `main`.
- Release workflow attaches Android assets **only** when a real APK build succeeds (e.g. tag `v*-android` or a dedicated release).

## Roadmap (Android-specific)

1. **Done:** Gradle project, mobile + tv flavors, stub UI, deep links, local APK build
2. **Done (this pass):** Real `assembleTvDebug` / `assembleMobileDebug` on agent box; dist copies; launcher docs
3. **Next:** Sideload TV APK + wire launcher pill on Adam’s TCL
4. **Next:** UniFFI bridge + NDK `arm64-v8a`
5. **Later:** Compose for TV / leanback rows (10-foot focus), real banner asset
6. **Later:** CI Release assets for APKs on every tagged Android release

## License

Same MIT as the repo root.
