fn main() -> Result<(), Box<dyn std::error::Error>> { #[cfg(feature="text")] wgsl::wgsl(&["view"])?; Ok(()) }
