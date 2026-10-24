# ChannelCast Android

Scaffold toward a **phone APK** (Samsung S25 Ultra target) and a later **Android TV** leanback APK that a custom TV launcher can open (same idea as Guide → TiviMate).

**Status (this pass):** Kotlin Compose stub + `mobile` / `tv` product flavors. **No UniFFI/Rust bridge yet.** **No Release APK assets** until a real APK builds on a machine with the Android SDK (or Actions with SDK).

## Layout

```
android/
  settings.gradle.kts
  app/
    build.gradle.kts          # flavors: mobile, tv
    src/main/                 # shared Activity, deep links, placeholder channels
    src/mobile/               # LAUNCHER (phone)
    src/tv/                   # LEANBACK_LAUNCHER + leanback feature
docs/ANDROID.md               # this file
```

Rust workspace (`channelcast-core`, CLI, desktop) is unchanged. Android is a **sibling** Gradle project under `android/`, not a Cargo member.

## Phone vs TV APKs

| Flavor | Gradle assemble | applicationId | Launcher category | Notes |
|--------|-----------------|---------------|-------------------|--------|
| **mobile** | `./gradlew :app:assembleMobileDebug` | `com.cruiseliner.channelcast` (+ `.debug` for debug) | `LAUNCHER` | Phone / S25 Ultra sideload |
| **tv** | `./gradlew :app:assembleTvDebug` | `…channelcast.tv` (+ `.debug`) | `LEANBACK_LAUNCHER` | 10-foot / D-pad; leanback required |

Both flavors share `MainActivity` and the `channelcast://` deep-link filters. TV uses a larger Compose layout later; today both show the same stub channel list.

**Do not** publish fake APKs on GitHub Releases. When an APK actually builds in CI or locally, attach that artifact and document the SHA.

## Build steps (developer machine)

Requires **JDK 17+**, **Android SDK** (API 35), and optionally Android Studio Ladybug+.

```bash
cd android
cp local.properties.example local.properties
# edit sdk.dir=...

# First time: generate wrapper if gradlew is missing or broken
# gradle wrapper --gradle-version 8.11.1

chmod +x gradlew
./gradlew :app:assembleMobileDebug
./gradlew :app:assembleTvDebug
```

Outputs (typical):

- `app/build/outputs/apk/mobile/debug/app-mobile-debug.apk`
- `app/build/outputs/apk/tv/debug/app-tv-debug.apk`

Install phone:

```bash
adb install -r app/build/outputs/apk/mobile/debug/app-mobile-debug.apk
```

Install TV (device/emulator with leanback):

```bash
adb install -r app/build/outputs/apk/tv/debug/app-tv-debug.apk
```

### Blockers on the ChannelCast CI box / agent VM

As of the scaffold commit:

- **No JDK** and **no Android SDK/NDK** on the agent box used for early ChannelCast work
- Therefore **no APK was produced in that environment**
- Desktop/CLI CI (`cargo test`) must stay green and **must not** depend on the Android SDK

## TV launcher deep-link

Custom TV launcher pill (ErsatzTV / Guide-style) can start ChannelCast like TiviMate:

### Preferred: custom scheme

```text
channelcast://open
channelcast://channel/tech
channelcast://channel/spacecoast
channelcast://channel/watchman
```

Intent (Kotlin / launcher):

```kotlin
val intent = Intent(Intent.ACTION_VIEW, Uri.parse("channelcast://open")).apply {
    addCategory(Intent.CATEGORY_DEFAULT)
    // Optional: pin the TV package when both APKs exist
    setPackage("com.cruiseliner.channelcast.tv")
}
startActivity(intent)
```

### Explicit component (no scheme)

```text
com.cruiseliner.channelcast.tv / com.cruiseliner.channelcast.MainActivity
```

Debug builds append `.debug` to the applicationId (`…channelcast.tv.debug`).

LEANBACK_LAUNCHER also makes the TV APK appear in the Android TV apps row once installed.

## UniFFI / Rust bridge plan (next, not this scaffold)

Goal: call `channelcast-core` (`headlines_for_channel`, channel list from `channels.toml`) from Kotlin without rewriting fetch logic.

Pragmatic path:

1. Add a small `crates/channelcast-ffi` crate with [UniFFI](https://mozilla.github.io/uniffi-rs/) `.udl` (or proc-macro) exposing:
   - `list_channels(config_toml: String) -> Vec<ChannelInfo>`
   - `headlines(channel_id: String, limit: UInt32) -> Vec<ArticleInfo>`
2. Build `libchannelcast.so` for `arm64-v8a` (S25 Ultra / most TVs) and optionally `x86_64` (emulator) via `cargo ndk` / `cargo-ndk`.
3. Wire Gradle: `cargo-ndk` task or Mozilla `uniffi-bindgen-gradle` so `assemble*Debug` produces JNI libs under `android/app/src/main/jniLibs/`.
4. Replace `PlaceholderChannels` + Toast stub with real FFI calls; keep Compose UI thin (same pattern as egui desktop).

**Why not in this pass:** UniFFI + NDK needs the Android NDK, `cargo-ndk`, and a successful `aarch64-linux-android` link. Without SDK/NDK on the box, shipping a half-wired FFI crate would be vaporware. Document first; implement when a machine can compile the `.so`.

Alternative if UniFFI friction is high: thin `extern "C"` + JNI hand bindings for two functions only. Prefer UniFFI if the crate stays small.

## CI policy

- Default **CI** job remains `cargo test --workspace` only (desktop deps on Linux).
- Optional workflow **Android stub** is `workflow_dispatch` only and **continues / skips** when the SDK is unavailable — it must never fail desktop CI on push to `main`.
- Release workflow does **not** upload Android assets until an APK build step succeeds.

## Roadmap (Android-specific)

1. **Done (scaffold):** Gradle project, mobile + tv flavors, stub UI, deep links, this doc
2. **Local APK:** Build on a machine with Android Studio; sideload to S25 Ultra
3. **UniFFI bridge:** `channelcast-ffi` + NDK `arm64-v8a`
4. **TV UI:** Compose for TV / leanback rows (10-foot focus), real banner asset
5. **Launcher pill:** Wire `channelcast://open` in the existing TV launcher
6. **CI APK:** Actions job with `android-actions/setup-android` producing artifacts; then (and only then) optional Release assets

## License

Same MIT as the repo root.
