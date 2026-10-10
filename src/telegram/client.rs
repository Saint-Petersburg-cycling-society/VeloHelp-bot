// SPDX-License-Identifier: AGPL-3.0-only

use reqwest::{tls::Certificate, Client};

/// Builds the Telegram HTTP client without consulting a runtime OS trust store.
///
/// This is intentional: the production image is `FROM scratch`. Mozilla root
/// certificates are compiled into the binary through `webpki-root-certs`.
///
/// # Errors
///
/// Returns [`reqwest::Error`] when a compiled-in certificate is invalid
/// or the client cannot be built.
pub fn telegram_http_client() -> Result<Client, reqwest::Error> {
    let roots = webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .map(|der| Certificate::from_der(der.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;

    Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(45))
        .tls_certs_only(roots)
        .https_only(true)
        .build()
}
