#!/usr/bin/env python3
"""Customises the Android project made by `tauri android init` (src-tauri/gen/android).

- MainActivity: USB MIDI (OTG) + LED out, storage permission, full screen
- AndroidManifest: permissions, MIDI feature, landscape
- launcher icons from src-tauri/icons/android
"""
import glob, os, re, shutil, sys

root = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
gen = os.path.join(root, 'src-tauri', 'gen', 'android')
if not os.path.isdir(gen):
    sys.exit('gen/android not found — run `npx tauri android init --ci` first')

acts = glob.glob(os.path.join(gen, 'app', 'src', 'main', 'java', '**', 'MainActivity.kt'), recursive=True)
assert acts, 'MainActivity.kt not found'
act = acts[0]
pkg = re.search(r'^package\s+([\w.]+)', open(act, encoding='utf-8').read(), re.M).group(1)
src = open(os.path.join(root, 'tools', 'android', 'MainActivity.kt'), encoding='utf-8').read().replace('__PACKAGE__', pkg)
open(act, 'w', encoding='utf-8').write(src)
print('MainActivity →', act, '(package', pkg + ')')

man = os.path.join(gen, 'app', 'src', 'main', 'AndroidManifest.xml')
m = open(man, encoding='utf-8').read()
perms = '''    <uses-permission android:name="android.permission.READ_MEDIA_AUDIO" />
    <uses-permission android:name="android.permission.READ_EXTERNAL_STORAGE" android:maxSdkVersion="32" />
    <uses-feature android:name="android.software.midi" android:required="false" />
    <uses-feature android:name="android.hardware.usb.host" android:required="false" />
'''
if 'READ_MEDIA_AUDIO' not in m:
    m = re.sub(r'(<manifest[^>]*>\s*\n)', lambda x: x.group(1) + perms, m, count=1)
if 'screenOrientation' not in m:
    m = m.replace('android:name=".MainActivity"', 'android:name=".MainActivity"\n            android:screenOrientation="sensorLandscape"', 1)
open(man, 'w', encoding='utf-8').write(m)
print('AndroidManifest patched')

res = os.path.join(gen, 'app', 'src', 'main', 'res')
icons = os.path.join(root, 'src-tauri', 'icons', 'android')
for d in os.listdir(icons):
    dst = os.path.join(res, d)
    os.makedirs(dst, exist_ok=True)
    for f in os.listdir(os.path.join(icons, d)):
        shutil.copy(os.path.join(icons, d, f), os.path.join(dst, f))
print('icons copied')
