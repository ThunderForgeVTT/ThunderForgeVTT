use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "logger"])]
    pub fn log(message: &str);
    #[wasm_bindgen(js_namespace = ["window", "logger"])]
    pub fn info(message: &str);
    #[wasm_bindgen(js_namespace = ["window", "logger"])]
    pub fn warn(message: &str);
    #[wasm_bindgen(js_namespace = ["window", "logger"])]
    pub fn error(message: &str);
    #[wasm_bindgen(js_namespace = ["window", "logger"])]
    pub fn debug(message: &str);
}
