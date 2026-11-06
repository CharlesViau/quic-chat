use std::process::ExitCode;

fn main() -> ExitCode {
    match create_certificate() {
        Ok(identity) => {
            println!("{}", identity.cert.pem());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Certificate generation failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn create_certificate() -> Result<rcgen::CertifiedKey<rcgen::KeyPair>, rcgen::Error> {
    rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
}
