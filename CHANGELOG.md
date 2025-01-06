# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased
### Added
- Exit with error when runtime dependencies are or will not be met (install)
- adds suggestion when status call not working

## [0.3.0] - 2024-04-21

### Changes
- Time idle before a break is subtracted from the break time
- User is notified if staying idle for longer will reset the work period

## [0.2.2] - 2024-04-15

### Added 
- Status file, use it to get the current status of `break_enforcer`. Useful in
  a bar of a window manager or when writing a widget.

### Fixes
- No longer crashing if grace/warn for lock duration smaller then work duration

### Changes
- Durations consisting of a single number without postfix unit or a colon in
  front are no longer allowed. These where usually the result of a user
  forgetting the unit. This led to way shorter break/work times then intended
- Grace duration is now `lock_warning` and is optional (omitting it will prevent
  a notification being send when the break/lock is close.

## [0.2.1] - 2024-04-13

### Fixed
- Removed and then readded devices are locked when appropriate

## [0.2.0] - 2024-04-09

### Added
- Installer and Uninstaller (remove). Sets up a service to run on boot.

## [0.1.0] 
Init release
