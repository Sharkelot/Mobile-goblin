#!/usr/bin/env bash
set -euo pipefail
trap 'adb logcat -d -t 2000 > "$RUNNER_TEMP/mobile-goblin-logcat.txt" || true; adb exec-out screencap -p > "$RUNNER_TEMP/mobile-goblin-launch.png" || true' EXIT

apk_path="$RUNNER_TEMP/Mobile-Goblin-emulator.apk"
test -s "$apk_path"
adb install -r "$apk_path"
adb shell pm path com.sharkelot.mobilegoblin
adb logcat -c
adb shell am start -W -n com.sharkelot.mobilegoblin/android.app.NativeActivity
sleep 8
adb shell pidof com.sharkelot.mobilegoblin
adb logcat -d -t 2000 > "$RUNNER_TEMP/mobile-goblin-logcat.txt"
adb exec-out screencap -p > "$RUNNER_TEMP/mobile-goblin-launch.png"
test -s "$RUNNER_TEMP/mobile-goblin-launch.png"
if rg 'FATAL EXCEPTION|Fatal signal' "$RUNNER_TEMP/mobile-goblin-logcat.txt"; then
  exit 1
fi
