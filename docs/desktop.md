# Desktop

Tide uses KDE Plasma as its primary desktop, with XFCE and a minimal Openbox profile defined in
`config/tide.yaml`. None of these have been visually validated in a booted Tide image yet.

## Profile selection

```bash
./build.sh --desktop kde all
./build.sh --desktop xfce all
./build.sh --desktop minimal all
./build.sh --desktop none all
```

The default is `kde`. Profiles are declarative package lists; the builder does not fork Plasma.

| Profile | Base | Notes |
|---|---|---|
| `kde` | `kde-plasma-desktop` + Plasma workspace, SDDM, Breeze, PipeWire, Flatpak | Primary; receives Tide skel defaults |
| `xfce` | `task-xfce-desktop`, LightDM | Package profile only; no Tide XFCE skel yet |
| `minimal` | Xorg, Openbox, LightDM, xterm | Small graphical environment |
| `none` | No desktop packages | Useful for testing the base system |

## Identity assets

| Asset | Staged path | Purpose |
|---|---|---|
| `assets/branding/os-release` | `/etc/os-release` | `ID=tide`, `ID_LIKE=debian`, `TIDE_FOUNDATION` |
| `assets/branding/issue`, `issue.net` | `/etc/`, `/etc/` | Console identity |
| `assets/branding/logo/tide-logo.svg` | `/usr/share/pixmaps/`, `/usr/share/tide/branding/` | Logo |
| `assets/branding/wallpaper/tide-default.svg` | `/usr/share/backgrounds/tide/` | 2560×1440 wallpaper |
| `assets/desktop/kde/color-schemes/Tide.colors` | `/usr/share/color-schemes/` | Dark glass/teal palette |
| `assets/desktop/plasma/look-and-feel/org.tide.desktop` | `/usr/share/plasma/look-and-feel/` | Look-and-feel metadata + defaults |
| `assets/desktop/kde/skel/.config/kdeglobals` | `/etc/skel/.config/` | New-user defaults |

The design direction is a dark abyss background with teal/cyan accents. It reuses Breeze and Plasma
components rather than shipping a custom shell.

## KDE specifics

The look-and-feel package (`org.tide.desktop`) sets:

- `ColorScheme=Tide`
- `LookAndFeelPackage=org.tide.desktop`
- `WidgetStyle=Breeze`
- `Icons=breeze-dark`
- wallpaper `/usr/share/backgrounds/tide/tide-default.svg`

The `metadata.json` follows the Plasma look-and-feel format. It has not been loaded by a running
Plasma session in this repository, so treat it as structurally valid, not visually verified.

## First boot

A oneshot service runs once:

- Unit: `/usr/lib/systemd/system/tide-firstboot.service`
- Script: `/usr/lib/tide/tide-firstboot.sh`
- Marker: `/var/lib/tide/firstboot-done`
- Report: `/var/log/tide/firstboot-doctor.json`

The service runs `tidectl doctor --json`, records the result, and exits. It does not install software,
change configuration, or make network calls. If `tidectl` is missing, the report records that fact.

The marker is removed by `scripts/rootfs-config.sh` during image configuration so the service runs on the
installed system's first boot.

## Gaps

- No first-boot setup wizard; the `tide` user exists with a locked password.
- No Tide settings application. `tidectl --json` is the intended data source.
- No XFCE-specific Tide defaults.
- No accessibility or low-end hardware preset.
- No validated screenshot or theme preview.
