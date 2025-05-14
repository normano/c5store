// c5cli/src/commands/mod.rs

pub mod encrypt;
pub mod decrypt;
pub mod generate;

// Optional: Re-export the top-level argument structs if main.rs needs them directly
// without full path, though full path is often clearer.
// pub use encrypt::EncryptArgs;
// pub use decrypt::DecryptArgs;
// pub use generate::GenArgs; // Assuming GenArgs is the parent for generate subcommands