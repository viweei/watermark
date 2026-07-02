# watermark

A CLI tool for adding watermarks to images.

## Usage

```sh
watermark [text] [options]
```

### Example:

```sh
watermark "仅用于xxx项目投标使用"
```

## Arguments

- `--images, -i` : Specify the input images directory.
- `--size, -s` : Set the font size.
- `--font, -f` : Set the font family name (e.g., "FZCuHeiSongS-B-GB").

## Build

```sh
git clone ...
cd watermark
cargo build --release
```
