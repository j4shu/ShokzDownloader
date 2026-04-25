https://github.com/user-attachments/assets/f0fb84b7-7c46-4379-ab5d-da61ab13f7af

# ShokzDownloader

A simple command-line tool for transferring music files to a Shokz device (i.e. the [Shokz OpenSwim](https://shokz.com/products/openswim)).

Paired with something like [stacher.io](https://stacher.io/), this can be a way to easily download music to your Shokz device.

This is a Rust rewrite of [shokz_downloader](https://github.com/j4shu/shokz_downloader), which was in Python.

## Why?

I usually listen to albums from start to finish. These headphones play music in the order the files were written to the device. When dragging and dropping files onto the device, the order they "finish" is random, resulting in a different track order when playing them on the device.

See [How to list the track order on OpenSwim](https://intl.help.shokz.com/s/article/How-to-list-the-track-order-on-OpenSwim-formerly-Xtrainerz-17) for more info. I'm aware of [this article](https://en.help.shokz.com/s/get-article?urlName=how-to-list-tracks-order-EN). However, even after trying the steps in it, the track order sometimes still ends up random.

The goal of this program is to ensure songs are transferred to the device in a deterministic order.

## Usage

```
cargo run
```

The program guides you through three steps:

1. **Select device** - Select your Shokz device after it's plugged in.
2. **Select folder** - Select a folder from your Desktop containing your music files.
3. **Confirm** - Review the file list and confirm the transfer.

## Assumptions

- Music files should start with a track number prefix. This is to ensure the order they are transferred to the device is deterministic. For example, `1 - Song Name.mp3`, `2 - Song Name.mp3`, etc.
- The selected folder should not contain any subfolders. Non-music files will be silently skipped during the transfer process.
