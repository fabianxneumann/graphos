use alloc::string::String;

pub fn help_text() -> String {
    let mut s = String::new();
    s.push_str("GraphOS Shell - Graph-native Traversal REPL\n");
    s.push_str("============================================\n");
    s.push_str("\nDirect Operations (.-prefix):\n");
    s.push_str("  .look [deep]        Show edges from current node\n");
    s.push_str("  .go <path>          Move cursor to node\n");
    s.push_str("  .spawn <Type> [n]   Create new node\n");
    s.push_str("  .link <s> <t> <k>   Connect two nodes\n");
    s.push_str("  .cut <s> <t>        Remove edge\n");
    s.push_str("  .find <Type> [d N]  Search graph (BFS)\n");
    s.push_str("  .touch <edge>       Traverse named edge\n");
    s.push_str("  .who                Show session info\n");
    s.push_str("  .bind <name> <tgt>  Create alias\n");
    s.push_str("  .ask <prompt>       AI inference\n");
    s.push_str("  .snapshot           Create graph snapshot\n");
    s.push_str("  .rollback <epoch>   Rollback to snapshot\n");
    s.push_str("  .store <data>       Store content-addressed blob\n");
    s.push_str("  .similar [k]        Find k similar nodes\n");
    s.push_str("  .physics [n]        Run n force-layout steps\n");
    s.push_str("  .clusters           Show detected clusters\n");
    s.push_str("  .help               This help\n");
    s.push_str("\nPaths:\n");
    s.push_str("  /root/foo           Absolute path\n");
    s.push_str("  ./bar               Relative path\n");
    s.push_str("  ..                  Parent (reverse edge)\n");
    s.push_str("  @name               Alias lookup\n");
    s.push_str("  #hex_id             Direct NodeId\n");
    s.push_str("\nNatural Language:\n");
    s.push_str("  Just type normally  AI interprets intent\n");
    s
}

pub fn prompt(cwd_type: u16) -> String {
    let type_name = match cwd_type {
        0x0001 => "root",
        0x0002 => "process",
        0x0003 => "memory",
        0x0004 => "file",
        0x0005 => "device",
        0x0006 => "model",
        0x0007 => "wasm",
        0x000B => "dir",
        0x000C => "sched",
        0x000D => "jit",
        0x000E => "serial",
        0x000F => "disk",
        0x0010 => "net",
        0x0011 => "fs",
        0x0012 => "ai",
        0x0013 => "shell",
        _ => "?",
    };
    alloc::format!("graphos:/{}>  ", type_name)
}
