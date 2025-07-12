// Device update workflow types

export type VersionComparison = 'current' | 'patchBehind' | 'minorBehind' | 'majorBehind'

export interface BootloaderCheck {
  currentVersion: string
  latestVersion: string
  needsUpdate: boolean
}

export interface FirmwareCheck {
  currentVersion: string
  latestVersion: string
  needsUpdate: boolean
}

export interface InitializationCheck {
  initialized: boolean
  hasBackup: boolean
  imported: boolean
  needsSetup: boolean
}

export interface DeviceStatus {
  deviceId: string
  connected: boolean
  features?: DeviceFeatures
  needsBootloaderUpdate: boolean
  needsFirmwareUpdate: boolean
  needsInitialization: boolean
  needsPinUnlock: boolean
  bootloaderCheck?: BootloaderCheck
  firmwareCheck?: FirmwareCheck
  initializationCheck?: InitializationCheck
}

export interface DeviceFeatures {
  label?: string
  vendor?: string
  model?: string
  firmwareVariant?: string
  deviceId?: string
  language?: string
  bootloaderMode: boolean
  bootloader_mode?: boolean  // Backend sends this format
  version: string
  firmwareHash?: string
  bootloaderHash?: string
  bootloaderVersion?: string
  initialized: boolean
  imported?: boolean
  noBackup: boolean
  pinProtection: boolean
  pinCached: boolean
  passphraseProtection: boolean
  passphraseCached: boolean
  wipeCodeProtection: boolean
  autoLockDelayMs?: number
  policies: string[]
} 