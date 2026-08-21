fn main() {
    #[cfg(all(feature = "aws-lc-rs", feature = "rust-crypto"))]
    compile_error!("Crypto implementations cannot be enabled simultaneously");

    #[cfg(not(any(feature = "aws-lc-rs", feature = "rust-crypto")))]
    compile_error!("Crypto implementation must be enabled");

    #[cfg(all(feature = "tls-openssl", feature = "tls-rustls"))]
    compile_error!("TLS implementations cannot be enabled simultaneously");
}
