// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `plume mcp` speaks the Model Context Protocol on stdin and stdout, so a
    // conversation can put a document in the workbook. Branching here rather than
    // shipping a second binary means the command a client has to spawn is one
    // that is already installed and already signed.
    //
    // Before Tauri, and before anything that could print: stdout is the
    // protocol from the first byte.
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        plume_lib::mcp::serve();
        return;
    }

    plume_lib::run()
}
