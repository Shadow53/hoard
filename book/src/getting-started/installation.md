# Installation

This page lists the supported methods of installing `hoard`.

## GitHub Releases (Recommended)

The recommended method of installation is by downloading a prebuilt executable from the
[latest release](https://github.com/Shadow53/hoard/releases/latest) on GitHub. All files are named after the type
of system that it can run on. There are many options available, but most people will want one of the following:

- Windows: ` hoard-x86_64-pc-windows-msvc.zip`
- Mac:
  - Intel (Older): `hoard-x86_64-apple-darwin.tar.gz`
  - Apple M1 (Newer): `hoard-aarch64-apple-darwin.tar.gz`
- Linux:
  - GNU libc (most distributions, dynamically linked): `hoard-x86_64-unknown-linux-gnu.tar.gz`
  - MUSL libc (Alpine Linux, statically linked): `hoard-x86_64-unknown-linux-musl.tar.gz`
- Modern Android phones: `hoard-aarch64-linux-android.tar.gz`
  - (you may also want to install [Termux](https://termux.com/))

### I've downloaded it, how do I install?

There is no installer for these files -- Hoard is a self-contained executable. Just extract the archive (`.zip`, `.tar.gz`),
rename the extracted file to `hoard`, and add it to your `$PATH`.

What is the `$PATH`? It is a list of directories that the computer searches for programs when you enter a command on the
command line. The process of adding a program to your path is beyond the scope of this guide. Instead, try searching
online for "add executable to PATH `os name`" where `os name` is the operating system you are running: "Windows", "Mac",
"Linux", "Ubuntu", etc.

### GNU or MUSL?

Most Linux distributions use GNU libc, so it should be safe to use that version. Because the libc is dynamically linked,
security updates to the libc are automatically applied to Hoard and all other programs that use it. This also means that
there is a small chance of a libc update breaking programs linked against older versions, though.

MUSL libc is statically compiled into the executable itself, so it can run more or less standalone, without fear of libc
breakages. The downside of this is that you do not receive bugfix updates to libc until a newer version of Hoard is
released.

You get to decide which one is best to use. For most users (Ubuntu/Debian, etc.), I suggest the GNU libc. For users of
fast-moving rolling release systems and systems without GNU libc (Arch, Alpine, etc.), I suggest MUSL.

## Cargo

If you have `cargo` and the Rust toolchain installed, you can install `hoard` with the following command:

```bash
cargo install hoard
```
