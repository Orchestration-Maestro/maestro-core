//! An owned backup-loss-rebuild ranking drill and its safety/equality checks.

mod authority;
mod backup_loss_drill;
mod fixture;
mod qdrant;
mod ranking_oracle;
#[cfg(test)]
mod ranking_oracle_tests;
mod resume;
mod wipe_safety;
