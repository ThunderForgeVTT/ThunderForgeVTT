// use reqwest_wasm::{Client, RequestBuilder, Url};

// pub struct HttpClient {
//     client: Client,
//     api_server: String,
// }

// impl HttpClient {
//     pub(crate) fn new() -> HttpClient {
//         HttpClient {
//             client: Client::new(),
//             api_server: String::from(match std::env::var("NODE_ENV") {
//                 Ok(v) => {
//                     if v.eq("development") {
//                         "http://127.0.0.1:30000"
//                     } else {
//                         ""
//                     }
//                 }
//                 Err(_) => "http://127.0.0.1:30000",
//             }),
//         }
//     }
//     fn build_url(&self, url: &str) -> String {
//         Url::parse(&format!("{}/{}", &self.api_server, &url).replace("//", "/"))
//             .unwrap()
//             .to_string()
//     }
//     pub fn get(&self, url: &str) -> RequestBuilder {
//         #[cfg(feature = "client")]
//         web_sys::console::debug_1(&format!("[core.utils.HttpClient][GET]: {}", &url).into());

//         self.client.clone().get(&self.build_url(url))
//     }
//     pub fn post(&self, url: &str) -> RequestBuilder {
//         #[cfg(feature = "client")]
//         web_sys::console::debug_1(&format!("[core.utils.HttpClient][POST]: {}", &url).into());

//         self.client.clone().post(&self.build_url(url))
//     }
//     pub fn delete(&self, url: &str) -> RequestBuilder {
//         #[cfg(feature = "client")]
//         web_sys::console::debug_1(&format!("[core.utils.HttpClient][DELETE]: {}", &url).into());

//         self.client.clone().delete(&self.build_url(url))
//     }
//     pub fn put(&self, url: &str) -> RequestBuilder {
//         #[cfg(feature = "client")]
//         web_sys::console::debug_1(&format!("[core.utils.HttpClient][PUT]: {}", &url).into());

//         self.client.clone().put(&self.build_url(url))
//     }
//     pub fn patch(&self, url: &str) -> RequestBuilder {
//         #[cfg(feature = "client")]
//         web_sys::console::debug_1(&format!("[core.utils.HttpClient][PATCH]: {}", &url).into());

//         self.client.clone().patch(&self.build_url(url))
//     }
//     pub fn head(&self, url: &str) -> RequestBuilder {
//         #[cfg(feature = "client")]
//         web_sys::console::debug_1(&format!("[core.utils.HttpClient][HEAD]: {}", &url).into());

//         self.client.clone().head(&self.build_url(url))
//     }
// }
