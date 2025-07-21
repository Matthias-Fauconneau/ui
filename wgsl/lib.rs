pub fn wgsl(names: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
	for name in names {
		println!("cargo:rerun-if-changed=src/{name}.wgsl");
		let ref source = std::fs::read_to_string(format!("src/{name}.wgsl"))?;
		let ref module = naga::front::wgsl::parse_str(source)?;
		pub fn default<T: Default>() -> T { Default::default() }
		let ref out_dir = std::env::var("OUT_DIR")?;
		let ref spv = format!("{name}.spv");
		use naga::back::spv::*;
		std::fs::write(&format!("{out_dir}/{spv}"), bytemuck::cast_slice(&write_vec(module, &naga::valid::Validator::new(default(), default()).validate(module)?, &Options{flags: Options::default().flags|WriterFlags::DEBUG, ..default()}, None)?))?;
		let ref o = format!("{spv}.o");
		assert!(std::process::Command::new("objcopy").current_dir(out_dir).args(["-I","binary","-O","default","--set-section-alignment",".data=4", spv, o]).spawn().unwrap().wait().unwrap().success());
		println!("cargo:rustc-link-arg={out_dir}/{o}");
	}
	Ok(())
}