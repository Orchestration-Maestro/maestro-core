//! Independent-process production lifecycle checks. Windows writers deliberately refuse;
//! its immutable reader fixture is exercised in the native unit suite.
#![cfg(all(feature = "engine", not(windows)))]
mod fixture;
mod native_processes;
