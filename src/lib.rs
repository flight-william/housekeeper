//! house: runs the tools that check the rules we care about, replies to agents
//! in one format, and records every run.

pub mod adapter;
pub mod adapters;
pub mod approval;
pub mod cli;
pub mod commitmsg;
pub mod config;
pub mod excuses;
pub mod files;
pub mod finding;
pub mod init;
pub mod push;
pub mod record;
pub mod reply;
pub mod run;
