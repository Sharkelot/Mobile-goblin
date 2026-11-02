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
adb exec-out screencap -p > "$RUNNER_TEMP/mobile-goblin-launch.png"
test -s "$RUNNER_TEMP/mobile-goblin-launch.png"
screen_size="$(adb shell wm size | sed -n 's/.*Physical size: \([0-9]*\)x\([0-9]*\).*/\1 \2/p')"
read -r screen_width screen_height <<< "$screen_size"
test -n "$screen_width"
adb shell input tap "$((screen_width / 2))" "$((screen_height * 69 / 100))"
sleep 2
adb exec-out screencap -p > "$RUNNER_TEMP/mobile-goblin-password-focus.png"
test -s "$RUNNER_TEMP/mobile-goblin-password-focus.png"
adb shell pidof com.sharkelot.mobilegoblin
adb logcat -d -t 2000 > "$RUNNER_TEMP/mobile-goblin-logcat.txt"
if rg 'FATAL EXCEPTION|Fatal signal' "$RUNNER_TEMP/mobile-goblin-logcat.txt"; then
  exit 1
fi
