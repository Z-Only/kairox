//! Data types and constants for the status bar component.
//!
//! Currently holds the notification log limit; additional data types
//! (e.g. notification severity levels) would be added here.

/// Maximum number of notification messages retained in the status bar log.
pub(super) const NOTIFICATION_LOG_LIMIT: usize = 100;
