# ChannelCast Android (Gradle)

Product flavors: **mobile** (phone) and **tv** (Android TV leanback).

See [docs/ANDROID.md](../docs/ANDROID.md) for build, sideload, and launcher deep-link instructions.

```bash
./gradlew :app:assembleTvDebug
./gradlew :app:assembleMobileDebug
```

APKs: `app/build/outputs/apk/{tv,mobile}/debug/` — also copy to `dist/` (gitignored `*.apk`).
