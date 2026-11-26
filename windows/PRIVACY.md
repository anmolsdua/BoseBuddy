# QC Buddy privacy policy

*Last updated: November 25, 2026*

QC Buddy is a Windows app for Bose QuietComfort headphones. It doesn't
collect, send or sell any personal data. It has no accounts, no analytics, no
ads and no telemetry.

## What stays on your PC

- **Your headphones.** QC Buddy talks to your headphones over your PC's
  Bluetooth connection to read their battery, modes, EQ and connected devices,
  and to change them. None of this leaves your PC.
- **EQ presets** you save, in `eq-presets.json`.
- **A log file** (`QCBuddy.log`, at most about 1 MB including one older
  copy). It records what QC Buddy did, such as connecting, switching modes or
  changing the EQ, and can include your headphones' Bluetooth address and the
  names of devices paired with them. It's there for troubleshooting and is
  never sent anywhere. You can read it with `qcb log`, and delete it at any
  time.

These files are in your user profile's local app data (`QC Buddy` folder).
Uninstalling QC Buddy from the Microsoft Store removes them.

## Network

- **Microsoft Store version:** QC Buddy doesn't connect to the internet. The
  Store handles updates, under
  [Microsoft's privacy statement](https://privacy.microsoft.com/privacystatement).
- **Version downloaded from GitHub:** to check for updates, QC Buddy
  downloads a small file from GitHub when it starts and every 6 hours. The
  request carries nothing about you or your headphones; GitHub sees an
  ordinary download from your IP address, under
  [GitHub's privacy statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).

## Contact

Questions about this policy: open an issue at
<https://github.com/hu13/BoseBuddy/issues>.

QC Buddy is an independent project, not affiliated with or endorsed by Bose
Corporation. Bose and QuietComfort are trademarks of Bose Corporation.
