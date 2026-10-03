//! The catalog host format probe (S3 C01): runs the pinned, already
//! installed AI hosts on synthetic catalog files in temporary homes, against a
//! scripted local model endpoint and the real `maestro mcp`, and records what
//! each host loads, exposes, calls and shadows.

mod code_digests;
mod copilot_formats;
mod host_pins;
mod host_sandbox;
mod mcp_hosts;
mod pi_projection;
mod provider;
