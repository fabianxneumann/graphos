use alloc::string::String;
use graphos_core::GraphPool;

use crate::ops;
use crate::parser;
use crate::render;
use crate::session::ShellSession;

/// Process one line of input, return (output, prompt)
pub fn process_line(
    input: &str,
    session: &mut ShellSession,
    pool: &mut GraphPool,
) -> (String, String) {
    let cmd = parser::parse(input);

    let output = match ops::execute(&cmd, session, pool) {
        Ok(text) => text,
        Err(e) => alloc::format!("[ERROR] {:?}", e),
    };

    let prompt = render::prompt(session.cwd.node_type());
    (output, prompt)
}
