//! The burner key: thirty-two random bytes from the browser, kept in this origin's local storage so
//! the page remembers it. Testnet only: anything that can read the page's storage can read the key.

use std::sync::Arc;

use df_elysium_core::account::LocalKey;
use df_elysium_core::utils::hex;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::net::Burner;
use crate::state::Out;

const KEPT: &str = "df-elysium-core.reference-app.burner";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn js(error: JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

/// The key this browser kept, if any.
pub fn stored() -> Option<Burner> {
    let secret = storage()?.get_item(KEPT).ok()??;
    LocalKey::from_hex(&secret).ok().map(|key| Burner(Arc::new(key)))
}

/// Thirty-two bytes from the browser's cryptographic random source.
pub fn random() -> Out<[u8; 32]> {
    let crypto = web_sys::window().ok_or("no window")?.crypto().map_err(js)?;
    let mut bytes = [0u8; 32];
    crypto.get_random_values_with_u8_array(&mut bytes).map_err(js)?;
    Ok(bytes)
}

/// A new key, kept.
pub fn generate() -> Out<Burner> {
    keep(&hex::encode_prefixed(&random()?))
}

/// A key pasted in, kept.
pub fn import(secret: &str) -> Out<Burner> {
    keep(secret.trim())
}

fn keep(secret: &str) -> Out<Burner> {
    let key = LocalKey::from_hex(secret)?;
    storage().ok_or("this browser keeps no local storage")?.set_item(KEPT, secret).map_err(js)?;
    Ok(Burner(Arc::new(key)))
}

pub fn forget() {
    if let Some(storage) = storage() {
        let _ = storage.remove_item(KEPT);
    }
}

/// Puts the kept key on the clipboard.
pub async fn copy() -> Out<()> {
    let secret = storage().and_then(|storage| storage.get_item(KEPT).ok().flatten()).ok_or("no key is kept")?;
    let window = web_sys::window().ok_or("no window")?;
    JsFuture::from(window.navigator().clipboard().write_text(&secret)).await.map_err(js)?;
    Ok(())
}

/// Asks the person; `false` when the browser cannot.
pub fn confirm(question: &str) -> bool {
    web_sys::window().and_then(|window| window.confirm_with_message(question).ok()).unwrap_or(false)
}
