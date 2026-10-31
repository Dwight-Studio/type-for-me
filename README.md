# Type for Me!

`Type for Me` is a simple utility written in Rust to type the content of your clipboard (instead of pasting it). It
allows you to use your clipboard anywhere, even in virtual machines. It uses the built-in `Remote Desktop` features of
modern Wayland compositors.

## Installation

Download the `.flatpak` file of the [latest release](https://github.com/Dwight-Studio/type-for-me/releases/latest), then
run:

```bash
flatpak install --user type-for-me-<version>-x86_64.flatpak
```

The GNOME runtime is installed from Flathub if needed.

## Building

### Dependencies

*Type For Me* requires `libadwaita` (and all its dependencies, such as `gtk4`) and `libxkbcommon`. Building the
translations requires GNU `gettext` (`msgfmt`).

### Flatpak

The manifest is `rsc/fr.dwightstudio.TypeForMe.json`. It builds the current state of the repository, and needs the
cargo dependencies listed in `rsc/cargo-sources.json`, generated from `Cargo.lock` with
[flatpak-cargo-generator](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo):

```bash
python3 flatpak-cargo-generator.py Cargo.lock -o rsc/cargo-sources.json
flatpak run org.flatpak.Builder --user --install --install-deps-from=flathub --force-clean flatpak-build rsc/fr.dwightstudio.TypeForMe.json
```

### Releases

Releases are automated: when the version in `Cargo.toml` changes on `main`, the CI creates the `vX.Y.Z` tag, builds the
Flatpak bundle and publishes it in a GitHub release. The changelog of the release is the `<release>` block of the
metainfo (`rsc/fr.dwightstudio.TypeForMe.metainfo.xml`), which must be added in the same commit as the version change.

## Translating

Translations are in the `po/` directory and are embedded in the executable at build time. They cover the application,
the desktop file and the AppStream metainfo.

- **Update an existing translation:** run `po/update.sh` (requires GNU gettext >= 0.24) to extract the new strings, then
  edit `po/<language>.po`.
- **Add a language:** add its code to `po/LINGUAS`, run `po/update.sh` and translate the generated `po/<language>.po`.

To test a translation, run the application with the `LANGUAGE` environment variable, e.g. `LANGUAGE=fr cargo run`.
