# Type for Me!

`Type for Me` is a simple utility to type the content of your clipboard (instead of pasting it). It allows you to use
your clipboard anywhere, even in virtual machines. It uses the built-in `Remote Desktop` features of modern Wayland
compositor.

## Building

### Dependencies

*Type For Me* requires `libadwaita` (and all its dependencies, such as `gtk4`) and `libxkbcommon`.

On DNF distributions (Fedora, RHEL, ...) you can install the dependencies with :

```bash
sudo dnf install libadwaita-devel libxkbcommon-devel
```