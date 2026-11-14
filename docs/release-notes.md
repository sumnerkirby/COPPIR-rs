## Download

| System | File |
|---|---|
| macOS 11 or later (Apple Silicon and Intel) | `COPPIR_<version>_universal.dmg` |
| Windows 10 or 11 | `COPPIR_<version>_x64-setup.exe` |
| Ubuntu, Debian | `coppir_<version>_amd64.deb` |
| Any other Linux | `coppir_<version>_x86_64.AppImage` |

Players and observers install nothing. They join from a browser on the same network.

## First launch

These builds are not signed with a paid certificate, so each system asks once.

- **macOS**: open the disk image and drag COPPIR to Applications. Open it. When macOS says it cannot check the app, go to **System Settings → Privacy & Security** and click **Open Anyway**. On macOS 14 and earlier you can instead right-click the app and choose **Open**.
- **Windows**: SmartScreen shows "Windows protected your PC". Click **More info**, then **Run anyway**. The installer needs no administrator rights.
- **AppImage**: make it executable (`chmod +x coppir_*.AppImage`), then run it.
- **.deb**: `sudo apt install ./coppir_*_amd64.deb`.

`SHA256SUMS.txt` lists each file's checksum.
