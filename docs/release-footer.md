
---

### Downloads

| System | File |
|---|---|
| Windows 10/11 installer | `OwlSSH_<version>_x64-setup.exe` |
| Windows portable (no install) | `OwlSSH_<version>_x64_portable.exe` |
| macOS (Apple Silicon and Intel) | `OwlSSH_<version>_universal.dmg` |
| Linux, any distribution | `OwlSSH_<version>_amd64.AppImage` |
| Debian / Ubuntu | `OwlSSH_<version>_amd64.deb` |
| Fedora / RHEL | `OwlSSH-<version>-1.x86_64.rpm` |

The builds are not signed with a paid certificate yet. Windows SmartScreen may say
"Windows protected your PC": click **More info** → **Run anyway**. On macOS, open the
app once with right click → **Open**.

Check a download against `SHA256SUMS.txt`:

```powershell
Get-FileHash .\OwlSSH_<version>_x64-setup.exe -Algorithm SHA256
```

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```
