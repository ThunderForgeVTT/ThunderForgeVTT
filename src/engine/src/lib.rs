mod token;

use wasm_bindgen::prelude::*;
use yew::services::ConsoleService;

// Import the `window.alert` function from the Web.
#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

// Export a `greet` function from Rust to JavaScript, that alerts a
// hello message.
#[wasm_bindgen]
pub fn main() {
    ConsoleService::debug("Loading Engine...")
}
