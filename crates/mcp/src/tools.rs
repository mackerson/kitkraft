//! MCP tool names. The power is in the command catalog, not the tool count.

/// Tools exposed by `kit mcp`, in the order `tools/list` returns them. `render` comes later.
pub const TOOLS: &[&str] = &[
    "execute",
    "command_line",
    "inspect_record",
    "query_records",
    "diff_plugins",
];

#[cfg(test)]
mod tests {
    use super::TOOLS;

    #[test]
    fn tool_names_are_unique() {
        let mut names = TOOLS.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TOOLS.len());
    }
}
