//! `agents-tui service`: the served agents-tui as a user service (#424).
//! For now only the service file's text ([`unit`]); the subcommand that
//! writes it and drives systemd or launchd comes with #426 and #427.
pub mod unit;
