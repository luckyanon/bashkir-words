#!/bin/bash
# Build without signing credentials on a Mac; signing happens on the user's PC.
set -euo pipefail
cd "$(dirname "$0")/.."
sdk="${1:-iphoneos}"
case "$sdk" in
  iphoneos) triple=arm64-apple-ios17.0 ;;
  iphonesimulator) triple=arm64-apple-ios17.0-simulator ;;
  *) echo 'Use iphoneos or iphonesimulator' >&2; exit 1 ;;
esac
test -f ios/Web/index.html || { echo 'Run npm run build:ios first' >&2; exit 1; }
out="ios/build/$sdk"
app="$out/Payload/BashkirWords.app"
mkdir -p "$app"
xcrun --sdk "$sdk" swiftc -swift-version 5 -parse-as-library -O \
  -sdk "$(xcrun --sdk "$sdk" --show-sdk-path)" -target "$triple" \
  ios/Sources/BashkirWords/*.swift -o "$app/BashkirWords"
cp ios/Info.plist "$app/Info.plist"
cp ios/PrivacyInfo.xcprivacy "$app/PrivacyInfo.xcprivacy"
cp ios/Resources/AppIcon.png "$app/AppIcon.png"
ditto ios/Web "$app/Web"
/usr/libexec/PlistBuddy -c "Add :CFBundleSupportedPlatforms array" "$app/Info.plist"
if [ "$sdk" = iphoneos ]; then
  /usr/libexec/PlistBuddy -c 'Add :CFBundleSupportedPlatforms:0 string iPhoneOS' "$app/Info.plist"
else
  /usr/libexec/PlistBuddy -c 'Add :CFBundleSupportedPlatforms:0 string iPhoneSimulator' "$app/Info.plist"
  cp ios/SmokeTest.js "$app/SmokeTest.js"
  codesign --force --sign - "$app"
fi
plutil -lint "$app/Info.plist" "$app/PrivacyInfo.xcprivacy"
if [ "$sdk" = iphoneos ]; then
  (cd "$out" && ditto -c -k --keepParent Payload BashkirWords-unsigned.ipa)
fi
echo "Built: $app"
