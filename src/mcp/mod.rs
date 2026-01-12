//! MCP (Model Context Protocol) server implementation for Screenly CLI.
//!
//! This module provides an MCP server that exposes the Screenly v4 API as tools
//! that can be used by AI assistants.

pub mod server;
pub mod tools;

#[cfg(test)]
mod tests;

pub use server::ScreenlyMcpServer;
