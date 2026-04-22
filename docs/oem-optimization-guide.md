# OEM Battery Optimization Guide

Ref: `FocusBridge_Development_Plan.md` §18.

Android OEMs kill background services. User must whitelist FocusBridge.

## Per-OEM instructions

### Xiaomi / MIUI
1. Settings → Apps → FocusBridge → Battery saver → "No restrictions"
2. Settings → Apps → FocusBridge → "Autostart" → ON
3. Recents → long-press FocusBridge → Lock
4. Disable "Clear cache" for FocusBridge

### Huawei / EMUI
1. Settings → Apps → FocusBridge → Battery → "App launch" → Manage manually → enable all
2. Settings → Battery → disable "Close excessively power-intensive apps"
3. "Protected apps" → ON (EMUI 5 and earlier)

### Samsung / One UI
1. Settings → Apps → FocusBridge → Battery → "Unrestricted"
2. Battery and device care → Battery → Background usage limits → "Never sleep"
3. Disable auto Device Care for FocusBridge

### OnePlus / OxygenOS
1. Settings → Battery → Advanced optimization → FocusBridge → "Don't optimize"
2. Recent apps → Lock FocusBridge

### Oppo / ColorOS / Realme
1. Settings → Battery → FocusBridge → "Allow background activity"
2. Settings → Apps → FocusBridge → "Allow autostart"
3. Recents → Lock FocusBridge

### Vivo / FuntouchOS
1. Settings → Battery → Background management → FocusBridge → "Allow"
2. Settings → More settings → Permissions → Autostart → FocusBridge → ON

### Stock / Pixel
No action required.
