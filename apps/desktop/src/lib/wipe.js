// What a seller should do before handing a computer over, per system.

export const WIPE_GUIDES = {
  windows: {
    label: 'Windows',
    steps: [
      { title: 'Back up your files', how: 'Copy everything you want to keep to OneDrive or an external drive, and check you can open it from another device.' },
      {
        title: 'Turn off Find my device and remove the computer from your account',
        how: 'Settings > Privacy & security > Find my device: turn it off. Then remove this computer from your Microsoft account.',
        link: { label: 'Your Microsoft devices', url: 'https://account.microsoft.com/devices' },
      },
      { title: 'Sign out of apps and browsers', how: 'Sign out of your browser, and of apps with licences you will reuse (Microsoft 365, Adobe and similar), so the licence moves to your next computer.' },
      {
        title: 'Remove firmware passwords and work management',
        how: 'If you set a BIOS or firmware password, remove it in the firmware settings. If this was a work computer, ask the IT department to release it from management (Intune or Autopilot).',
        warn: 'A buyer cannot remove these without you. probe flags them as locks.',
      },
      {
        title: 'Reset Windows and clean the drive',
        how: 'Settings > System > Recovery > Reset PC > Remove everything. Under Change settings, turn on Clean data. On Windows 10 it is under Settings > Update & Security > Recovery.',
        warn: 'Clean data can take an hour or more; it is what makes your files unrecoverable.',
      },
      { title: 'Stop at the setup screen', how: 'When the computer restarts to "Let’s start with region", shut it down. Do not sign in: the buyer sets it up as theirs.' },
      { title: 'Remove your extras', how: 'Take out memory cards, USB sticks and any SIM card. Keep the charger with the computer.' },
    ],
  },
  mac: {
    label: 'Mac',
    steps: [
      { title: 'Back up your files', how: 'Use Time Machine or copy your files to iCloud or an external drive.' },
      {
        title: 'Sign out of your Apple Account',
        how: 'System Settings > your name > Sign Out. This turns off Find My and Activation Lock, so the buyer can set the Mac up as theirs.',
        warn: 'Skip this and the Mac stays locked to you after it is erased.',
      },
      { title: 'Sign out of Messages and FaceTime, and unpair devices', how: 'Sign out in each app’s settings. Unpair Bluetooth keyboards, mice and AirPods you are keeping.' },
      { title: 'Remove a firmware password (Intel Macs)', how: 'Start up in Recovery, open Utilities > Startup Security Utility and turn off the firmware password. Apple silicon Macs have none.' },
      {
        title: 'Erase the Mac',
        how: 'System Settings > General > Transfer or Reset > Erase All Content and Settings. On older Macs without it, start up in Recovery, erase the drive in Disk Utility, then choose Reinstall macOS.',
      },
      {
        title: 'Check it is gone from your account',
        how: 'Look at the device list for your Apple Account and remove the Mac if it is still there.',
        link: { label: 'Your Apple Account', url: 'https://account.apple.com' },
      },
      { title: 'Company Macs: ask IT to release it', how: 'If your workplace set it up, ask them to remove it from Apple Business Manager, or it will enrol itself again for the buyer.' },
    ],
  },
  linux: {
    label: 'Linux',
    steps: [
      { title: 'Back up your files', how: 'Copy your home folder, including hidden files such as SSH keys, to an external drive.' },
      { title: 'Sign out of browsers and online accounts', how: 'Sign out of browser sync and any online accounts, and revoke this computer’s SSH keys and tokens where you used them.' },
      { title: 'Remove firmware passwords', how: 'If you set a BIOS or firmware password, remove it in the firmware settings.' },
      {
        title: 'Reinstall from a USB stick and erase the disk',
        how: 'Start from your distribution’s installer and choose to erase the disk. Turning on disk encryption during the new install, with a password you then give the buyer to change, keeps old data unreadable.',
        warn: 'A plain reinstall does not overwrite every block. If the old system was not encrypted, use the drive’s secure erase from the firmware settings or a live USB first.',
      },
      { title: 'Remove your extras', how: 'Take out memory cards and USB sticks. Keep the charger with the computer.' },
    ],
  },
};

/** The guide to open first for a scan's operating system. */
export function wipeOs(os) {
  if (os === 'macos') return 'mac';
  if (os === 'linux') return 'linux';
  if (os === 'windows') return 'windows';
  const ua = typeof navigator !== 'undefined' ? navigator.userAgent : '';
  if (/Mac OS X/.test(ua)) return 'mac';
  if (/Linux|X11/.test(ua)) return 'linux';
  return 'windows';
}
